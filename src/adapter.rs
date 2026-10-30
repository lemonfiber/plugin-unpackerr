//! What this adapter says it is, and every route it answers: the contract the core asks,
//! and the events Unpackerr posts.

use std::sync::Arc;

use axum::Router;
use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::archive;

use crate::unpackerr::{events, Unpackerr};

/// Where Unpackerr's configuration directory is, in the adapter's container, which the
/// adapter shares.
pub const ADDRESS: &str = "/unpackerr";

/// Where Unpackerr reaches the adapter, by its service id and the port it listens on.
pub const REACHED_AT: &str = "http://unpackerr-adapter:8080";

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Unpackerr";

/// The Unpackerr release this adapter's recordings were taken from.
pub const RELEASE: &str = "release-0.15.2";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:724c702e81727e3d90c42cf2b3d900674203906a6d135ba07416c9a3f9f67fbb";

/// The SHA-256 of `/app/unpackerr.conf.example` in each platform's image [`DIGEST`]
/// names, `linux/amd64` then `linux/arm64`, which the image copies to
/// `/config/unpackerr.conf` where it finds none: the one configuration the adapter did
/// not write that it takes as nobody's own. Each platform's image stamps the file with
/// the time it was generated, so each has its own digest.
pub const IMAGE_DEFAULTS: [&str; 2] = [
    "4942565c910c9b48053d54ac9383f23e75c3fe817d3fabde51f8f01e5f57a154",
    "2d53264ae968db01e16267fea09eab17e177ffa67d8f956502c70cd6d0b3ee9e",
];

/// Where Unpackerr posts its events.
#[must_use]
pub fn hook() -> String {
    format!("{REACHED_AT}{}", events::PATH)
}

/// What this adapter says it is: the contract it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![lemonfiber_contract::spoken(
            archive::extract::CAPABILITY,
            archive::extract::MAJOR,
        )],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The contract, served from `unpackerr` under `key`, ready while Unpackerr's
/// configuration directory is there to write.
#[must_use]
pub fn kit(key: String, unpackerr: Arc<Unpackerr>) -> Kit {
    let asked = Arc::clone(&unpackerr);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(archive::extract::served(unpackerr))
}

/// Every route: the contract under `key`, and the events `unpackerr` hears.
pub fn router(key: String, unpackerr: Arc<Unpackerr>) -> Router {
    let heard = unpackerr.events();
    kit(key, unpackerr).router().merge(events::router(heard))
}

#[cfg(test)]
mod tests;
