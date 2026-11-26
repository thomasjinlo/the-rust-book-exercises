#![allow(dead_code)]

use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use ex08_async_line_server::{Config, Stats, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Every wait on the server is bounded by this. Real responses arrive in
/// milliseconds; the margin keeps loaded CI machines from failing the tests.
pub const WAIT: Duration = Duration::from_secs(10);

/// Fails the test instead of hanging when the body never finishes.
pub async fn watchdog<F: Future>(body: F) -> F::Output {
    timeout(Duration::from_secs(60), body)
        .await
        .expect("test body exceeded 60 s")
}

pub fn config(max_connections: usize, max_line: usize) -> Config {
    Config {
        max_connections,
        max_line,
    }
}

/// A running server bound to 127.0.0.1:0.
pub struct Server {
    pub addr: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Stats>,
}

impl Server {
    pub async fn start(config: Config) -> Server {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let (stop, signal) = oneshot::channel::<()>();
        let task = tokio::spawn(serve(listener, config, async move {
            let _ = signal.await;
        }));
        Server {
            addr,
            stop: Some(stop),
            task,
        }
    }

    /// Resolves the shutdown future without waiting for `serve` to return.
    pub fn signal_shutdown(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }

    /// True once `serve` has returned.
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    /// Waits for `serve` to return after the shutdown signal.
    pub async fn join(self) -> Stats {
        timeout(WAIT, self.task)
            .await
            .expect("serve did not return within 10 s")
            .expect("serve task panicked")
    }

    pub async fn stop(mut self) -> Stats {
        self.signal_shutdown();
        self.join().await
    }
}

pub struct Client {
    rd: OwnedReadHalf,
    wr: OwnedWriteHalf,
    pending: Vec<u8>,
}

impl Client {
    pub async fn connect(addr: SocketAddr) -> Client {
        let stream = TcpStream::connect(addr).await.expect("connect");
        stream.set_nodelay(true).expect("nodelay");
        let (rd, wr) = stream.into_split();
        Client {
            rd,
            wr,
            pending: Vec::new(),
        }
    }

    pub async fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.wr.write_all(bytes).await
    }

    /// Next line without its `\n`. `None` on EOF, reset or after `limit`.
    pub async fn try_line_within(&mut self, limit: Duration) -> Option<String> {
        let result = timeout(limit, async {
            loop {
                if let Some(i) = self.pending.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = self.pending.drain(..=i).collect();
                    return Some(String::from_utf8_lossy(&line[..i]).into_owned());
                }
                let mut buf = [0u8; 4096];
                match self.rd.read(&mut buf).await {
                    Ok(0) | Err(_) => return None,
                    Ok(n) => self.pending.extend_from_slice(&buf[..n]),
                }
            }
        })
        .await;
        result.ok().flatten()
    }

    pub async fn try_line(&mut self) -> Option<String> {
        self.try_line_within(WAIT).await
    }

    pub async fn line(&mut self) -> String {
        self.try_line()
            .await
            .expect("expected a line, got EOF, reset or timeout")
    }

    /// Asserts the server closes the connection without sending more bytes.
    pub async fn expect_eof(&mut self) {
        assert!(self.pending.is_empty(), "unread bytes: {:?}", self.pending);
        let mut buf = [0u8; 64];
        match timeout(WAIT, self.rd.read(&mut buf)).await {
            Ok(Ok(0)) => {}
            Ok(Ok(n)) => panic!("expected EOF, got {n} bytes: {:?}", &buf[..n]),
            // A reset also means the server closed.
            Ok(Err(_)) => {}
            Err(_) => panic!("server did not close the connection within 10 s"),
        }
    }
}
