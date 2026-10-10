use inkwave_core::HealthStatus;
use inkwave_server::app;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::test]
async fn test_health_check() {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("failed to bind listener");
    let port = listener.local_addr().unwrap().port();

    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app()).await.unwrap();
    });

    // Make an HTTP request
    let client = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("failed to connect to server");

    let req =
        format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut rd, mut wr) = tokio::io::split(client);
    wr.write_all(req.as_bytes()).await.unwrap();

    let mut resp = String::new();
    rd.read_to_string(&mut resp).await.unwrap();

    assert!(resp.starts_with("HTTP/1.1 200 OK"));

    let body = resp.split("\r\n\r\n").nth(1).expect("missing body");
    let health: HealthStatus = serde_json::from_str(body).expect("failed to parse JSON response");

    assert_eq!(health.status, "ok");
    assert_eq!(health.version, env!("CARGO_PKG_VERSION"));

    server_handle.abort();
}
