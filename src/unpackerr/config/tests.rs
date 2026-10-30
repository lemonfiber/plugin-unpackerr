use lemonfiber_ports::service::Curated;

use super::{rendered, Hook, Refused, HOOK_NAME};

const KEY: &str = "0123456789abcdef0123456789abcdef";

fn curated(media_type: &str, address: &str, folder: &str) -> Curated {
    Curated {
        media_type: media_type.to_owned(),
        address: address.to_owned(),
        key: KEY.to_owned(),
        folder: folder.to_owned(),
    }
}

fn hook() -> Hook<'static> {
    Hook {
        url: "http://unpackerr-adapter:8080/lemonfiber/unpackerr/events",
        template: "/config/lemonfiber-webhook.tmpl",
        token: "the-token",
    }
}

fn read(written: &str) -> toml::Table {
    written.parse().unwrap_or_default()
}

#[test]
fn every_curator_is_watched_in_the_section_unpackerr_names_its_application_by() {
    let written = rendered(
        &[
            curated("tv", "http://sonarr:8989", "/data/downloads/tv"),
            curated("movies", "http://radarr:7878", "/data/downloads/movies"),
            curated("music", "http://lidarr:8686", "/data/downloads/music"),
            curated("books", "http://bindery:8787", "/data/downloads/books"),
        ],
        &hook(),
    )
    .unwrap_or_default();

    let table = read(&written);
    for (section, url, folder) in [
        ("sonarr", "http://sonarr:8989", "/data/downloads/tv"),
        ("radarr", "http://radarr:7878", "/data/downloads/movies"),
        ("lidarr", "http://lidarr:8686", "/data/downloads/music"),
        ("readarr", "http://bindery:8787", "/data/downloads/books"),
    ] {
        let watched = table
            .get(section)
            .and_then(toml::Value::as_array)
            .and_then(|all| all.first())
            .cloned()
            .unwrap_or(toml::Value::Boolean(false));
        assert_eq!(watched.get("url").and_then(toml::Value::as_str), Some(url));
        assert_eq!(
            watched.get("api_key").and_then(toml::Value::as_str),
            Some(KEY)
        );
        assert_eq!(
            watched
                .get("paths")
                .and_then(toml::Value::as_array)
                .and_then(|paths| paths.first())
                .and_then(toml::Value::as_str),
            Some(folder)
        );
    }
    assert_eq!(
        table.get("debug").and_then(toml::Value::as_bool),
        Some(false)
    );
}

#[test]
fn every_event_is_posted_to_the_adapter_under_a_name_that_hides_its_address() {
    let table = read(&rendered(&[], &hook()).unwrap_or_default());

    let webhook = table
        .get("webhook")
        .and_then(toml::Value::as_array)
        .and_then(|all| all.first())
        .cloned()
        .unwrap_or(toml::Value::Boolean(false));
    assert_eq!(
        webhook.get("name").and_then(toml::Value::as_str),
        Some(HOOK_NAME)
    );
    assert_eq!(
        webhook.get("url").and_then(toml::Value::as_str),
        Some(hook().url)
    );
    assert_eq!(
        webhook.get("template_path").and_then(toml::Value::as_str),
        Some(hook().template)
    );
    assert_eq!(
        webhook.get("token").and_then(toml::Value::as_str),
        Some("the-token")
    );
    assert_eq!(
        webhook.get("silent").and_then(toml::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        webhook.get("events").map(ToString::to_string),
        Some("[0]".to_owned())
    );
    assert!(table.get("sonarr").is_none());
}

#[test]
fn a_value_out_of_its_grammar_is_refused_before_anything_is_written() {
    for (curator, refused) in [
        (
            curated("comics", "http://a:1", "/data"),
            Refused::Kind("comics".to_owned()),
        ),
        (
            curated("tv", "ftp://sonarr", "/data"),
            Refused::Address("ftp://sonarr".to_owned()),
        ),
        (
            curated("tv", "http://", "/data"),
            Refused::Address("http://".to_owned()),
        ),
        (
            curated("tv", "http://a\"\n[[sonarr]]", "/data"),
            Refused::Address("http://a\"\n[[sonarr]]".to_owned()),
        ),
        (
            curated("tv", "http://sonarr:8989", "data"),
            Refused::Folder("data".to_owned()),
        ),
        (
            curated("tv", "http://sonarr:8989", "/data/../etc"),
            Refused::Folder("/data/../etc".to_owned()),
        ),
        (
            curated("tv", "http://sonarr:8989", "/data\n"),
            Refused::Folder("/data\n".to_owned()),
        ),
        (
            curated(
                "tv",
                "http://sonarr:8989",
                &format!("/{}", "a".repeat(2048)),
            ),
            Refused::Folder(format!("/{}", "a".repeat(2048))),
        ),
    ] {
        assert_eq!(rendered(&[curator], &hook()), Err(refused));
    }
    for key in ["", "a\"b", "a b", &"a".repeat(129)] {
        let mut keyed = curated("tv", "http://sonarr:8989", "/data");
        key.clone_into(&mut keyed.key);
        assert_eq!(rendered(&[keyed], &hook()), Err(Refused::Key));
    }
}
