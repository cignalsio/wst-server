use std::env;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use std::convert::Infallible;
use futures_util::{SinkExt, StreamExt};
use log::*;
use tokio_tungstenite;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::handshake;
use hyper::{header, upgrade, StatusCode, Body, Request, Response, Server, server::conn::AddrStream};
use hyper::service::{make_service_fn, service_fn};

mod protocol;


#[tokio::main(flavor = "current_thread")]
async fn main() {
    let _ = env_logger::try_init();
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    info!("Listening on: {}", addr);

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

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

async fn handle_request(mut request: Request<Body>, remote_addr: SocketAddr) -> Result<Response<Body>, Infallible> {
    match (request.uri().path(), request.headers().contains_key(header::UPGRADE)) {
        ("/", true) => {
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

                                let (mut ws_write, mut ws_read) = ws_stream.split();

                                while let Some(data) = ws_read.next().await {
                                    let data = data.unwrap();
                                    if data.is_text() || data.is_binary() {
                                        let mut request: protocol::Request = serde_json::from_str(&data.to_string()).unwrap();

                                        let response = request.process(get_epoch_ms());

                                        let json = serde_json::to_string(&response).unwrap();
                                        ws_write.send(tungstenite::Message::Text(json)).await.unwrap();
                                    }
                                };
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

fn get_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
