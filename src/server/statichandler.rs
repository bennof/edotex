use std::{
    error::Error,
    path::{Component, Path, PathBuf},
};

use axum::{
    body::Body,
    extract::Request,
    http::{Method, Response, StatusCode, header},
};

use tokio::fs;

use super::mime::get_content_type;

fn safe_request_path(path: &str) -> Option<PathBuf> {
    let path = path.trim_start_matches('/');

    if path.is_empty() {
        return Some(PathBuf::from("index.html"));
    }

    let path = Path::new(path);

    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        .then(|| path.to_path_buf())
}

pub async fn handle_static(
    request: Request,
    base_dir: impl AsRef<Path>,
) -> Result<Response<Body>, Box<dyn Error>> {
    let is_head = match *request.method() {
        Method::GET => false,
        Method::HEAD => true,
        _ => {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header(header::ALLOW, "GET, HEAD")
                .body(Body::empty())?);
        }
    };

    let relative = match safe_request_path(request.uri().path()) {
        Some(path) => path,
        None => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Invalid path"))?);
        }
    };

    let mut path = base_dir.as_ref().join(relative);

    if fs::metadata(&path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
    {
        path.push("index.html");
    }

    serve_static(&path, is_head).await
}

pub async fn serve_static(path: &Path, is_head: bool) -> Result<Response<Body>, Box<dyn Error>> {
    let metadata = match fs::metadata(path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))?);
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))?);
        }
        Err(err) => return Err(err.into()),
    };

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, get_content_type(path))
        .header(header::CONTENT_LENGTH, metadata.len().to_string());

    if let Ok(modified) = metadata.modified() {
        builder = builder.header(header::LAST_MODIFIED, httpdate::fmt_http_date(modified));
    }

    if is_head {
        return Ok(builder.body(Body::empty())?);
    }

    let data = match fs::read(path).await {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))?);
        }
        Err(err) => return Err(err.into()),
    };

    Ok(builder.body(Body::from(data))?)
}
