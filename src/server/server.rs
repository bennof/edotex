use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Request, State},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
};

use serde::Serialize;

pub fn write_json<T>(value: T) -> Response
where
    T: Serialize,
{
    Json(value).into_response()
}

pub struct Server {
    addr: String,
    mux: Router,
}

impl Server {
    pub fn new(addr: String) -> Self {
        Self {
            addr,
            mux: Router::new(),
        }
    }

    pub fn set_router(&mut self, router: Router) {
        self.mux = router;
    }

    pub fn log(msg: String) {
        println!("{}: {msg}", timestamp());
    }

    pub fn error(msg: String) {
        eprintln!("ERROR {}: {msg}", timestamp());
    }

    pub async fn listen(self) -> Result<(), Box<dyn std::error::Error>> {
        let server = Arc::new(self);

        let router = server
            .mux
            .clone()
            .layer(from_fn_with_state(Arc::clone(&server), logging_middleware));

        let listener = tokio::net::TcpListener::bind(&server.addr).await?;

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        Ok(())
    }
}

async fn logging_middleware(
    State(_server): State<Arc<Server>>,
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();

    let method = request.method().clone();
    let uri = request.uri().clone();

    let response = next.run(request).await;
    let status = response.status();

    if status.is_server_error() {
        Server::error("Internal error".into());
    } else if status.is_client_error() {
        Server::error("Client error".into());
    }

    Server::log(format!(
        "{} {} {} {}ms",
        method,
        uri,
        status,
        start.elapsed().as_millis(),
    ));

    response
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        Server::error(format!("failed to listen for shutdown signal: {err}"));
    }
}

fn timestamp() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();

    millis.to_string()
}
