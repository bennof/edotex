use std::path::Path;

use axum::{
    body::Body,
    extract::Request,
    http::{Method, Response, StatusCode},
};
use tokio::fs;

use super::filehandler::{
    FileRequest, error_response, file_response, index_path, io_error, spa_fallback,
};

/// Serves GET/HEAD requests from a disk directory. URL percent escapes are not decoded.
///
/// `fallback` names a file below `base_dir`, e.g. `index.html`, served with 200 for
/// missing paths without a file extension, so single-page app routes like `/doc/42`
/// load the app. Missing paths with an extension, e.g. `/app.js`, still get 404.
pub async fn handle_static(
    request: Request,
    base_dir: impl AsRef<Path>,
    fallback: Option<&str>,
) -> Result<Response<Body>, StatusCode> {
    let file_request = match FileRequest::parse(&request) {
        Ok(value) => value,
        Err(status) => return error_response(status, request.method() == Method::HEAD),
    };
    let mut relative = file_request.path.clone();
    if file_request.directory {
        relative = index_path(&relative);
    }
    let mut path = base_dir.as_ref().join(&relative);
    if !file_request.directory
        && let Ok(metadata) = fs::metadata(&path).await
        && metadata.is_dir()
    {
        path = base_dir.as_ref().join(index_path(&relative));
    }
    let missing = match fs::metadata(&path).await {
        Ok(metadata) => !metadata.is_file(),
        Err(err) => matches!(
            err.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
        ),
    };
    if missing && let Some(fallback) = spa_fallback(&file_request.path, fallback) {
        path = base_dir.as_ref().join(fallback);
    }
    serve_static(&path, file_request.is_head).await
}

/// Serves a trusted disk path; the caller is responsible for its document root.
pub async fn serve_static(path: &Path, is_head: bool) -> Result<Response<Body>, StatusCode> {
    let metadata = match fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return error_response(StatusCode::NOT_FOUND, is_head),
        Err(err) => return io_error(err, is_head),
    };
    let body = if is_head {
        Body::empty()
    } else {
        match fs::read(path).await {
            Ok(data) => Body::from(data),
            Err(err) => return io_error(err, is_head),
        }
    };
    file_response(
        path,
        metadata.len(),
        metadata.modified().ok(),
        body,
        is_head,
    )
}
