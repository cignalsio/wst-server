use std::env;
use std::net::SocketAddr;
use futures_util::{SinkExt, StreamExt};
use log::*;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite;
use tokio_tungstenite::tungstenite;
use serde_json::Result;
use std::time::{SystemTime, UNIX_EPOCH};

mod protocol;
use protocol::Request;


#[tokio::main]
async fn main() {
    let _ = env_logger::try_init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(&addr).await;
    let listener = try_socket.expect("Failed to bind");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(accept_connection(peer, stream));
    }
}

async fn accept_connection(peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(peer, stream).await {
        error!("Error processing connection: {}", e);
        // match e {
        //     tungstenite::Error::ConnectionClosed | tungstenite::Error::Protocol(_) | tungstenite::Error::Utf8 => (),
        //     err => error!("Error processing connection: {}", err),
        // }
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await.expect("Failed to accept");
    info!("New WebSocket connection: {}", peer);

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    while let Some(data) = ws_receiver.next().await {
        let data = data.unwrap();
        if data.is_text() || data.is_binary() {
            let mut request: Request = serde_json::from_str(&data.to_string()).unwrap();

            let response = request.process(get_epoch_ms());

            let json = serde_json::to_string(&response).unwrap();
            ws_sender.send(tungstenite::Message::Text(json)).await.unwrap();
        }
    }

    Ok(())
}

fn get_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
