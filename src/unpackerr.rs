//! Unpackerr, told which curators to watch by the configuration it reads when it starts,
//! and heard about its work through the events it posts.
//!
//! The adapter writes `unpackerr.conf` into Unpackerr's configuration directory, which it
//! shares, through the kit's shared-directory helper and owner-only, because it carries
//! each curator's key. Its first line carries the digest of the rest, so a file whose
//! digest no longer holds and that is not the default Unpackerr's image writes is
//! someone's own: it is named in the refusal and never written over. Unpackerr takes a configuration once it starts
//! again, which the core does when told so.

pub mod config;
pub mod events;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_adapter_kit::shared::{Shared, SharedName, Unshared};
use lemonfiber_ports::service::{Applied, Curated, Extracting, Extraction, Failure};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use self::config::{Hook, Refused};
use self::events::Events;
use crate::adapter::IMAGE_DEFAULTS;

/// What a failure names Unpackerr as.
const SERVICE: &str = "Unpackerr";

/// The file Unpackerr reads its configuration from, in its configuration directory.
pub const CONFIGURATION: &str = "unpackerr.conf";

/// The template Unpackerr writes each event with, beside it.
pub const TEMPLATE: &str = "lemonfiber-webhook.tmpl";

/// What the first line of a configuration the adapter wrote starts with, before the
/// digest of the rest.
const MARK: &str = "# lemonfiber wrote this; sha256 ";

/// Where Unpackerr finds the template, as it sees its configuration directory.
const TEMPLATE_AS_SEEN: &str = "/config/lemonfiber-webhook.tmpl";

/// The mode of the configuration, which carries every curator's key: Unpackerr's alone.
const SECRET: u32 = 0o600;

/// The mode of the files beside it, which carry nothing secret.
const READABLE: u32 = 0o644;

/// The bytes of randomness a token is minted from.
const TOKEN_BYTES: usize = 32;

/// One Unpackerr, told what to watch through its configuration directory.
pub struct Unpackerr {
    directory: PathBuf,
    hook: String,
    events: Arc<Events>,
}

#[derive(Deserialize)]
struct Written {
    #[serde(default)]
    webhook: Vec<Posted>,
}

#[derive(Deserialize)]
struct Posted {
    token: String,
}

impl Unpackerr {
    /// The Unpackerr whose configuration directory is shared at `directory`, posting its
    /// events to `hook`: proved by the token the configuration last written carries, or
    /// by one minted now where none was written.
    ///
    /// # Errors
    ///
    /// Where the operating system gave no randomness to mint a token from.
    pub async fn at(directory: &Path, hook: &str) -> Result<Self, getrandom::Error> {
        let unpackerr = Self {
            directory: directory.to_path_buf(),
            hook: hook.to_owned(),
            events: Arc::new(Events::new(String::new())),
        };
        let token = match unpackerr.written_token().await {
            Some(token) => token,
            None => minted()?,
        };
        Ok(Self {
            events: Arc::new(Events::new(token)),
            ..unpackerr
        })
    }

    /// What Unpackerr has said about its work.
    #[must_use]
    pub fn events(&self) -> Arc<Events> {
        Arc::clone(&self.events)
    }

    /// Whether Unpackerr's configuration directory is there to write.
    pub async fn answers(&self) -> bool {
        tokio::fs::metadata(&self.directory)
            .await
            .is_ok_and(|held| held.is_dir())
    }

    /// The token the configuration last written carries, where it is the one written.
    async fn written_token(&self) -> Option<String> {
        let held = self.read(CONFIGURATION).await.ok()??;
        toml::from_str::<Written>(ours(&held)?)
            .ok()?
            .webhook
            .into_iter()
            .next()
            .map(|posted| posted.token)
    }

    /// What `file` holds, or none where it is not there.
    async fn read(&self, file: &str) -> Result<Option<String>, Failure> {
        Shared::at(&self.directory)
            .read(&SharedName::new(file).map_err(unshared)?)
            .await
            .map_err(unshared)?
            .map(|bytes| {
                String::from_utf8(bytes).map_err(|_| refused(&format!("{file} is not text")))
            })
            .transpose()
    }

    /// Write `contents` to `file` with `mode`, in one step.
    async fn put(&self, file: &str, contents: String, mode: u32) -> Result<(), Failure> {
        Shared::at(&self.directory)
            .write(
                &SharedName::new(file).map_err(unshared)?,
                contents.into_bytes(),
                mode,
            )
            .await
            .map_err(unshared)
    }

    /// Whether the configuration is someone's own: neither one the adapter wrote nor the
    /// default Unpackerr's image writes.
    async fn edited(&self) -> Result<bool, Failure> {
        let Some(held) = self.read(CONFIGURATION).await? else {
            return Ok(false);
        };
        let digest = hex::encode(Sha256::digest(held.as_bytes()));
        Ok(ours(&held).is_none() && !IMAGE_DEFAULTS.contains(&digest.as_str()))
    }
}

#[async_trait]
impl Extracting for Unpackerr {
    async fn watch(&self, curators: &[Curated]) -> Result<Applied, Failure> {
        let configuration = config::rendered(
            curators,
            &Hook {
                url: &self.hook,
                template: TEMPLATE_AS_SEEN,
                token: self.events.token(),
            },
        )
        .map_err(|why| said(&why))?;
        if self.edited().await? {
            return Err(refused(&format!(
                "{CONFIGURATION} was changed by hand, so it is left as it is"
            )));
        }
        self.put(TEMPLATE, events::TEMPLATE.to_owned(), READABLE)
            .await?;
        self.put(CONFIGURATION, marked(&configuration), SECRET)
            .await?;
        Ok(Applied::OnRestart)
    }

    async fn working(&self) -> Result<Vec<Extraction>, Failure> {
        Ok(self.events.working())
    }
}

/// A token minted from the operating system's randomness, as hexadecimal.
///
/// # Errors
///
/// Where the operating system gave no randomness.
fn minted() -> Result<String, getrandom::Error> {
    let mut bytes = [0_u8; TOKEN_BYTES];
    getrandom::getrandom(&mut bytes)?;
    Ok(hex::encode(bytes))
}

/// `configuration`, its first line carrying the digest of the rest.
fn marked(configuration: &str) -> String {
    format!(
        "{MARK}{}\n{configuration}",
        hex::encode(Sha256::digest(configuration.as_bytes()))
    )
}

/// The configuration under the first line of `held`, where `held` is one the adapter
/// wrote and nobody has changed since.
fn ours(held: &str) -> Option<&str> {
    let (first, rest) = held.split_once('\n')?;
    let written = first.strip_prefix(MARK)?;
    (written == hex::encode(Sha256::digest(rest.as_bytes()))).then_some(rest)
}

/// What a refusal of a curator says.
fn said(why: &Refused) -> Failure {
    match why {
        Refused::Kind(kind) => Failure::Unsupported {
            service: SERVICE.to_owned(),
            detail: format!("Unpackerr watches no curator filing {kind:?}"),
        },
        Refused::Address(value) => refused(&format!("{value:?} is not an http or https address")),
        Refused::Key => refused("a curator's key is not letters and digits"),
        Refused::Folder(value) => refused(&format!("{value:?} is not an absolute folder")),
    }
}

/// A refusal saying `detail`.
fn refused(detail: &str) -> Failure {
    Failure::Refused {
        service: SERVICE.to_owned(),
        detail: detail.to_owned(),
    }
}

/// A file in Unpackerr's configuration directory that could not be reached: a link or
/// a file that is not plain is refused by name, and a directory that is not there to
/// read or write leaves Unpackerr unavailable.
fn unshared(why: Unshared) -> Failure {
    match why {
        Unshared::Failed(..) => Failure::Unavailable {
            service: SERVICE.to_owned(),
        },
        refusal => refused(&refusal.to_string()),
    }
}

#[cfg(test)]
mod tests;
