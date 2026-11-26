//! A line-oriented TCP server on tokio with a connection limit.

use std::io;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;

/// Serves the line protocol on `listener` until the task running it is dropped or aborted.
/// At most `max_connections` clients are served at once.
pub async fn serve(listener: TcpListener, max_connections: usize) {
    let slots = Arc::new(Semaphore::new(max_connections));
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            continue;
        };
        match slots.clone().try_acquire_owned() {
            Ok(permit) => {
                tokio::spawn(async move {
                    let _ = handle(stream).await;
                    // Free the slot before the socket closes, so a client that sees
                    // the close and reconnects is not refused.
                    drop(permit);
                });
            }
            Err(_) => {
                tokio::spawn(async move {
                    let _ = stream.write_all(b"ERR busy\n").await;
                });
            }
        }
    }
}

async fn handle(mut stream: TcpStream) -> io::Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    loop {
        line.clear();
        reader.read_until(b'\n', &mut line).await?;
        // A partial line at EOF is never answered.
        if line.pop() != Some(b'\n') {
            return Ok(());
        }
        let mut reply = Vec::new();
        let mut quit = false;
        if line == b"PING" {
            reply.extend_from_slice(b"PONG");
        } else if line == b"QUIT" {
            reply.extend_from_slice(b"BYE");
            quit = true;
        } else if let Some(text) = line.strip_prefix(b"ECHO ") {
            reply.extend_from_slice(text);
        } else {
            reply.extend_from_slice(b"ERR unknown command");
        }
        reply.push(b'\n');
        writer.write_all(&reply).await?;
        if quit {
            return Ok(());
        }
    }
}
