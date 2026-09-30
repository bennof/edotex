#![cfg(feature = "server")]

use axum::{
    Router,
    body::{Body, to_bytes},
    extract::Request,
    http::{Method, StatusCode, header},
};
use edotex::server::handle_embedded;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "tests/fixtures/web/"]
struct Assets;

fn build(method: Method, path: &str) -> Request {
    Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

async fn request(method: Method, path: &str) -> axum::http::Response<Body> {
    handle_embedded(build(method, path), Assets, None)
        .await
        .unwrap()
}

async fn with_fallback(method: Method, path: &str) -> axum::http::Response<Body> {
    handle_embedded(build(method, path), Assets, Some("index.html"))
        .await
        .unwrap()
}

#[tokio::test]
async fn serves_files_and_directory_indexes() {
    for (url, asset, mime) in [
        ("/", "index.html", "text/html; charset=utf-8"),
        ("/help", "help/index.html", "text/html; charset=utf-8"),
        ("/help/", "help/index.html", "text/html; charset=utf-8"),
        ("/style.css?v=1", "style.css", "text/css; charset=utf-8"),
    ] {
        let response = request(Method::GET, url).await;
        let expected = Assets::get(asset).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            expected.data.len().to_string()
        );
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .as_ref(),
            expected.data.as_ref()
        );
    }
}

#[tokio::test]
async fn head_preserves_headers_without_a_body_including_errors() {
    for url in ["/", "/missing", "/../secret"] {
        let get = request(Method::GET, url).await;
        let head = request(Method::HEAD, url).await;
        assert_eq!(head.status(), get.status());
        assert_eq!(head.headers(), get.headers());
        assert!(
            to_bytes(head.into_body(), usize::MAX)
                .await
                .unwrap()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn rejects_parent_paths_and_unsupported_methods() {
    assert_eq!(
        request(Method::GET, "/../index.html").await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(Method::GET, "/help/../../index.html")
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(Method::GET, "/missing").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(Method::GET, "/style.css/").await.status(),
        StatusCode::NOT_FOUND
    );
    let response = request(Method::POST, "/").await;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()[header::ALLOW], "GET, HEAD");
}

#[test]
fn can_be_registered_as_a_router_fallback() {
    let _: Router = Router::new().fallback(|request| handle_embedded(request, Assets, None));
}

#[tokio::test]
async fn spa_fallback_serves_routes_without_extension() {
    let index = Assets::get("index.html").unwrap();
    for url in ["/doc/42", "/doc/42/", "/missing?x=1"] {
        let response = with_fallback(Method::GET, url).await;
        assert_eq!(response.status(), StatusCode::OK, "{url}");
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .as_ref(),
            index.data.as_ref(),
            "{url}"
        );
    }
    // Existing files and directory indexes take precedence.
    let help = with_fallback(Method::GET, "/help").await;
    assert_eq!(
        to_bytes(help.into_body(), usize::MAX)
            .await
            .unwrap()
            .as_ref(),
        Assets::get("help/index.html").unwrap().data.as_ref()
    );
    for (url, status) in [
        ("/missing.js", StatusCode::NOT_FOUND),
        ("/doc/app.css", StatusCode::NOT_FOUND),
        ("/style.css/", StatusCode::NOT_FOUND),
        ("/../index.html", StatusCode::BAD_REQUEST),
    ] {
        assert_eq!(
            with_fallback(Method::GET, url).await.status(),
            status,
            "{url}"
        );
    }
    assert_eq!(
        with_fallback(Method::POST, "/doc/42").await.status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    // A missing fallback asset leaves the 404.
    let response = handle_embedded(build(Method::GET, "/doc/42"), Assets, Some("nope.html"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn disk_and_embedded_handlers_have_identical_http_behavior() {
    use edotex::server::handle_static;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/web");
    for fallback in [None, Some("index.html"), Some("nope.html")] {
        for method in [Method::GET, Method::HEAD, Method::POST] {
            for path in [
                "/",
                "/help",
                "/help/",
                "/help/.",
                "/style.css?v=1",
                "/missing",
                "/missing/child",
                "/style.css/",
                "/../index.html",
                "/help/../../index.html",
                "/C:/secret",
                "/%20missing",
                "/doc/42",
                "/missing.js",
            ] {
                let embedded = handle_embedded(build(method.clone(), path), Assets, fallback)
                    .await
                    .unwrap();
                let disk = handle_static(build(method.clone(), path), &root, fallback)
                    .await
                    .unwrap();
                let case = format!("{method} {path} {fallback:?}");
                assert_eq!(disk.status(), embedded.status(), "{case}");
                assert_eq!(disk.headers(), embedded.headers(), "{case}");
                assert_eq!(
                    to_bytes(disk.into_body(), usize::MAX).await.unwrap(),
                    to_bytes(embedded.into_body(), usize::MAX).await.unwrap(),
                    "{case}"
                );
            }
        }
    }
}
