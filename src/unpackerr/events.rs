//! What Unpackerr tells the adapter about its work: one event per download each time
//! its extraction moves on, posted to the adapter over the network the two share alone.
//!
//! Each event carries the token Unpackerr was configured with in its body, compared in
//! constant time, and an event without it is refused before anything in it is read. A
//! body is read only up to [`LARGEST`], one whose shape does not parse is refused, and the
//! downloads held as extracting are bounded at [`MOST`], so no flood of events grows what
//! the adapter holds.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use lemonfiber_ports::media::{Kind, BOOKS, MUSIC};
use lemonfiber_ports::service::Extraction;
use serde::Deserialize;

/// Where Unpackerr posts its events, on the adapter.
pub const PATH: &str = "/lemonfiber/unpackerr/events";

/// The largest event read, in bytes.
pub const LARGEST: usize = 64 * 1024;

/// The most downloads held as extracting at once.
pub const MOST: usize = 512;

/// The template Unpackerr writes each event with: the token, what happened, to which
/// download, for which application.
pub const TEMPLATE: &str = r#"{"token": {{encode token}}, "event": {{encode .Event}}, "path": {{encode .Path}}, "app": {{encode .App}}}
"#;

/// What Unpackerr says has happened to a download, in its own words.
const EXTRACTING: &str = "extracting";

/// What Unpackerr says of a download it has not begun to extract.
const BEFORE: [&str; 2] = ["waiting", "queued"];

/// What Unpackerr says of a download it is done extracting.
const AFTER: [&str; 7] = [
    "extractfailed",
    "extracted",
    "imported",
    "deleting",
    "deletefailed",
    "deleted",
    "extractednothing",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Said {
    token: String,
    event: String,
    path: String,
    app: String,
}

/// The downloads Unpackerr has said it is extracting, by path.
pub struct Events {
    token: String,
    extracting: Mutex<BTreeMap<String, Extraction>>,
}

impl Events {
    /// None yet, each event to be proved with `token`.
    #[must_use]
    pub fn new(token: String) -> Self {
        Self {
            token,
            extracting: Mutex::new(BTreeMap::new()),
        }
    }

    /// The token every event proves itself with.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// What Unpackerr is extracting now, as it last said.
    #[must_use]
    pub fn working(&self) -> Vec<Extraction> {
        self.extracting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    /// Take one event's body, answering what Unpackerr is told.
    fn heard(&self, body: &[u8]) -> StatusCode {
        let Ok(event) = serde_json::from_slice::<Said>(body) else {
            return StatusCode::BAD_REQUEST;
        };
        if !lemonfiber_adapter_kit::same_secret(&self.token, &event.token) {
            return StatusCode::UNAUTHORIZED;
        }
        let Some(media_type) = media_type(&event.app) else {
            return StatusCode::NO_CONTENT;
        };
        let mut held = self
            .extracting
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match event.event.as_str() {
            EXTRACTING if held.contains_key(&event.path) || held.len() < MOST => {
                let name = event.path.rsplit('/').find(|part| !part.is_empty());
                held.insert(
                    event.path.clone(),
                    Extraction {
                        name: name.unwrap_or(&event.path).to_owned(),
                        media_type: media_type.to_owned(),
                    },
                );
                StatusCode::NO_CONTENT
            }
            EXTRACTING => StatusCode::SERVICE_UNAVAILABLE,
            said if AFTER.contains(&said) => {
                held.remove(&event.path);
                StatusCode::NO_CONTENT
            }
            said if BEFORE.contains(&said) => StatusCode::NO_CONTENT,
            _ => StatusCode::BAD_REQUEST,
        }
    }
}

/// The media type of the curator an application of Unpackerr's is, or none for a folder
/// it watches on its own.
fn media_type(app: &str) -> Option<&'static str> {
    match app {
        "Sonarr" => Some(Kind::Tv.media_type()),
        "Radarr" => Some(Kind::Movies.media_type()),
        "Lidarr" => Some(MUSIC),
        "Readarr" => Some(BOOKS),
        _ => None,
    }
}

/// The route Unpackerr posts its events to.
pub fn router(events: Arc<Events>) -> Router {
    Router::new()
        .route(PATH, post(posted))
        .layer(DefaultBodyLimit::max(LARGEST))
        .with_state(events)
}

/// One event posted.
async fn posted(State(events): State<Arc<Events>>, body: Bytes) -> StatusCode {
    events.heard(&body)
}

#[cfg(test)]
mod tests;
