use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use lemonfiber_ports::service::Extraction;
use serde_json::json;
use tower::ServiceExt as _;

use super::{router, Events, LARGEST, MOST, PATH};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn event(token: &str, said: &str, path: &str, app: &str) -> String {
    json!({ "token": token, "event": said, "path": path, "app": app }).to_string()
}

async fn posted(events: &Arc<Events>, body: impl Into<Body>) -> StatusCode {
    let Ok(request) = Request::post(PATH).body(body.into()) else {
        return StatusCode::IM_A_TEAPOT;
    };
    let Ok(response) = router(Arc::clone(events)).oneshot(request).await;
    response.status()
}

#[tokio::test]
async fn a_download_is_working_from_its_extraction_until_it_is_done() {
    let events = Arc::new(Events::new(TOKEN.to_owned()));
    let path = "/data/downloads/tv/Show.S01E01.1080p";

    for said in ["waiting", "queued", "extracting"] {
        assert_eq!(
            posted(&events, event(TOKEN, said, path, "Sonarr")).await,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        events.working(),
        vec![Extraction {
            name: "Show.S01E01.1080p".to_owned(),
            media_type: "tv".to_owned(),
        }]
    );

    assert_eq!(
        posted(&events, event(TOKEN, "extracted", path, "Sonarr")).await,
        StatusCode::NO_CONTENT
    );
    assert!(events.working().is_empty());
}

#[tokio::test]
async fn each_application_is_named_by_the_media_type_its_curator_files() {
    let events = Arc::new(Events::new(TOKEN.to_owned()));

    for (app, path) in [
        ("Sonarr", "/d/tv/a/"),
        ("Radarr", "/d/movies/b"),
        ("Lidarr", "/d/music/c"),
        ("Readarr", "/d/books/d"),
        ("Folder", "/d/watched/e"),
    ] {
        assert_eq!(
            posted(&events, event(TOKEN, "extracting", path, app)).await,
            StatusCode::NO_CONTENT
        );
    }

    let mut working: Vec<(String, String)> = events
        .working()
        .into_iter()
        .map(|one| (one.name, one.media_type))
        .collect();
    working.sort();
    assert_eq!(
        working,
        [("a", "tv"), ("b", "movies"), ("c", "music"), ("d", "books")]
            .map(|(name, kind)| (name.to_owned(), kind.to_owned()))
            .to_vec()
    );
}

#[tokio::test]
async fn an_event_with_a_wrong_or_missing_token_is_refused_and_changes_nothing() {
    let events = Arc::new(Events::new(TOKEN.to_owned()));
    let wrong = format!("{}0", &TOKEN[..TOKEN.len() - 1]);

    for body in [
        event(&wrong, "extracting", "/d/a", "Sonarr"),
        event("", "extracting", "/d/a", "Sonarr"),
        event(&TOKEN[..10], "extracting", "/d/a", "Sonarr"),
    ] {
        assert_eq!(posted(&events, body).await, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        posted(
            &events,
            json!({"event": "extracting", "path": "/d/a", "app": "Sonarr"}).to_string()
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert!(events.working().is_empty());
}

#[tokio::test]
async fn an_event_whose_shape_does_not_parse_is_refused() {
    let events = Arc::new(Events::new(TOKEN.to_owned()));

    for body in [
        "not json".to_owned(),
        json!({"token": TOKEN, "event": "extracting", "path": "/d/a", "app": "Sonarr", "more": 1})
            .to_string(),
        json!({"token": TOKEN, "event": 2, "path": "/d/a", "app": "Sonarr"}).to_string(),
        event(TOKEN, "exploded", "/d/a", "Sonarr"),
    ] {
        assert_eq!(posted(&events, body).await, StatusCode::BAD_REQUEST);
    }
    assert!(events.working().is_empty());
}

#[tokio::test]
async fn a_body_past_the_largest_is_never_read() {
    let events = Arc::new(Events::new(TOKEN.to_owned()));
    let body = event(TOKEN, "extracting", &"a".repeat(LARGEST), "Sonarr");

    assert_eq!(posted(&events, body).await, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(events.working().is_empty());
}

#[tokio::test]
async fn a_flood_of_extractions_holds_no_more_than_the_most() {
    let events = Arc::new(Events::new(TOKEN.to_owned()));

    for one in 0..MOST {
        let _ = posted(
            &events,
            event(TOKEN, "extracting", &format!("/d/{one}"), "Radarr"),
        )
        .await;
    }
    let over = posted(&events, event(TOKEN, "extracting", "/d/over", "Radarr")).await;
    let again = posted(&events, event(TOKEN, "extracting", "/d/0", "Radarr")).await;

    assert_eq!(over, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(again, StatusCode::NO_CONTENT);
    assert_eq!(events.working().len(), MOST);
}
