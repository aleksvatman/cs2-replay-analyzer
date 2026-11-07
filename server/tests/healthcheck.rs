//! The `server healthcheck` probe against a real listener.

use std::net::SocketAddr;

use server::config::Config;
use server::state::AppState;
use tokio::net::TcpListener;

async fn serve(vars: &'static [(&'static str, &'static str)], dir: &std::path::Path) -> SocketAddr {
    let data = dir.to_str().unwrap().to_owned();
    let config = Config::from_lookup(|name| {
        if name == "DATA_DIR" {
            return Some(data.clone());
        }
        vars.iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| v.to_string())
    })
    .unwrap();
    let app = server::router(AppState::new(config).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

async fn probe(addr: SocketAddr) -> Result<(), String> {
    tokio::task::spawn_blocking(move || server::healthcheck(addr))
        .await
        .unwrap()
}

#[tokio::test]
async fn a_running_server_is_healthy_even_with_a_token() {
    let dir = tempfile::tempdir().unwrap();
    let addr = serve(&[("API_TOKEN", "0123456789abcdef0123")], dir.path()).await;
    assert_eq!(probe(addr).await, Ok(()));
}

#[tokio::test]
async fn an_unspecified_address_is_reached_through_loopback() {
    let dir = tempfile::tempdir().unwrap();
    let addr = serve(&[], dir.path()).await;
    let unspecified: SocketAddr = format!("0.0.0.0:{}", addr.port()).parse().unwrap();
    assert_eq!(probe(unspecified).await, Ok(()));
}

#[tokio::test]
async fn nothing_listening_is_unhealthy() {
    // take a free port and close it again
    let free = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let error = probe(free).await.unwrap_err();
    assert!(error.contains("cannot connect"), "{error}");
}

#[tokio::test]
async fn a_server_that_answers_something_else_is_unhealthy() {
    use tokio::io::AsyncWriteExt;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = socket
            .write_all(b"HTTP/1.0 503 Service Unavailable\r\n\r\n")
            .await;
    });
    let error = probe(addr).await.unwrap_err();
    assert!(error.contains("503"), "{error}");
}
