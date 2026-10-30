use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use lemonfiber_contract::adapter::{about_path, ready_path, About};
use serde_json::json;
use tower::ServiceExt as _;

use super::{about, hook, router, DIGEST, RELEASE, UPSTREAM};
use crate::unpackerr::events::PATH;
use crate::unpackerr::Unpackerr;

fn a_key() -> String {
    ('e'..='t').collect()
}

/// A configuration directory of its own, removed when dropped.
struct Directory(PathBuf);

impl Directory {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("unpackerr-adapter-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn answered(
    unpackerr: Arc<Unpackerr>,
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    let Ok(request) = request.body(Body::from(body.to_owned())) else {
        return (StatusCode::IM_A_TEAPOT, Vec::new());
    };
    let Ok(response) = router(a_key(), unpackerr).oneshot(request).await;
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, body)
}

async fn unpackerr(directory: &Directory) -> Arc<Unpackerr> {
    match Unpackerr::at(&directory.0, &hook()).await {
        Ok(unpackerr) => Arc::new(unpackerr),
        Err(why) => unreachable!("{why}"),
    }
}

#[test]
fn it_says_it_speaks_archive_extract_for_the_unpackerr_its_recordings_were_taken_from() {
    let about = about();

    assert_eq!(about.speaks, vec!["archive.extract@1".to_owned()]);
    assert_eq!(about.upstream, UPSTREAM);
    assert_eq!(
        about.releases.first().map(|one| one.version.as_str()),
        Some(RELEASE)
    );
    assert_eq!(
        about.releases.first().map(|one| one.digest.as_str()),
        Some(DIGEST)
    );
    assert_eq!(
        hook(),
        "http://unpackerr-adapter:8080/lemonfiber/unpackerr/events"
    );
}

#[tokio::test]
async fn it_names_itself_under_the_key_and_to_nobody_else() {
    let directory = Directory::new("about");
    let unpackerr = unpackerr(&directory).await;

    let (status, body) = answered(
        Arc::clone(&unpackerr),
        "GET",
        &about_path(),
        Some(&a_key()),
        "",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_slice::<About>(&body).ok(), Some(about()));

    let (status, _) = answered(unpackerr, "GET", &about_path(), Some("another"), "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn it_is_ready_while_unpackerrs_configuration_directory_is_there() {
    let directory = Directory::new("ready");
    let unpackerr = unpackerr(&directory).await;
    let (_, before) = answered(
        Arc::clone(&unpackerr),
        "GET",
        &ready_path(),
        Some(&a_key()),
        "",
    )
    .await;

    assert!(std::fs::create_dir_all(&directory.0).is_ok());
    let (_, after) = answered(unpackerr, "GET", &ready_path(), Some(&a_key()), "").await;

    assert_eq!(before, b"false");
    assert_eq!(after, b"true");
}

#[tokio::test]
async fn an_extraction_unpackerr_posts_is_what_the_contract_says_it_is_working_on() {
    let directory = Directory::new("working");
    let unpackerr = unpackerr(&directory).await;
    let token = unpackerr.events().token().to_owned();

    let (status, _) = answered(
        Arc::clone(&unpackerr),
        "POST",
        PATH,
        None,
        &json!({"token": token, "event": "extracting", "path": "/d/tv/Show.S01", "app": "Sonarr"})
            .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = answered(
        Arc::clone(&unpackerr),
        "POST",
        PATH,
        None,
        &json!({"token": "wrong", "event": "extracting", "path": "/d/tv/Other", "app": "Sonarr"})
            .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = answered(
        unpackerr,
        "POST",
        "/lemonfiber/archive.extract/v1/working",
        Some(&a_key()),
        "{}",
    )
    .await;
    assert!(status.is_success(), "{status}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).ok(),
        Some(json!([{"name": "Show.S01", "media_type": "tv"}]))
    );
}
