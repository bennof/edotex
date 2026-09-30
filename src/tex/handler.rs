//! Streaming HTTP interface to the synchronous TeX compiler.
use std::{
    collections::HashSet,
    convert::Infallible,
    error::Error,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{Body, Bytes, to_bytes},
    extract::Request,
    http::{Method, Response, StatusCode, header},
};
use multer::{Constraints, Multipart, SizeLimit};
use serde_json::json;
use tempfile::TempDir;
use tokio::{
    io::AsyncWriteExt,
    sync::{Semaphore, mpsc},
};
use tokio_stream::wrappers::ReceiverStream;

use super::{TeX_Env, TeX_Error, TeX_Output, compile};

static NEXT_BOUNDARY: AtomicU64 = AtomicU64::new(0);
type Sender = mpsc::Sender<Result<Bytes, Infallible>>;

/// Compiles a POST body, streaming status, NDJSON logs and a PDF in multipart/mixed.
/// Initialize the environment's TeX tree before registering this handler.
/// Once streaming starts, compilation errors are JSON parts in the HTTP 200 response.
/// Status messages stream during the run; the complete TeX log follows at the end.
/// Rejected requests (400, 413, 503) carry a plain-text reason in the body.
///
/// The body is either a single TeX document or `multipart/form-data`. In a multipart
/// body the first part is the TeX document; every further part is written to a
/// temporary directory under its relative `filename`, e.g. `images/logo.png`.
/// The document resolves relative file names against that directory, which is
/// removed after compilation. Single documents get an empty directory.
/// Missing, absolute, `..` or duplicate file names get 400.
///
/// `max_body_size` limits the request body; larger bodies get 413. `None` accepts any
/// size, which buffers the whole body in memory, so use it only in closed systems.
/// Axum's `DefaultBodyLimit` does not apply because the body is read directly.
///
/// `compile_slots` limits concurrent compilations; without a free permit the request
/// gets 503. A permit is held until compilation has finished. `None` means unlimited.
/// Share one semaphore between routes that should share the limit.
pub async fn handle_tex(
    request: Request,
    env: TeX_Env,
    max_body_size: Option<usize>,
    compile_slots: Option<Arc<Semaphore>>,
) -> Result<Response<Body>, StatusCode> {
    handle_with(
        request,
        env,
        max_body_size,
        compile_slots,
        |env, input, root, writer| compile(env, input, writer, None, Some(root), true),
    )
    .await
}

async fn handle_with<F>(
    request: Request,
    env: TeX_Env,
    max_body_size: Option<usize>,
    compile_slots: Option<Arc<Semaphore>>,
    compiler: F,
) -> Result<Response<Body>, StatusCode>
where
    F: FnOnce(&TeX_Env, Vec<u8>, &Path, &mut LogWriter) -> Result<TeX_Output, TeX_Error>
        + Send
        + 'static,
{
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
        return Rejection::too_large(limit).response();
    }
    let upload = match read_upload(request, limit).await {
        Ok(upload) => upload,
        Err(rejection) => return rejection.response(),
    };
    // The slot is held by the worker task until compilation has finished.
    let slot = match compile_slots
        .map(|slots| slots.try_acquire_owned())
        .transpose()
    {
        Ok(slot) => slot,
        Err(_) => {
            return Rejection::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "all compile slots are busy, try again later",
            )
            .response();
        }
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let boundary = format!(
        "edotex-{timestamp:x}-{:x}",
        NEXT_BOUNDARY.fetch_add(1, Ordering::Relaxed)
    );
    let (sender, receiver) = mpsc::channel(16);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/mixed; boundary={boundary}"),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from_stream(ReceiverStream::new(receiver)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tokio::spawn(async move {
        let _slot = slot;
        if send(
            &sender,
            json_part(&boundary, json!({"type":"progress", "status":"processing"})),
        )
        .await
        .is_err()
        {
            return;
        }
        if send(
            &sender,
            format!("--{boundary}\r\nContent-Type: application/x-ndjson\r\n\r\n"),
        )
        .await
        .is_err()
        {
            return;
        }
        let log_sender = sender.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut writer = LogWriter {
                sender: log_sender,
                pending: Vec::new(),
            };
            let Upload { input, root } = upload;
            let result = compiler(&env, input, root.path(), &mut writer);
            let _ = writer.flush();
            // Remove the uploaded files before the response ends.
            drop(root);
            result
        })
        .await;
        // The previous part is the NDJSON stream, including on a worker panic.
        if send(&sender, "\r\n").await.is_err() {
            return;
        }
        match result {
            Ok(Ok(output)) => {
                let pdf_headers = format!(
                    "--{boundary}\r\nContent-Type: application/pdf\r\n\
                     Content-Disposition: attachment; filename=\"result.pdf\"\r\n\
                     Content-Length: {}\r\n\r\n",
                    output.pdf.len()
                );
                if send(&sender, pdf_headers).await.is_err() {
                    return;
                }
                if send(&sender, output.pdf).await.is_err() {
                    return;
                }
                if send(&sender, "\r\n").await.is_err() {
                    return;
                }
            }
            result => {
                let message = match result {
                    Ok(Err(err)) => err.message,
                    Err(err) => {
                        crate::server::Server::error(format!("TeX worker failed: {err}"));
                        "TeX compilation worker failed".into()
                    }
                    Ok(Ok(_)) => unreachable!(),
                };
                if send(
                    &sender,
                    json_part(
                        &boundary,
                        json!({"type":"error", "status":"failed", "message":message}),
                    ),
                )
                .await
                .is_err()
                {
                    return;
                }
            }
        }
        let _ = send(&sender, format!("--{boundary}--\r\n")).await;
    });
    Ok(response)
}

/// Why a request was rejected before compiling; sent as a plain-text body.
struct Rejection {
    status: StatusCode,
    reason: String,
}

impl Rejection {
    fn new(status: StatusCode, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason.into(),
        }
    }

    fn bad_request(reason: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, reason)
    }

    fn too_large(limit: usize) -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("request body larger than {limit} bytes"),
        )
    }

    fn internal(reason: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, reason)
    }

    fn response(self) -> Result<Response<Body>, StatusCode> {
        Response::builder()
            .status(self.status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(self.reason))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// The TeX document and the directory holding the other uploaded files.
struct Upload {
    input: Vec<u8>,
    root: TempDir,
}

async fn read_upload(request: Request, limit: usize) -> Result<Upload, Rejection> {
    let root = tempfile::Builder::new()
        .prefix("edotex-")
        .tempdir()
        .map_err(|err| Rejection::internal(format!("cannot create upload directory: {err}")))?;
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let input = match multer::parse_boundary(content_type) {
        Ok(boundary) => read_multipart(request.into_body(), boundary, limit, root.path()).await?,
        Err(multer::Error::NoBoundary) => {
            return Err(Rejection::bad_request("multipart body without boundary"));
        }
        Err(_) => read_body(request.into_body(), limit).await?,
    };
    if input.is_empty() {
        return Err(Rejection::bad_request("empty TeX document"));
    }
    Ok(Upload { input, root })
}

async fn read_body(body: Body, limit: usize) -> Result<Vec<u8>, Rejection> {
    let bytes = to_bytes(body, limit).await.map_err(|err| {
        // Axum wraps the body's length-limit error as its source.
        if err
            .source()
            .is_some_and(|source| source.is::<http_body_util::LengthLimitError>())
        {
            Rejection::too_large(limit)
        } else {
            Rejection::bad_request(format!("unreadable request body: {err}"))
        }
    })?;
    Ok(bytes.to_vec())
}

/// Returns the first part and streams all further parts into files below `root`.
async fn read_multipart(
    body: Body,
    boundary: String,
    limit: usize,
    root: &Path,
) -> Result<Vec<u8>, Rejection> {
    let constraints = Constraints::new().size_limit(SizeLimit::new().whole_stream(limit as u64));
    let mut multipart = Multipart::with_constraints(body.into_data_stream(), boundary, constraints);
    let mut input = None;
    let mut names = HashSet::new();
    let invalid = |err| multipart_rejection(err, limit);
    while let Some(mut field) = multipart.next_field().await.map_err(invalid)? {
        if input.is_none() {
            input = Some(field.bytes().await.map_err(invalid)?.to_vec());
            continue;
        }
        let raw_name = field.file_name().map(str::to_owned);
        let name = raw_name.as_deref().and_then(upload_path).ok_or_else(|| {
            Rejection::bad_request(match &raw_name {
                Some(name) => {
                    format!("invalid asset name {name:?}: use a relative path without ..")
                }
                None => "asset part without a file name".into(),
            })
        })?;
        if !names.insert(name.clone()) {
            return Err(Rejection::bad_request(format!(
                "duplicate asset name {:?}",
                name.display()
            )));
        }
        let path = root.join(&name);
        let clash = |err| file_rejection(err, &name);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(clash)?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(clash)?;
        while let Some(chunk) = field.chunk().await.map_err(invalid)? {
            file.write_all(&chunk)
                .await
                .map_err(|err| Rejection::internal(format!("cannot store asset: {err}")))?;
        }
        file.flush()
            .await
            .map_err(|err| Rejection::internal(format!("cannot store asset: {err}")))?;
    }
    input.ok_or_else(|| Rejection::bad_request("multipart body without parts"))
}

/// Accepts only relative paths made of normal components, e.g. `images/logo.png`.
fn upload_path(name: &str) -> Option<PathBuf> {
    if name.contains('\\') {
        return None;
    }
    let mut path = PathBuf::new();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(part) => path.push(part),
            _ => return None,
        }
    }
    (!path.as_os_str().is_empty()).then_some(path)
}

fn multipart_rejection(err: multer::Error, limit: usize) -> Rejection {
    match err {
        multer::Error::StreamSizeExceeded { .. } | multer::Error::FieldSizeExceeded { .. } => {
            Rejection::too_large(limit)
        }
        err => Rejection::bad_request(format!("malformed multipart body: {err}")),
    }
}

/// Name clashes such as `a` and `a/b` are client errors.
fn file_rejection(err: io::Error, name: &Path) -> Rejection {
    match err.kind() {
        io::ErrorKind::AlreadyExists
        | io::ErrorKind::NotADirectory
        | io::ErrorKind::IsADirectory => Rejection::bad_request(format!(
            "asset name {:?} clashes with another asset",
            name.display()
        )),
        _ => Rejection::internal(format!("cannot store asset: {err}")),
    }
}

async fn send(sender: &Sender, data: impl Into<Bytes>) -> Result<(), ()> {
    sender.send(Ok(data.into())).await.map_err(|_| ())
}

fn json_part(boundary: &str, value: serde_json::Value) -> String {
    format!("--{boundary}\r\nContent-Type: application/json\r\n\r\n{value}\r\n")
}

/// Converts compiler lines into JSON records and applies channel backpressure.
struct LogWriter {
    sender: Sender,
    pending: Vec<u8>,
}

impl LogWriter {
    fn emit(&self, bytes: &[u8]) -> io::Result<()> {
        let record = json!({"type":"log", "message":String::from_utf8_lossy(bytes)});
        self.sender
            .blocking_send(Ok(Bytes::from(format!("{record}\n"))))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "HTTP client disconnected"))
    }
}

impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            self.emit(&self.pending[..end])?;
            self.pending.drain(..=end);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {
            self.emit(&self.pending)?;
            self.pending.clear();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    fn request(method: Method, body: impl Into<Body>) -> Request {
        Request::builder().method(method).body(body.into()).unwrap()
    }

    #[tokio::test]
    async fn streams_logs_before_compilation_finishes_and_preserves_pdf_bytes() {
        let (release, wait) = std::sync::mpsc::channel();
        let response = handle_with(
            request(Method::POST, "tex input"),
            TeX_Env::default(),
            None,
            None,
            move |_, input, _, writer| {
                assert_eq!(input, b"tex input");
                writer.write_all(b"a \"quoted\" log\n").unwrap();
                wait.recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                // A UTF-8 character split across writes and an unterminated last line.
                writer.write_all(&[0xc3]).unwrap();
                writer.write_all(&[0xa4]).unwrap();
                Ok(TeX_Output {
                    pdf: b"%PDF-1.7\n\x00\xff\n%%EOF".to_vec(),
                    output_log: String::new(),
                })
            },
        )
        .await
        .unwrap();
        let boundary = response.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .split("boundary=")
            .nth(1)
            .unwrap()
            .to_owned();
        let mut body = response.into_body();
        let mut prefix = Vec::new();
        for _ in 0..3 {
            let frame = tokio::time::timeout(std::time::Duration::from_secs(5), body.frame())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            prefix.extend_from_slice(&frame.into_data().unwrap());
        }
        let prefix = String::from_utf8(prefix).unwrap();
        assert!(prefix.contains("\"status\":\"processing\""));
        assert!(prefix.contains("Content-Type: application/x-ndjson"));
        assert!(prefix.contains("a \\\"quoted\\\" log"));
        release.send(()).unwrap();
        let tail = to_bytes(body, usize::MAX).await.unwrap();
        assert!(tail.windows("ä".len()).any(|part| part == "ä".as_bytes()));
        assert!(
            tail.windows(b"%PDF-1.7\n\x00\xff\n%%EOF".len())
                .any(|part| part == b"%PDF-1.7\n\x00\xff\n%%EOF")
        );
        assert!(tail.ends_with(format!("\r\n--{boundary}--\r\n").as_bytes()));
        assert!(String::from_utf8_lossy(&tail).contains("filename=\"result.pdf\""));
    }

    #[tokio::test]
    async fn compiler_failure_is_json_without_a_pdf() {
        let response = handle_with(
            request(Method::POST, "bad tex"),
            TeX_Env::default(),
            None,
            None,
            |_, _, _, writer| {
                writeln!(writer, "compiler diagnostic").unwrap();
                Err(TeX_Error {
                    message: "invalid TeX".into(),
                    output_log: String::new(),
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("\"type\":\"error\""));
        assert!(body.contains("invalid TeX"));
        assert!(!body.contains("application/pdf"));
        assert!(body.ends_with("--\r\n"));
    }

    fn succeed(
        _: &TeX_Env,
        _: Vec<u8>,
        _: &Path,
        _: &mut LogWriter,
    ) -> Result<TeX_Output, TeX_Error> {
        Ok(TeX_Output {
            pdf: Vec::new(),
            output_log: String::new(),
        })
    }

    async fn status(
        request: Request,
        max_body_size: Option<usize>,
        compile_slots: Option<Arc<Semaphore>>,
    ) -> StatusCode {
        match handle_with(
            request,
            TeX_Env::default(),
            max_body_size,
            compile_slots,
            succeed,
        )
        .await
        {
            Ok(response) => response.status(),
            Err(status) => status,
        }
    }

    #[tokio::test]
    async fn validates_requests_before_compiling() {
        let response = handle_tex(request(Method::GET, ""), TeX_Env::default(), None, None)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()[header::ALLOW], "POST");
        assert_eq!(
            status(request(Method::POST, ""), None, None).await,
            StatusCode::BAD_REQUEST
        );
        let body = Body::from_stream(tokio_stream::iter([Err::<Bytes, _>(io::Error::other(
            "read failed",
        ))]));
        assert_eq!(
            status(request(Method::POST, body), None, None).await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn enforces_optional_body_size_limit() {
        assert_eq!(
            status(request(Method::POST, vec![b'x'; 5]), Some(5), None).await,
            StatusCode::OK
        );
        assert_eq!(
            status(request(Method::POST, vec![b'x'; 6]), Some(5), None).await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        // Streamed body without Content-Length is limited while reading.
        let body = Body::from_stream(tokio_stream::iter([Ok::<_, Infallible>(Bytes::from(
            vec![b'x'; 6],
        ))]));
        assert_eq!(
            status(request(Method::POST, body), Some(5), None).await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        // An announced oversized body is rejected before it is read.
        let pending = Body::from_stream(tokio_stream::pending::<Result<Bytes, Infallible>>());
        let mut oversized = request(Method::POST, pending);
        oversized
            .headers_mut()
            .insert(header::CONTENT_LENGTH, "6".parse().unwrap());
        assert_eq!(
            status(oversized, Some(5), None).await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        // Without a limit, a body larger than axum's 2 MiB default is accepted.
        assert_eq!(
            status(
                request(Method::POST, vec![b'x'; 3 * 1024 * 1024]),
                None,
                None
            )
            .await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn rejects_requests_without_free_compile_slot() {
        let slots = Arc::new(Semaphore::new(1));
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let busy = handle_with(
            request(Method::POST, "tex"),
            TeX_Env::default(),
            None,
            Some(slots.clone()),
            move |env, input, root, writer| {
                let _ = wait.recv_timeout(std::time::Duration::from_secs(10));
                succeed(env, input, root, writer)
            },
        )
        .await
        .unwrap();
        // The response has started, but the slot is still held by the worker.
        assert_eq!(
            status(request(Method::POST, "tex"), None, Some(slots.clone())).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
        release.send(()).unwrap();
        to_bytes(busy.into_body(), usize::MAX).await.unwrap();
        // The worker task releases the slot right after the stream has ended.
        drop(
            tokio::time::timeout(std::time::Duration::from_secs(5), slots.acquire())
                .await
                .unwrap()
                .unwrap(),
        );
        assert_eq!(
            status(request(Method::POST, "tex"), None, Some(slots)).await,
            StatusCode::OK
        );
    }

    /// Builds a `multipart/form-data` request; parts without a file name omit it.
    fn multipart(parts: &[(Option<&str>, &[u8])]) -> Request {
        let mut body = Vec::new();
        for (index, (file_name, data)) in parts.iter().enumerate() {
            body.extend_from_slice(
                format!("--B\r\nContent-Disposition: form-data; name=\"f{index}\"").as_bytes(),
            );
            if let Some(file_name) = file_name {
                body.extend_from_slice(format!("; filename=\"{file_name}\"").as_bytes());
            }
            body.extend_from_slice(b"\r\n\r\n");
            body.extend_from_slice(data);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(b"--B--\r\n");
        Request::builder()
            .method(Method::POST)
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=B")
            .body(Body::from(body))
            .unwrap()
    }

    /// Runs a request and returns the compiler's input, root and file listing.
    async fn compiled_upload(request: Request) -> (Vec<u8>, PathBuf, Vec<(String, Vec<u8>)>) {
        let (seen, received) = std::sync::mpsc::channel();
        let response = handle_with(
            request,
            TeX_Env::default(),
            None,
            None,
            move |env, input, root, writer| {
                let mut files = Vec::new();
                let mut dirs = vec![root.to_path_buf()];
                while let Some(dir) = dirs.pop() {
                    for entry in std::fs::read_dir(dir).unwrap() {
                        let path = entry.unwrap().path();
                        if path.is_dir() {
                            dirs.push(path);
                        } else {
                            let name = path
                                .strip_prefix(root)
                                .unwrap()
                                .to_string_lossy()
                                .into_owned();
                            files.push((name, std::fs::read(&path).unwrap()));
                        }
                    }
                }
                files.sort();
                seen.send((input.clone(), root.to_path_buf(), files))
                    .unwrap();
                succeed(env, input, root, writer)
            },
        )
        .await
        .unwrap();
        to_bytes(response.into_body(), usize::MAX).await.unwrap();
        received.recv().unwrap()
    }

    #[tokio::test]
    async fn multipart_files_are_available_in_the_compile_root() {
        let (input, root, files) = compiled_upload(multipart(&[
            (Some("main.tex"), b"\\input{chapters/one}"),
            (Some("chapters/one.tex"), b"one"),
            (Some("logo.png"), b"\x89PNG\x00"),
        ]))
        .await;
        assert_eq!(input, b"\\input{chapters/one}");
        assert_eq!(
            files,
            [
                ("chapters/one.tex".to_owned(), b"one".to_vec()),
                ("logo.png".to_owned(), b"\x89PNG\x00".to_vec()),
            ]
        );
        // The response ends only after the upload directory was removed.
        assert!(!root.exists());
    }

    #[tokio::test]
    async fn single_documents_get_an_empty_compile_root() {
        let (input, root, files) = compiled_upload(request(Method::POST, "tex")).await;
        assert_eq!(input, b"tex");
        assert!(files.is_empty());
        assert!(!root.exists());
    }

    #[tokio::test]
    async fn rejects_invalid_multipart_uploads() {
        let tex: (Option<&str>, &[u8]) = (None, b"tex");
        for parts in [
            vec![],
            vec![(None, b"" as &[u8])],
            vec![tex, (None, b"missing file name")],
            vec![tex, (Some(""), b"empty name")],
            vec![tex, (Some("../escape.tex"), b"x")],
            vec![tex, (Some("a/../../escape.tex"), b"x")],
            vec![tex, (Some("/etc/escape.tex"), b"x")],
            vec![tex, (Some("./local.tex"), b"x")],
            vec![tex, (Some("dup.tex"), b"x"), (Some("dup.tex"), b"y")],
            vec![tex, (Some("a"), b"x"), (Some("a/b"), b"y")],
            vec![tex, (Some("a/b"), b"x"), (Some("a"), b"y")],
        ] {
            assert_eq!(
                status(multipart(&parts), None, None).await,
                StatusCode::BAD_REQUEST,
                "{parts:?}"
            );
        }
        let no_boundary = Request::builder()
            .method(Method::POST)
            .header(header::CONTENT_TYPE, "multipart/form-data")
            .body(Body::from("tex"))
            .unwrap();
        assert_eq!(
            status(no_boundary, None, None).await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn rejections_explain_the_reason() {
        let tex: (Option<&str>, &[u8]) = (None, b"tex");
        for (request, reason) in [
            (request(Method::POST, ""), "empty TeX document"),
            (
                multipart(&[tex, (Some("../x.png"), b"x")]),
                "invalid asset name \"../x.png\"",
            ),
            (
                multipart(&[tex, (Some("a.png"), b"x"), (Some("a.png"), b"y")]),
                "duplicate asset name \"a.png\"",
            ),
            (
                multipart(&[tex, (None, b"x")]),
                "asset part without a file name",
            ),
        ] {
            let response = handle_with(request, TeX_Env::default(), None, None, succeed)
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body = String::from_utf8_lossy(&body);
            assert!(body.contains(reason), "{body}");
        }
        let response = handle_with(
            request(Method::POST, vec![b'x'; 6]),
            TeX_Env::default(),
            Some(5),
            None,
            succeed,
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), b"request body larger than 5 bytes");
    }

    #[tokio::test]
    async fn body_size_limit_applies_to_multipart() {
        let parts: [(Option<&str>, &[u8]); 2] = [(None, b"tex"), (Some("big.bin"), &[b'x'; 1000])];
        assert_eq!(
            status(multipart(&parts), Some(500), None).await,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            status(multipart(&parts), Some(2000), None).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn worker_panic_closes_multipart_with_an_error() {
        let response = handle_with(
            request(Method::POST, "tex"),
            TeX_Env::default(),
            None,
            None,
            |_, _, _, _| panic!("test worker panic"),
        )
        .await
        .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("TeX compilation worker failed"));
        assert!(body.ends_with("--\r\n"));
    }

    #[tokio::test]
    async fn public_handler_reports_real_compiler_setup_errors() {
        let env = TeX_Env {
            cache_path: Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")),
            ..TeX_Env::default()
        };
        let response = handle_tex(request(Method::POST, "tex"), env, None, None)
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("\"type\":\"error\""));
    }
}
