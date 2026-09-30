use std::{
    borrow::Cow,
    path::Path,
    time::{Duration, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::Request,
    http::{Method, Response, StatusCode},
};
use rust_embed::RustEmbed;

use super::filehandler::{FileRequest, error_response, file_response, index_path, spa_fallback};

/// Serves GET/HEAD requests from embedded assets. URL percent escapes are not decoded.
/// The asset value selects the type; rust-embed accesses its files through `E::get`.
///
/// `fallback` names an asset, e.g. `index.html`, served with 200 for missing paths
/// without a file extension, so single-page app routes like `/doc/42` load the app.
/// Missing paths with an extension, e.g. `/app.js`, still get 404.
pub async fn handle_embedded<E: RustEmbed>(
    request: Request,
    _embed: E,
    fallback: Option<&str>,
) -> Result<Response<Body>, StatusCode> {
    let file_request = match FileRequest::parse(&request) {
        Ok(value) => value,
        Err(status) => return error_response(status, request.method() == Method::HEAD),
    };
    let mut path = file_request.path;
    let file = if file_request.directory {
        None
    } else {
        E::get(&path)
    };
    let file = file
        .or_else(|| E::get(&index_path(&path)).inspect(|_| path = index_path(&path)))
        .or_else(|| {
            let fallback = spa_fallback(&path, fallback)?;
            path = fallback.to_owned();
            E::get(fallback)
        });
    let Some(file) = file else {
        return error_response(StatusCode::NOT_FOUND, file_request.is_head);
    };
    let modified = file
        .metadata
        .last_modified()
        .and_then(|seconds| UNIX_EPOCH.checked_add(Duration::from_secs(seconds)));
    let length = file.data.len() as u64;
    let body = if file_request.is_head {
        Body::empty()
    } else {
        match file.data {
            Cow::Borrowed(data) => Body::from(data),
            Cow::Owned(data) => Body::from(data),
        }
    };
    file_response(
        Path::new(&path),
        length,
        modified,
        body,
        file_request.is_head,
    )
}
