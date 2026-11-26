//! A line-oriented TCP server on tokio with a connection limit, an idle
//! timeout, a bounded line length and graceful shutdown.

use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout};

/// Server limits. All fields are read once when `serve` starts.
#[derive(Debug, Clone)]
pub struct Config {
    /// Connections served at the same time. A client that connects while this
    /// many are active is rejected with `ERR busy\n`.
    pub max_connections: usize,
    /// Longest accepted line in bytes, excluding the `\n` and one optional
    /// trailing `\r`.
    pub max_line: usize,
    /// A connection that receives no bytes for this long is closed.
    pub idle_timeout: Duration,
    /// After shutdown starts, connections get this long before they are dropped.
    pub shutdown_grace: Duration,
}

/// Counters returned by [`serve`] after it has stopped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    /// Connections admitted (not rejected as busy).
    pub accepted: u64,
    /// Connections rejected with `ERR busy`.
    pub rejected_busy: u64,
    /// Connections closed with `ERR idle timeout`.
    pub closed_idle: u64,
    /// Connections closed with `ERR line too long`.
    pub closed_too_long: u64,
    /// Complete lines within `max_line` that were processed, including
    /// unknown commands and `QUIT`.
    pub commands: u64,
}

#[derive(Default)]
struct Counters {
    accepted: AtomicU64,
    rejected_busy: AtomicU64,
    closed_idle: AtomicU64,
    closed_too_long: AtomicU64,
    commands: AtomicU64,
}

impl Counters {
    fn snapshot(&self) -> Stats {
        Stats {
            accepted: self.accepted.load(Relaxed),
            rejected_busy: self.rejected_busy.load(Relaxed),
            closed_idle: self.closed_idle.load(Relaxed),
            closed_too_long: self.closed_too_long.load(Relaxed),
            commands: self.commands.load(Relaxed),
        }
    }
}

/// Size of the socket read buffer; also the most that one read can add.
const READ_CHUNK: usize = 4096;
/// Pause after a failed `accept` (for example out of file descriptors).
const ACCEPT_BACKOFF: Duration = Duration::from_millis(10);
/// Upper bound for delivering `ERR busy` to a client that never reads.
const BUSY_WRITE_LIMIT: Duration = Duration::from_secs(1);

enum Line {
    Complete,
    Eof,
    TooLong,
    Idle,
}

/// Reads one line into `line` (without terminator). `line` never holds more
/// than `max_line + 2` bytes: the content plus an optional `\r` and the `\n`.
async fn read_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    line: &mut Vec<u8>,
    max_line: usize,
    idle: Duration,
) -> io::Result<Line> {
    line.clear();
    loop {
        let chunk = match timeout(idle, reader.fill_buf()).await {
            Err(_) => return Ok(Line::Idle),
            Ok(chunk) => chunk?,
        };
        // A partial line at EOF is discarded: it was never a command.
        if chunk.is_empty() {
            return Ok(Line::Eof);
        }
        let newline = chunk.iter().position(|&b| b == b'\n');
        let take = newline.map_or(chunk.len(), |i| i + 1);
        // Without a newline the line still needs one byte, so it can hold at
        // most max_line + 1 bytes (content + `\r`).
        let limit = max_line.saturating_add(if newline.is_some() { 2 } else { 1 });
        if line.len() + take > limit {
            return Ok(Line::TooLong);
        }
        line.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if newline.is_some() {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            // `\n` without `\r` and max_line + 1 content bytes passes the
            // check above and is rejected here.
            if line.len() > max_line {
                return Ok(Line::TooLong);
            }
            return Ok(Line::Complete);
        }
    }
}

async fn handle(stream: &mut TcpStream, config: &Config, counters: &Counters) -> io::Result<()> {
    stream.set_nodelay(true)?;
    let (read_half, mut writer) = stream.split();
    let mut reader = BufReader::with_capacity(READ_CHUNK, read_half);
    let mut line = Vec::with_capacity(READ_CHUNK.min(config.max_line.saturating_add(2)));
    let mut reply = Vec::new();
    loop {
        match read_line(&mut reader, &mut line, config.max_line, config.idle_timeout).await? {
            Line::Complete => {}
            Line::Eof => return Ok(()),
            Line::Idle => {
                counters.closed_idle.fetch_add(1, Relaxed);
                return writer.write_all(b"ERR idle timeout\n").await;
            }
            Line::TooLong => {
                counters.closed_too_long.fetch_add(1, Relaxed);
                return writer.write_all(b"ERR line too long\n").await;
            }
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

/// Serves the line protocol on `listener` until `shutdown` resolves, then
/// drains connections and returns the final counters.
pub async fn serve(
    listener: TcpListener,
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Stats {
    let config = Arc::new(config);
    let counters = Arc::new(Counters::default());
    let slots = Arc::new(Semaphore::new(config.max_connections));
    // Holds every spawned task so shutdown can wait for, or abort, all of them.
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
                        let config = config.clone();
                        let counters = counters.clone();
                        tasks.spawn(async move {
                            let _ = handle(&mut stream, &config, &counters).await;
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
    let drain = async { while tasks.join_next().await.is_some() {} };
    if timeout(config.shutdown_grace, drain).await.is_err() {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
    counters.snapshot()
}
