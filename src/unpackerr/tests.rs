use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use lemonfiber_ports::service::{Applied, Curated, Extracting as _, Failure};

use super::{events, Unpackerr, CONFIGURATION, TEMPLATE};

/// A configuration nobody wrote here: close to the default Unpackerr's image writes,
/// and not it.
const UNSET: &str = "debug = false\n\n[[sonarr]]\n# url = \"http://127.0.0.1:8989\"\n\n[[radarr]]\n\n[webserver]\n metrics = false\n";

const HOOK: &str = "http://unpackerr-adapter:8080/lemonfiber/unpackerr/events";

/// A configuration directory of its own, removed when dropped.
struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("unpackerr-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }

    fn file(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap_or_default()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn curators() -> Vec<Curated> {
    vec![Curated {
        media_type: "tv".to_owned(),
        address: "http://sonarr:8989".to_owned(),
        key: ('a'..='z').chain('0'..='5').collect(),
        folder: "/data/downloads/tv".to_owned(),
    }]
}

async fn unpackerr(directory: &Path) -> Option<Unpackerr> {
    Unpackerr::at(directory, HOOK).await.ok()
}

fn token_in(written: &str) -> Option<String> {
    let table: toml::Table = written.parse().ok()?;
    table
        .get("webhook")?
        .as_array()?
        .first()?
        .get("token")?
        .as_str()
        .map(str::to_owned)
}

#[tokio::test]
async fn curators_watched_are_written_owner_only_with_the_template_beside_them() {
    let directory = Directory::new("watch");
    let Some(unpackerr) = unpackerr(&directory.0).await else {
        unreachable!("a token is minted");
    };

    assert!(matches!(
        unpackerr.watch(&curators()).await,
        Ok(Applied::OnRestart)
    ));

    let written = directory.file(CONFIGURATION);
    let key = curators()
        .first()
        .map(|one| one.key.clone())
        .unwrap_or_default();
    assert!(written.contains(&format!("api_key = \"{key}\"")));
    assert_eq!(
        token_in(&written).as_deref(),
        Some(unpackerr.events().token())
    );
    assert_eq!(unpackerr.events().token().len(), 64);
    assert_eq!(directory.file(TEMPLATE), events::TEMPLATE);
    let mode = std::fs::metadata(directory.0.join(CONFIGURATION))
        .map(|held| held.permissions().mode() & 0o777)
        .unwrap_or_default();
    assert_eq!(mode, 0o600);
    assert!(unpackerr
        .working()
        .await
        .is_ok_and(|working| working.is_empty()));
}

#[tokio::test]
async fn the_token_written_is_kept_and_a_new_one_minted_only_where_none_was() {
    let directory = Directory::new("token");
    let Some(first) = unpackerr(&directory.0).await else {
        unreachable!("a token is minted");
    };
    let Some(fresh) = unpackerr(&directory.0).await else {
        unreachable!("a token is minted");
    };
    assert_ne!(first.events().token(), fresh.events().token());

    assert!(first.watch(&curators()).await.is_ok());
    let Some(again) = unpackerr(&directory.0).await else {
        unreachable!("a token is read");
    };

    assert_eq!(again.events().token(), first.events().token());
    assert!(again.watch(&curators()).await.is_ok());
}

#[tokio::test]
async fn a_configuration_changed_by_hand_is_named_and_never_written_over() {
    let directory = Directory::new("edited");
    let Some(unpackerr) = unpackerr(&directory.0).await else {
        unreachable!("a token is minted");
    };
    assert!(unpackerr.watch(&curators()).await.is_ok());
    let edited = directory
        .file(CONFIGURATION)
        .replace("debug = false", "debug = true");
    let _ = std::fs::write(directory.0.join(CONFIGURATION), &edited);

    let refused = unpackerr.watch(&curators()).await;

    assert!(
        matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains(CONFIGURATION)),
        "{refused:?}"
    );
    assert_eq!(directory.file(CONFIGURATION), edited);
    let Some(restarted) = super::Unpackerr::at(&directory.0, HOOK).await.ok() else {
        unreachable!("a token is minted");
    };
    assert_ne!(
        Some(restarted.events().token().to_owned()),
        token_in(&edited)
    );
}

#[tokio::test]
async fn a_link_or_a_file_that_is_not_text_is_refused_naming_it() {
    let linked = Directory::new("linked");
    let outside = Directory::new("linked-outside");
    let _ = std::fs::write(outside.0.join(CONFIGURATION), UNSET);
    let _ = std::os::unix::fs::symlink(outside.0.join(CONFIGURATION), linked.0.join(CONFIGURATION));
    let binary = Directory::new("binary");
    let _ = std::fs::write(binary.0.join(CONFIGURATION), [0xff, 0xfe]);

    for at in [&linked, &binary] {
        let Some(unpackerr) = unpackerr(&at.0).await else {
            unreachable!("a token is minted");
        };
        let refused = unpackerr.watch(&curators()).await;
        assert!(
            matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains(CONFIGURATION)),
            "{refused:?}"
        );
    }
    assert_eq!(outside.file(CONFIGURATION), UNSET);
}

#[tokio::test]
async fn a_curator_out_of_grammar_is_refused_before_anything_is_written() {
    let directory = Directory::new("refused");
    let Some(unpackerr) = unpackerr(&directory.0).await else {
        unreachable!("a token is minted");
    };

    for (media_type, address, key, folder) in [
        ("comics", "http://a:1", "abc", "/d"),
        ("tv", "ftp://a", "abc", "/d"),
        ("tv", "http://a:1", "a b", "/d"),
        ("tv", "http://a:1", "abc", "d"),
    ] {
        let answer = unpackerr
            .watch(&[Curated {
                media_type: media_type.to_owned(),
                address: address.to_owned(),
                key: key.to_owned(),
                folder: folder.to_owned(),
            }])
            .await;
        assert!(
            matches!(
                answer,
                Err(Failure::Refused { .. } | Failure::Unsupported { .. })
            ),
            "{media_type} {address} {folder}"
        );
    }
    assert!(matches!(
        unpackerr
            .watch(&[Curated {
                media_type: "comics".to_owned(),
                address: "http://a:1".to_owned(),
                key: "abc".to_owned(),
                folder: "/d".to_owned(),
            }])
            .await,
        Err(Failure::Unsupported { .. })
    ));
    assert!(directory.file(CONFIGURATION).is_empty());
}

#[tokio::test]
async fn a_directory_that_is_not_there_is_unavailable() {
    let Some(unpackerr) = unpackerr(Path::new("/nonexistent/unpackerr-configuration")).await else {
        unreachable!("a token is minted");
    };

    assert!(!unpackerr.answers().await);
    assert!(matches!(
        unpackerr.watch(&curators()).await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn a_directory_with_no_configuration_yet_is_written_one() {
    let directory = Directory::new("empty");
    let Some(unpackerr) = unpackerr(&directory.0).await else {
        unreachable!("a token is minted");
    };

    assert!(unpackerr.watch(&curators()).await.is_ok());

    assert_eq!(
        token_in(&directory.file(CONFIGURATION)).as_deref(),
        Some(unpackerr.events().token())
    );
}

#[tokio::test]
async fn any_configuration_but_its_own_or_the_images_default_is_named_and_left() {
    for (name, theirs) in [
        ("theirs", "[[sonarr]]\nurl = \"http://mine:8989\"\n"),
        ("hooked", "[[webhook]]\nurl = \"http://elsewhere/\"\n"),
        ("untoml", "[[sonarr\n"),
        ("unset", UNSET),
    ] {
        let directory = Directory::new(name);
        let _ = std::fs::write(directory.0.join(CONFIGURATION), theirs);
        let Some(unpackerr) = unpackerr(&directory.0).await else {
            unreachable!("a token is minted");
        };

        let refused = unpackerr.watch(&curators()).await;

        assert!(
            matches!(&refused, Err(Failure::Refused { detail, .. }) if detail.contains(CONFIGURATION)),
            "{name}: {refused:?}"
        );
        assert_eq!(directory.file(CONFIGURATION), theirs);
    }
}
