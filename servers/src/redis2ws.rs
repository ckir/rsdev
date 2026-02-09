//! # Redis to WebSocket Bridge (Placeholder/Example)
//!
//! This module provides a foundational WebSocket server and client example,
//! with the intention of serving as a bridge for data from a Redis Pub/Sub
//! channel to WebSocket clients.
//!
//! ## Current Functionality:
//! - **Echo Server**: Establishes a basic WebSocket server that echoes
//!   received text or binary messages back to the sender.
//! - **Self-Connecting Client**: Includes a WebSocket client that connects
//!   to the local echo server, sends a message, and asserts the echo.
//!
//! ## Intended Future Use:
//! The name `redis2ws.rs` implies its future purpose: to subscribe to messages
//! from a Redis Pub/Sub channel and broadcast them to connected WebSocket clients.
//! The current implementation serves as a minimal setup for WebSocket
//! communication.

use futures_util::{SinkExt, StreamExt};
use http::Uri;
use tokio::net::TcpListener;
use tokio_websockets::{ClientBuilder, Error, Message, ServerBuilder};

#[tokio::main]
/// Main entry point for the Redis-to-WebSocket example server and client.
///
/// This function sets up a local WebSocket echo server, connects a client to it,
/// sends a "Hello world!" message, and verifies that the message is echoed back.
/// It demonstrates basic WebSocket server and client functionality using `tokio-websockets`.
async fn main() -> Result<(), Error> {
  /// Binds the TCP listener for the WebSocket server on port 3000.
  let listener = TcpListener::bind("127.0.0.1:3000").await?;

  /// Spawns an asynchronous task to handle incoming WebSocket connections.
  tokio::spawn(async move {
    /// Accepts new TCP connections and upgrades them to WebSocket connections.
    while let Ok((stream, _)) = listener.accept().await {
      let (_request, mut ws_stream) = ServerBuilder::new()
        .accept(stream)
        .await?;

      /// Spawns a new task for each client to handle message echoing.
      tokio::spawn(async move {
        // Just an echo server, really
        while let Some(Ok(msg)) = ws_stream.next().await {
          if msg.is_text() || msg.is_binary() {
            ws_stream.send(msg).await?;
          }
        }

        Ok::<_, Error>(())
      });
    }

    Ok::<_, Error>(())
  });

  /// Defines the URI for the local WebSocket server.
  let uri = Uri::from_static("ws://127.0.0.1:3000");
  /// Connects a WebSocket client to the server.
  let (mut client, _) = ClientBuilder::from_uri(uri).connect().await?;

  /// Sends a "Hello world!" text message from the client to the server.
  client.send(Message::text("Hello world!")).await?;

  /// Continuously receives messages from the server until the client disconnects.
  while let Some(Ok(msg)) = client.next().await {
    if let Some(text) = msg.as_text() {
      /// Asserts that the received message is the expected "Hello world!" echo.
      assert_eq!(text, "Hello world!");
      // We got one message, just stop now
      /// Closes the client connection after receiving and verifying the echo.
      client.close().await?;
    }
  }

  Ok(())
}