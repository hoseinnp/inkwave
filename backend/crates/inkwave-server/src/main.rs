use inkwave_server::app;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    println!("INKWAVE_PORT={}", local_addr.port());

    axum::serve(listener, app()).await?;

    Ok(())
}
