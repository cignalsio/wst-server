use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{duplex, AsyncRead, AsyncWrite, DuplexStream, ReadBuf};
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Role};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

use super::handle_websocket;

const TEST_TIMEOUT: Duration = Duration::from_secs(2);

async fn connection() -> (WebSocketStream<DuplexStream>, JoinHandle<()>) {
    let (client, server) = duplex(4096);
    let server = WebSocketStream::from_raw_socket(server, Role::Server, None).await;
    let task = tokio::spawn(handle_websocket(server, "127.0.0.1:12345".parse().unwrap()));
    let client = WebSocketStream::from_raw_socket(client, Role::Client, None).await;
    (client, task)
}

async fn next_message(client: &mut WebSocketStream<DuplexStream>) -> Message {
    timeout(TEST_TIMEOUT, client.next())
        .await
        .expect("server did not respond")
        .expect("server closed before responding")
        .expect("invalid websocket response")
}

async fn assert_session_finished(task: JoinHandle<()>) {
    timeout(TEST_TIMEOUT, task)
        .await
        .expect("server did not end the session")
        .expect("server session panicked");
}

#[tokio::test]
async fn abrupt_disconnect_does_not_panic() {
    let (client, task) = connection().await;
    // Dropping the transport without sending Close reproduces
    // Protocol(ResetWithoutClosingHandshake), the production panic.
    drop(client);
    assert_session_finished(task).await;
}

#[tokio::test]
async fn text_and_binary_requests_receive_time_responses() {
    let (mut client, task) = connection().await;

    for request in [
        Message::Text(r#"{"c":12345}"#.to_owned()),
        Message::Binary(br#"{"c":12345}"#.to_vec()),
    ] {
        timeout(TEST_TIMEOUT, client.send(request))
            .await
            .expect("request send timed out")
            .unwrap();
        let response = next_message(&mut client).await;
        assert!(response.is_text());
        let response: serde_json::Value = serde_json::from_slice(&response.into_data()).unwrap();
        assert_eq!(response["c"], 12345);
        assert!(response["s"].as_u64().unwrap() > 0);
        assert!(response["e"].is_null());
        assert_eq!(response["l"], 3);
    }

    drop(client);
    assert_session_finished(task).await;
}

#[tokio::test]
async fn ping_and_close_receive_automatic_replies() {
    let (mut client, task) = connection().await;
    let ping = b"still here".to_vec();
    timeout(TEST_TIMEOUT, client.send(Message::Ping(ping.clone())))
        .await
        .expect("ping send timed out")
        .unwrap();
    assert_eq!(next_message(&mut client).await, Message::Pong(ping));

    let close = Some(CloseFrame {
        code: CloseCode::Normal,
        reason: "done".into(),
    });
    timeout(TEST_TIMEOUT, client.send(Message::Close(close.clone())))
        .await
        .expect("close send timed out")
        .unwrap();
    assert_eq!(next_message(&mut client).await, Message::Close(close));
    assert_session_finished(task).await;
    assert!(timeout(TEST_TIMEOUT, client.next())
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn malformed_requests_end_the_session_without_panicking() {
    for request in [
        Message::Text("{".to_owned()),
        Message::Text(r#"{"c":"invalid"}"#.to_owned()),
        Message::Binary(vec![0xff, 0xfe]),
    ] {
        let (mut client, task) = connection().await;
        timeout(TEST_TIMEOUT, client.send(request))
            .await
            .expect("request send timed out")
            .unwrap();
        assert_session_finished(task).await;
    }
}

struct BrokenWriteStream {
    incoming: Vec<u8>,
    offset: usize,
    write_attempts: Arc<AtomicUsize>,
}

impl AsyncRead for BrokenWriteStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let count = buf.remaining().min(self.incoming.len() - self.offset);
        buf.put_slice(&self.incoming[self.offset..self.offset + count]);
        self.offset += count;
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for BrokenWriteStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.write_attempts.fetch_add(1, Ordering::Relaxed);
        Poll::Ready(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "peer disconnected",
        )))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn disconnect_during_response_write_does_not_panic() {
    let payload = br#"{"c":42}"#;
    let mask = [1, 2, 3, 4];
    // One valid, masked client text frame; every server write then fails.
    let mut incoming = vec![0x81, 0x80 | payload.len() as u8];
    incoming.extend_from_slice(&mask);
    incoming.extend(
        payload
            .iter()
            .enumerate()
            .map(|(i, byte)| byte ^ mask[i % 4]),
    );
    let write_attempts = Arc::new(AtomicUsize::new(0));
    let stream = BrokenWriteStream {
        incoming,
        offset: 0,
        write_attempts: Arc::clone(&write_attempts),
    };
    let server = WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
    let task = tokio::spawn(handle_websocket(server, "127.0.0.1:12345".parse().unwrap()));

    assert_session_finished(task).await;
    assert!(
        write_attempts.load(Ordering::Relaxed) > 0,
        "response write was not exercised"
    );
}
