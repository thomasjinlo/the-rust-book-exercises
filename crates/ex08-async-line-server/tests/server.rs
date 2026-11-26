use std::net::SocketAddr;
use std::time::{Duration, Instant};

use ex08_async_line_server::serve;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

const WAIT: Duration = Duration::from_secs(5);

/// Runs the body with a time limit so a server that never answers fails the test.
async fn watchdog<F: std::future::Future>(body: F) -> F::Output {
    timeout(Duration::from_secs(20), body)
        .await
        .expect("test took more than 20 s")
}

/// A server on 127.0.0.1:0, aborted when dropped.
struct Server {
    addr: SocketAddr,
    task: JoinHandle<()>,
}

impl Server {
    async fn start(max_connections: usize) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(serve(listener, max_connections));
        Server { addr, task }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Client {
    stream: TcpStream,
    pending: Vec<u8>,
}

impl Client {
    async fn connect(addr: SocketAddr) -> Client {
        let stream = TcpStream::connect(addr).await.unwrap();
        stream.set_nodelay(true).unwrap();
        Client {
            stream,
            pending: Vec::new(),
        }
    }

    async fn send(&mut self, bytes: &[u8]) {
        let _ = self.stream.write_all(bytes).await;
    }

    /// Next line without its `\n`, or `None` on close, reset or after 5 s.
    async fn try_line(&mut self) -> Option<String> {
        timeout(WAIT, async {
            loop {
                if let Some(i) = self.pending.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = self.pending.drain(..=i).collect();
                    return Some(String::from_utf8_lossy(&line[..i]).into_owned());
                }
                let mut buf = [0u8; 4096];
                match self.stream.read(&mut buf).await {
                    Ok(0) | Err(_) => return None,
                    Ok(n) => self.pending.extend_from_slice(&buf[..n]),
                }
            }
        })
        .await
        .ok()
        .flatten()
    }

    async fn line(&mut self) -> String {
        self.try_line().await.expect("expected a line")
    }

    async fn expect_closed(&mut self) {
        let mut buf = [0u8; 16];
        match timeout(WAIT, self.stream.read(&mut buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => {}
            other => panic!("expected the server to close, got {other:?}"),
        }
    }

    async fn ask(&mut self, request: &str) -> String {
        self.send(request.as_bytes()).await;
        self.line().await
    }
}

#[tokio::test]
async fn ping_gets_pong() {
    watchdog(async {
        let server = Server::start(4).await;
        let mut c = Client::connect(server.addr).await;
        assert_eq!(c.ask("PING\n").await, "PONG");
        assert_eq!(c.ask("PING\n").await, "PONG");
    })
    .await;
}

#[tokio::test]
async fn echo_returns_the_text() {
    watchdog(async {
        let server = Server::start(4).await;
        let mut c = Client::connect(server.addr).await;
        assert_eq!(c.ask("ECHO hello world\n").await, "hello world");
        assert_eq!(c.ask("ECHO \n").await, "");
    })
    .await;
}

#[tokio::test]
async fn anything_else_is_an_unknown_command() {
    watchdog(async {
        let server = Server::start(4).await;
        let mut c = Client::connect(server.addr).await;
        for request in ["ping\n", "FOO\n", "ECHO\n", "PING \n", "\n"] {
            assert_eq!(c.ask(request).await, "ERR unknown command", "{request:?}");
        }
        assert_eq!(c.ask("PING\n").await, "PONG", "the connection stays open");
    })
    .await;
}

#[tokio::test]
async fn quit_says_bye_and_closes() {
    watchdog(async {
        let server = Server::start(4).await;
        let mut c = Client::connect(server.addr).await;
        assert_eq!(c.ask("QUIT\n").await, "BYE");
        c.expect_closed().await;
    })
    .await;
}

#[tokio::test]
async fn a_request_split_across_writes_is_answered_once_complete() {
    watchdog(async {
        let server = Server::start(4).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"EC").await;
        sleep(Duration::from_millis(50)).await;
        c.send(b"HO sp").await;
        sleep(Duration::from_millis(50)).await;
        c.send(b"lit\n").await;
        assert_eq!(c.line().await, "split");
    })
    .await;
}

#[tokio::test]
async fn requests_in_one_write_are_answered_in_order() {
    watchdog(async {
        let server = Server::start(4).await;
        let mut c = Client::connect(server.addr).await;
        c.send(b"PING\nECHO hi\nFOO\nQUIT\nPING\n").await;
        for expected in ["PONG", "hi", "ERR unknown command", "BYE"] {
            assert_eq!(c.line().await, expected);
        }
        c.expect_closed().await;
    })
    .await;
}

/// Random requests sent in random pieces get the same replies as a plain model.
#[tokio::test]
async fn random_requests_in_random_pieces_match_a_model() {
    watchdog(async {
        let mut rng = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let server = Server::start(4).await;
        for _ in 0..20 {
            let mut c = Client::connect(server.addr).await;
            let mut bytes = Vec::new();
            let mut expected = Vec::new();
            for _ in 0..30 {
                match next() % 4 {
                    0 => {
                        bytes.extend_from_slice(b"PING\n");
                        expected.push("PONG".to_string());
                    }
                    1 => {
                        let text = format!("t{}", next() % 1000);
                        bytes.extend_from_slice(format!("ECHO {text}\n").as_bytes());
                        expected.push(text);
                    }
                    2 => {
                        bytes.extend_from_slice(b"NOPE\n");
                        expected.push("ERR unknown command".to_string());
                    }
                    _ => {
                        bytes.extend_from_slice(b"\n");
                        expected.push("ERR unknown command".to_string());
                    }
                }
            }
            let mut rest = &bytes[..];
            while !rest.is_empty() {
                let n = (1 + next() as usize % 9).min(rest.len());
                c.send(&rest[..n]).await;
                rest = &rest[n..];
            }
            for want in &expected {
                assert_eq!(&c.line().await, want);
            }
        }
    })
    .await;
}

#[tokio::test]
async fn clients_are_served_at_the_same_time() {
    watchdog(async {
        let server = Server::start(4).await;
        let mut a = Client::connect(server.addr).await;
        let mut b = Client::connect(server.addr).await;
        a.send(b"EC").await;
        assert_eq!(
            b.ask("PING\n").await,
            "PONG",
            "a half request blocks no one"
        );
        assert_eq!(a.ask("HO x\n").await, "x");
    })
    .await;
}

/// With 2 slots, a third client gets `ERR busy` and a close, and a slot frees when a client leaves.
#[tokio::test]
async fn a_third_client_is_busy_until_a_slot_frees() {
    watchdog(async {
        let server = Server::start(2).await;
        let mut a = Client::connect(server.addr).await;
        let mut b = Client::connect(server.addr).await;
        // A reply proves the connection is admitted and holds a slot.
        assert_eq!(a.ask("PING\n").await, "PONG");
        assert_eq!(b.ask("PING\n").await, "PONG");

        // The client sends nothing: the refusal must not wait for a request.
        let mut c = Client::connect(server.addr).await;
        assert_eq!(c.line().await, "ERR busy");
        c.expect_closed().await;

        drop(a);
        // The server sees the close a moment later, so retry for up to 5 s.
        // A correct server frees the slot in under 10 ms in a debug build.
        let deadline = Instant::now() + WAIT;
        loop {
            let mut d = Client::connect(server.addr).await;
            d.send(b"PING\n").await;
            if d.try_line().await.as_deref() == Some("PONG") {
                break;
            }
            assert!(Instant::now() < deadline, "no slot was freed");
            sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(
            b.ask("PING\n").await,
            "PONG",
            "the other client is unharmed"
        );
    })
    .await;
}
