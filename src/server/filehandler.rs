//! Shared HTTP behavior for disk-backed and embedded files.
use std::{path::Path, time::SystemTime};

use axum::{
    body::Body,
    extract::Request,
    http::{Method, Response, StatusCode, header},
};

use super::{Server, mime::get_content_type};

pub(super) struct FileRequest {
    pub path: String,
    pub is_head: bool,
    pub directory: bool,
}

impl FileRequest {
    pub fn parse(request: &Request) -> Result<Self, StatusCode> {
        let is_head = match *request.method() {
            Method::GET => false,
            Method::HEAD => true,
            _ => return Err(StatusCode::METHOD_NOT_ALLOWED),
        };
        let raw = request.uri().path();
        // Use URL separators on every platform. Do not allow Windows prefixes,
        // parent components or alternate separators to escape the document root.
        if raw.contains(['\\', '\0', ':']) || raw.split('/').any(|part| part == "..") {
            return Err(StatusCode::BAD_REQUEST);
        }
        let path = raw
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect::<Vec<_>>()
            .join("/");
        let directory = path.is_empty() || raw.ends_with('/') || raw.ends_with("/.");
        Ok(Self {
            path,
            is_head,
            directory,
        })
    }
}

pub(super) fn index_path(path: &str) -> String {
    if path.is_empty() {
        "index.html".into()
    } else {
        format!("{path}/index.html")
    }
}

/// Returns the SPA fallback for a missing path whose last segment has no file
/// extension, e.g. `doc/42`; missing assets such as `app.js` stay 404.
pub(super) fn spa_fallback<'a>(path: &str, fallback: Option<&'a str>) -> Option<&'a str> {
    let last = path.rsplit('/').next().unwrap_or_default();
    fallback.filter(|_| !last.contains('.'))
}

pub(super) fn error_response(
    status: StatusCode,
    is_head: bool,
) -> Result<Response<Body>, StatusCode> {
    let message = match status {
        StatusCode::BAD_REQUEST => "Invalid path",
        StatusCode::NOT_FOUND => "404 Not Found",
        StatusCode::METHOD_NOT_ALLOWED => "405 Method Not Allowed",
        _ => "500 Internal Server Error",
    };
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CONTENT_LENGTH, message.len().to_string());
    if status == StatusCode::METHOD_NOT_ALLOWED {
        builder = builder.header(header::ALLOW, "GET, HEAD");
    }
    builder
        .body(if is_head {
            Body::empty()
        } else {
            Body::from(message)
        })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) fn file_response(
    path: &Path,
    length: u64,
    modified: Option<SystemTime>,
    body: Body,
    is_head: bool,
) -> Result<Response<Body>, StatusCode> {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, get_content_type(path))
        .header(header::CONTENT_LENGTH, length.to_string());
    if let Some(modified) = modified {
        builder = builder.header(header::LAST_MODIFIED, httpdate::fmt_http_date(modified));
    }
    builder
        .body(if is_head { Body::empty() } else { body })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub(super) fn io_error(err: std::io::Error, is_head: bool) -> Result<Response<Body>, StatusCode> {
    let status = match err.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory => StatusCode::NOT_FOUND,
        _ => {
            Server::error(err.to_string());
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };
    error_response(status, is_head)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn io_failures_use_http_errors_and_keep_head_empty() {
        for (kind, status) in [
            (std::io::ErrorKind::NotFound, StatusCode::NOT_FOUND),
            (std::io::ErrorKind::NotADirectory, StatusCode::NOT_FOUND),
            (
                std::io::ErrorKind::PermissionDenied,
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            let get = io_error(std::io::Error::from(kind), false).unwrap();
            let head = io_error(std::io::Error::from(kind), true).unwrap();
            assert_eq!(get.status(), status);
            assert_eq!(head.status(), status);
            assert_eq!(get.headers(), head.headers());
            assert!(
                to_bytes(head.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
