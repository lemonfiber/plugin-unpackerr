//! The configuration Unpackerr reads when it starts, written from the curators it is told
//! to watch and the webhook it tells the adapter about its work through.
//!
//! Written as TOML that `toml` quotes, so no value carries structure of its own, and every
//! value is held to its grammar first: an address is `http` or `https`, a key is letters
//! and digits, a folder is an absolute path with no `..`.

use lemonfiber_ports::media::{Kind, BOOKS, MUSIC};
use lemonfiber_ports::service::Curated;
use serde::Serialize;

/// The longest key a curator is reached with.
const LONGEST_KEY: usize = 128;

/// The longest address or folder written.
const LONGEST: usize = 2048;

/// Every event Unpackerr posts, as its configuration names them all.
const EVERY_EVENT: u8 = 0;

/// The name the webhook is logged under, so its address is never logged.
pub const HOOK_NAME: &str = "lemonfiber";

/// Why a curator was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// A media type no section of Unpackerr's configuration watches.
    Kind(String),
    /// An address that is not `http` or `https`, or carries what an address does not.
    Address(String),
    /// A key that is not letters and digits.
    Key,
    /// A folder that is not an absolute path, or climbs out of one.
    Folder(String),
}

/// Where Unpackerr posts its events, and what it proves itself with.
pub struct Hook<'a> {
    /// The address, which carries no secret.
    pub url: &'a str,
    /// The template Unpackerr writes each event with, as Unpackerr sees its path.
    pub template: &'a str,
    /// The token each event carries in its body.
    pub token: &'a str,
}

#[derive(Serialize)]
struct Configuration<'a> {
    debug: bool,
    quiet: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sonarr: Vec<Watched<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    radarr: Vec<Watched<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lidarr: Vec<Watched<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    readarr: Vec<Watched<'a>>,
    webhook: Vec<Webhook<'a>>,
}

#[derive(Serialize)]
struct Watched<'a> {
    url: &'a str,
    api_key: &'a str,
    paths: Vec<&'a str>,
}

#[derive(Serialize)]
struct Webhook<'a> {
    name: &'static str,
    url: &'a str,
    template_path: &'a str,
    token: &'a str,
    silent: bool,
    events: Vec<u8>,
}

/// Unpackerr's configuration watching `curators` and posting every event to `hook`.
///
/// # Errors
///
/// [`Refused`] naming the first value that is not of its grammar.
pub fn rendered(curators: &[Curated], hook: &Hook<'_>) -> Result<String, Refused> {
    let mut configuration = Configuration {
        debug: false,
        quiet: false,
        sonarr: Vec::new(),
        radarr: Vec::new(),
        lidarr: Vec::new(),
        readarr: Vec::new(),
        webhook: vec![Webhook {
            name: HOOK_NAME,
            url: hook.url,
            template_path: hook.template,
            token: hook.token,
            silent: true,
            events: vec![EVERY_EVENT],
        }],
    };
    for curator in curators {
        let watched = Watched {
            url: address(&curator.address)?,
            api_key: key(&curator.key)?,
            paths: vec![folder(&curator.folder)?],
        };
        let section = match curator.media_type.as_str() {
            kind if kind == Kind::Tv.media_type() => &mut configuration.sonarr,
            kind if kind == Kind::Movies.media_type() => &mut configuration.radarr,
            MUSIC => &mut configuration.lidarr,
            BOOKS => &mut configuration.readarr,
            other => return Err(Refused::Kind(other.to_owned())),
        };
        section.push(watched);
    }
    Ok(toml::to_string(&configuration).unwrap_or_default())
}

/// `value`, where it is an `http` or `https` address of the characters an address is
/// written in.
fn address(value: &str) -> Result<&str, Refused> {
    let lower = value.to_ascii_lowercase();
    let plain = value.len() <= LONGEST
        && lower
            .strip_prefix("https://")
            .or_else(|| lower.strip_prefix("http://"))
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'.' | b'_' | b':' | b'/' | b'[' | b']')
        });
    if plain {
        Ok(value)
    } else {
        Err(Refused::Address(value.to_owned()))
    }
}

/// `value`, where it is a key of letters and digits.
fn key(value: &str) -> Result<&str, Refused> {
    if !value.is_empty()
        && value.len() <= LONGEST_KEY
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        Ok(value)
    } else {
        Err(Refused::Key)
    }
}

/// `value`, where it is an absolute path that never climbs out of itself.
fn folder(value: &str) -> Result<&str, Refused> {
    let plain = value.len() <= LONGEST
        && value.starts_with('/')
        && !value.chars().any(char::is_control)
        && value.split('/').all(|part| part != "..");
    if plain {
        Ok(value)
    } else {
        Err(Refused::Folder(value.to_owned()))
    }
}

#[cfg(test)]
mod tests;
