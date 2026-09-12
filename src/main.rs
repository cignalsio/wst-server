use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::convert::Infallible;
use futures_util::{SinkExt, StreamExt};
use log::*;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::handshake;
use hyper::{header, upgrade, StatusCode, Body, Request, Response, Server, server::conn::AddrStream};
use hyper::service::{make_service_fn, service_fn};

mod protocol;

#[cfg(test)]
mod tests;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _ = env_logger::try_init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to install SIGTERM handler");

    let socket_addr = SocketAddr::from_str(&addr).unwrap();

    let make_svc = make_service_fn(|conn: & AddrStream| {
        let remote_addr = conn.remote_addr();
        async move {
            Ok::<_, Infallible>(service_fn(move |request: Request<Body>|
                handle_request(request, remote_addr)
            ))
        }
    });

    let server = Server::bind(&socket_addr).serve(make_svc);
    info!("Listening on: {}", server.local_addr());

    tokio::select! {
        result = server => {
            if let Err(e) = result {
                eprintln!("server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {},
        _ = async {
            #[cfg(unix)]
            terminate.recv().await;
            #[cfg(not(unix))]
            std::future::pending::<()>().await;
        } => {},
    }
}

async fn handle_request(mut request: Request<Body>, remote_addr: SocketAddr) -> Result<Response<Body>, Infallible> {
    match (request.uri().path(), request.headers().contains_key(header::UPGRADE)) {
        ("/wst", true) => {
            // assume request is a handshake, so create the handshake response
            let response = match handshake::server::create_response_with_body(&request, || Body::empty()) {
                Ok(response) => {
                    tokio::spawn(async move {
                        match upgrade::on(&mut request).await {
                            Ok(upgraded) => {
                                let ws_stream = WebSocketStream::from_raw_socket(
                                    upgraded,
                                    tokio_tungstenite::tungstenite::protocol::Role::Server,
                                    None,
                                ).await;

                                handle_websocket(ws_stream, remote_addr).await;
                            },
                            Err(e) =>
                                println!("error when trying to upgrade connection \
                                        from address {} to websocket connection. \
                                        Error is: {}", remote_addr, e),
                        }
                    });
                    response
                },
                Err(error) => {
                    //probably the handshake request is not up to spec for websocket
                    println!("Failed to create websocket response \
                                to request from address {}. \
                                Error is: {}", remote_addr, error);
                    let mut res = Response::new(Body::from(format!("Failed to create websocket: {}", error)));
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            Ok::<_, Infallible>(response)
        },
        ("/", false) => {
            Ok(Response::new(Body::from(format!("WST Server - Connect via WebSocket"))))
        },
        (_, _) => {
            let mut res = Response::new(Body::from(format!("Not found")));
            *res.status_mut() = StatusCode::NOT_FOUND;
            return Ok(res);
        }
    }
}

async fn handle_websocket<S>(ws_stream: WebSocketStream<S>, remote_addr: SocketAddr)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut ws_write, mut ws_read) = ws_stream.split();

    while let Some(data) = ws_read.next().await {
        let data = match data {
            Ok(data) => data,
            Err(error) => {
                log_websocket_error(remote_addr, "read", &error);
                break;
            }
        };

        if !data.is_text() && !data.is_binary() {
            // Keep polling so Tungstenite flushes automatic pong and close replies.
            continue;
        }

        let mut request: protocol::Request = match serde_json::from_slice(&data.into_data()) {
            Ok(request) => request,
            Err(error) => {
                warn!("Invalid WST request from {}: {}", remote_addr, error);
                break;
            }
        };

        let response = request.process(get_epoch_ms());
        let json = match serde_json::to_string(&response) {
            Ok(json) => json,
            Err(error) => {
                error!("Failed to serialize WST response for {}: {}", remote_addr, error);
                break;
            }
        };

        if let Err(error) = ws_write.send(tungstenite::Message::Text(json)).await {
            log_websocket_error(remote_addr, "write", &error);
            break;
        }
    }
}

fn log_websocket_error(remote_addr: SocketAddr, operation: &str, error: &tungstenite::Error) {
    match error {
        tungstenite::Error::ConnectionClosed
        | tungstenite::Error::AlreadyClosed
        | tungstenite::Error::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake) => {
            debug!("WebSocket {} ended for {}: {}", operation, remote_addr, error);
        }
        _ => warn!("WebSocket {} failed for {}: {}", operation, remote_addr, error),
    }
}

fn get_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
