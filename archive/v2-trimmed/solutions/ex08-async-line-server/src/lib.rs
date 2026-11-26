//! A line-oriented TCP server on tokio with a connection limit and a bounded
//! line length.

use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout};

/// Server limits. Both fields are read once when `serve` starts.
#[derive(Debug, Clone)]
pub struct Config {
    /// Connections served at the same time. A client that connects while this
    /// many are active is rejected with `ERR busy\n`.
    pub max_connections: usize,
    /// Longest accepted line in bytes, excluding the `\n` and one optional
    /// trailing `\r`.
    pub max_line: usize,
}

/// Counters returned by [`serve`] after it has stopped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Connections admitted (not rejected as busy).
    pub accepted: u64,
    /// Connections rejected with `ERR busy`.
    pub rejected_busy: u64,
    /// Complete lines within `max_line` that were processed, including
    /// unknown commands and `QUIT`.
    pub commands: u64,
}

#[derive(Default)]
struct Counters {
    accepted: AtomicU64,
    rejected_busy: AtomicU64,
    commands: AtomicU64,
}

impl Counters {
    fn snapshot(&self) -> Stats {
        Stats {
            accepted: self.accepted.load(Relaxed),
            rejected_busy: self.rejected_busy.load(Relaxed),
            commands: self.commands.load(Relaxed),
        }
    }
}

/// Pause after a failed `accept` (for example out of file descriptors).
const ACCEPT_BACKOFF: Duration = Duration::from_millis(10);
/// Upper bound for delivering `ERR busy` to a client that never reads.
const BUSY_WRITE_LIMIT: Duration = Duration::from_secs(1);

enum Line {
    Complete,
    Eof,
    TooLong,
}

/// Reads one line into `line`, without its terminator.
async fn read_line<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    line: &mut Vec<u8>,
    max_line: usize,
) -> io::Result<Line> {
    line.clear();
    // Content, an optional `\r` and the `\n` fit in max_line + 2 bytes. If
    // `take` runs out first, the line is too long whatever follows.
    let limit = max_line.saturating_add(2) as u64;
    let n = reader.take(limit).read_until(b'\n', line).await?;
    if line.last() != Some(&b'\n') {
        // A partial line at EOF was never a command.
        return Ok(if n as u64 == limit {
            Line::TooLong
        } else {
            Line::Eof
        });
    }
    line.pop();
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    // Content of max_line + 1 bytes ending in `\n` alone also fits `limit`.
    Ok(if line.len() > max_line {
        Line::TooLong
    } else {
        Line::Complete
    })
}

async fn handle(stream: &mut TcpStream, max_line: usize, counters: &Counters) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let (read_half, mut writer) = stream.split();
    let mut reader = BufReader::new(read_half);
    let mut line = Vec::new();
    let mut reply = Vec::new();
    loop {
        match read_line(&mut reader, &mut line, max_line).await? {
            Line::Complete => {}
            Line::Eof => return Ok(()),
            Line::TooLong => return writer.write_all(b"ERR line too long\n").await,
        }
        counters.commands.fetch_add(1, Relaxed);
        reply.clear();
        let mut quit = false;
        if line == b"PING" {
            reply.extend_from_slice(b"PONG\n");
        } else if line == b"QUIT" {
            reply.extend_from_slice(b"BYE\n");
            quit = true;
        } else if let Some(text) = line.strip_prefix(b"ECHO ") {
            reply.extend_from_slice(text);
            reply.push(b'\n');
        } else {
            reply.extend_from_slice(b"ERR unknown command\n");
        }
        writer.write_all(&reply).await?;
        if quit {
            return Ok(());
        }
    }
}

/// Serves the line protocol on `listener` until `shutdown` resolves. Then it
/// stops accepting, waits until every open connection has ended, and returns
/// the final counters.
pub async fn serve(
    listener: TcpListener,
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Stats {
    let counters = Arc::new(Counters::default());
    let slots = Arc::new(Semaphore::new(config.max_connections));
    let max_line = config.max_line;
    let mut tasks: JoinSet<()> = JoinSet::new();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            // Reaps finished tasks so the set does not grow with total connections.
            _ = tasks.join_next(), if !tasks.is_empty() => {}
            accepted = listener.accept() => {
                let Ok((mut stream, _)) = accepted else {
                    sleep(ACCEPT_BACKOFF).await;
                    continue;
                };
                match slots.clone().try_acquire_owned() {
                    Ok(permit) => {
                        counters.accepted.fetch_add(1, Relaxed);
                        let counters = counters.clone();
                        tasks.spawn(async move {
                            let _ = handle(&mut stream, max_line, &counters).await;
                            // Free the slot before the socket closes, so a client
                            // that sees the close and reconnects is not refused.
                            drop(permit);
                            drop(stream);
                        });
                    }
                    Err(_) => {
                        counters.rejected_busy.fetch_add(1, Relaxed);
                        tasks.spawn(async move {
                            let _ = timeout(BUSY_WRITE_LIMIT, stream.write_all(b"ERR busy\n")).await;
                        });
                    }
                }
            }
        }
    }

    drop(listener);
    while tasks.join_next().await.is_some() {}
    counters.snapshot()
}
