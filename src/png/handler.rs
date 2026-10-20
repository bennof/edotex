//! HTTP interface: POST a PDF, get its first page as PNG.
use std::error::Error;

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{Method, Response, StatusCode, header},
};

use super::pdf_to_png;

/// Render scale; 2.0 ≈ 144 dpi.
const SCALE: f32 = 2.0;

/// Renders the first page of the PDF in a POST body and answers with `image/png`.
/// Rejected requests (400, 405, 413, 422) carry a plain-text reason in the body.
///
/// `max_body_size` limits the request body; larger bodies get 413. `None` accepts any
/// size, which buffers the whole body in memory, so use it only in closed systems.
/// Rendering is CPU-bound and runs on a blocking thread.
pub async fn handle_png(
    request: Request,
    max_body_size: Option<usize>,
) -> Result<Response<Body>, StatusCode> {
    if request.method() != Method::POST {
        return Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header(header::ALLOW, "POST")
            .body(Body::empty())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }
    let limit = max_body_size.unwrap_or(usize::MAX);
    // Reject announced oversized bodies without reading them.
    let content_length = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());
    if content_length.is_some_and(|length| length > limit as u64) {
        return too_large(limit);
    }
    let pdf = match to_bytes(request.into_body(), limit).await {
        Ok(bytes) => bytes.to_vec(),
        // Axum wraps the body's length-limit error as its source.
        Err(err)
            if err
                .source()
                .is_some_and(|source| source.is::<http_body_util::LengthLimitError>()) =>
        {
            return too_large(limit);
        }
        Err(err) => {
            return text(
                StatusCode::BAD_REQUEST,
                format!("unreadable request body: {err}"),
            );
        }
    };
    if pdf.is_empty() {
        return text(StatusCode::BAD_REQUEST, "empty PDF");
    }

    // A panic in the renderer ends the blocking task, not the server.
    match tokio::task::spawn_blocking(move || pdf_to_png(pdf, 0, SCALE)).await {
        Ok(Some(png)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(png))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR),
        Ok(None) | Err(_) => text(
            StatusCode::UNPROCESSABLE_ENTITY,
            "not a readable PDF or no first page",
        ),
    }
}

fn too_large(limit: usize) -> Result<Response<Body>, StatusCode> {
    text(
        StatusCode::PAYLOAD_TOO_LARGE,
        format!("request body larger than {limit} bytes"),
    )
}

fn text(status: StatusCode, reason: impl Into<String>) -> Result<Response<Body>, StatusCode> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(reason.into()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    fn request(method: Method, body: impl Into<Body>) -> Request {
        Request::builder().method(method).body(body.into()).unwrap()
    }

    /// A one-page PDF (100 × 100 pt, a filled square) with a correct xref table.
    fn small_pdf() -> Vec<u8> {
        let content = "0 0 1 rg 10 10 80 80 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources << >> /Contents 4 0 R >>"
                .to_owned(),
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ];
        let mut pdf = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
        }
        let xref = pdf.len();
        pdf.push_str(&format!(
            "xref\n0 {}\n0000000000 65535 f \n",
            objects.len() + 1
        ));
        for offset in offsets {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        ));
        pdf.into_bytes()
    }

    async fn body(response: Response<Body>) -> Vec<u8> {
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec()
    }

    #[tokio::test]
    async fn renders_the_first_page_as_png() {
        let response = handle_png(request(Method::POST, small_pdf()), None)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert!(body(response).await.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[tokio::test]
    async fn rejects_other_methods() {
        let response = handle_png(request(Method::GET, Body::empty()), None)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()[header::ALLOW], "POST");
    }

    #[tokio::test]
    async fn rejects_an_empty_body() {
        let response = handle_png(request(Method::POST, Body::empty()), None)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn rejects_a_body_that_is_no_pdf() {
        let response = handle_png(request(Method::POST, "no pdf"), None)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body(response).await,
            b"not a readable PDF or no first page".to_vec()
        );
    }

    #[tokio::test]
    async fn rejects_a_body_over_the_limit() {
        let response = handle_png(request(Method::POST, small_pdf()), Some(10))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
