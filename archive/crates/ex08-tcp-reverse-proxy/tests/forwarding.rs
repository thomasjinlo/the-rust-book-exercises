mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use common::*;
use ex08_tcp_reverse_proxy::ProxyConfig;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Notify;

fn config(backends: Vec<SocketAddr>) -> ProxyConfig {
    ProxyConfig {
        backends,
        connect_timeout: Duration::from_secs(5),
        eject_after_failures: 3,
        eject_cooldown: Duration::from_secs(60),
    }
}

#[tokio::test]
async fn client_half_close_reaches_the_backend_which_can_still_reply() {
    within(60, async {
        let listener = bind().await;
        let backend_addr = listener.local_addr().unwrap();
        // Reads the whole request (needs the client's EOF), then replies.
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut req = Vec::new();
            s.read_to_end(&mut req).await.unwrap();
            s.write_all(format!("got {} bytes", req.len()).as_bytes())
                .await
                .unwrap();
        });
        let proxy = start_proxy(config(vec![backend_addr])).await;

        let reply = roundtrip(proxy.local_addr(), &[7u8; 1000]).await;
        assert_eq!(reply, b"got 1000 bytes");
        wait_finished(&proxy, 1).await;
        let s = proxy.stats();
        assert_eq!(
            (s.bytes_client_to_backend, s.bytes_backend_to_client),
            (1000, 14)
        );
    })
    .await;
}

#[tokio::test]
async fn backend_half_close_reaches_the_client_which_can_still_send() {
    within(60, async {
        let listener = bind().await;
        let backend_addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();
        // Sends a greeting and shuts down only its write side, then reads what the client sends.
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            s.write_all(b"hello").await.unwrap();
            s.shutdown().await.unwrap();
            let mut rest = Vec::new();
            s.read_to_end(&mut rest).await.unwrap();
            tx.send(rest).unwrap();
        });
        let proxy = start_proxy(config(vec![backend_addr])).await;

        let mut c = TcpStream::connect(proxy.local_addr()).await.unwrap();
        let mut greeting = [0u8; 5];
        c.read_exact(&mut greeting).await.unwrap();
        assert_eq!(&greeting, b"hello");
        // The proxy forwards the backend's EOF while the client direction stays open.
        let mut eof = Vec::new();
        c.read_to_end(&mut eof).await.unwrap();
        assert!(eof.is_empty());
        c.write_all(b"after eof").await.unwrap();
        c.shutdown().await.unwrap();
        assert_eq!(rx.await.unwrap(), b"after eof");
    })
    .await;
}

#[tokio::test]
async fn greeting_then_close_backend_is_delivered_in_full() {
    within(60, async {
        let g = spawn_greeter(b"220 ready\r\n").await;
        let proxy = start_proxy(config(vec![g.addr])).await;
        for i in 0..5 {
            assert_eq!(roundtrip(proxy.local_addr(), b"").await, b"220 ready\r\n");
            wait_finished(&proxy, i + 1).await;
        }
        assert_eq!(proxy.stats().bytes_backend_to_client, 5 * 11);
    })
    .await;
}

#[tokio::test]
async fn byte_counters_are_exact() {
    within(60, async {
        let echo = spawn_echo().await;
        let proxy = start_proxy(config(vec![echo.addr])).await;
        let sizes = [0usize, 1, 2, 1023, 1024, 65_535, 65_536, 65_537, 1_000_000];
        for (i, &n) in sizes.iter().enumerate() {
            let payload = vec![0xA5u8; n];
            assert_eq!(roundtrip(proxy.local_addr(), &payload).await, payload);
            wait_finished(&proxy, i as u64 + 1).await;
        }
        let total: u64 = sizes.iter().map(|&n| n as u64).sum();
        let s = proxy.stats();
        assert_eq!(s.bytes_client_to_backend, total);
        assert_eq!(s.bytes_backend_to_client, total);
        assert_eq!(s.completed, sizes.len() as u64);
    })
    .await;
}

#[tokio::test]
async fn randomized_payloads_and_write_splits_round_trip_exactly() {
    within(120, async {
        let echo = spawn_echo().await;
        let proxy = start_proxy(config(vec![echo.addr])).await;
        let mut rng = Rng(0x0808);
        let mut total = 0u64;
        for i in 0..60u64 {
            let len = rng.below(200_000) as usize;
            let mut payload = vec![0u8; len];
            rng.fill(&mut payload);
            let mut c = TcpStream::connect(proxy.local_addr()).await.unwrap();
            let (mut r, mut w) = c.split();
            // Random write sizes exercise partial reads and writes in the proxy.
            let mut splits = Vec::new();
            let mut pos = 0;
            while pos < len {
                let n = (1 + rng.below(30_000) as usize).min(len - pos);
                splits.push(n);
                pos += n;
            }
            let send = async {
                let mut pos = 0;
                for n in splits {
                    w.write_all(&payload[pos..pos + n]).await.unwrap();
                    pos += n;
                }
                w.shutdown().await.unwrap();
            };
            let recv = async {
                let mut out = Vec::new();
                r.read_to_end(&mut out).await.unwrap();
                out
            };
            let (_, out) = tokio::join!(send, recv);
            assert!(out == payload, "connection {i}: echoed bytes differ");
            total += len as u64;
            wait_finished(&proxy, i + 1).await;
        }
        let s = proxy.stats();
        assert_eq!(
            (s.bytes_client_to_backend, s.bytes_backend_to_client),
            (total, total)
        );
    })
    .await;
}

#[tokio::test]
async fn tcp_nodelay_is_set_on_the_backend_socket() {
    within(60, async {
        let echo = spawn_echo().await;
        let fake = Fake::new();
        let proxy = start_proxy_with(config(vec![echo.addr]), fake.clone()).await;
        assert_eq!(roundtrip(proxy.local_addr(), b"n").await, b"n");
        let probes = fake.probes();
        assert_eq!(probes.len(), 1);
        assert!(probes[0].nodelay().unwrap());
    })
    .await;
}

/// A client that writes before any backend is reachable must see its bytes left unread.
/// Closing a socket with unread receive data makes the kernel send RST; after a read
/// the close is a plain FIN and the client would see EOF.
#[tokio::test]
async fn client_bytes_are_not_consumed_before_a_backend_is_connected() {
    within(60, async {
        let dead = refused_addr().await;
        let fake = Fake::new();
        let gate = Arc::new(Notify::new());
        fake.set(dead, Mode::GatedRefuse(gate.clone()));
        let proxy = start_proxy_with(config(vec![dead]), fake.clone()).await;

        let mut c = TcpStream::connect(proxy.local_addr()).await.unwrap();
        wait_until(10, "the connect attempt", || fake.attempts(dead) == 1).await;
        c.write_all(b"unread payload").await.unwrap();
        // Loopback delivery completes inside the write call; the pause covers scheduling only.
        tokio::time::sleep(Duration::from_millis(200)).await;
        gate.notify_one();

        let mut buf = [0u8; 64];
        match c.read(&mut buf).await {
            Err(e) => assert!(
                matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                ),
                "unexpected error {e}"
            ),
            Ok(n) => panic!("expected a reset (unread data), got a clean read of {n} bytes"),
        }
        assert_eq!(proxy.stats().no_backend_available, 1);
    })
    .await;
}

const LEN: usize = 64 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;

/// Sends the seeded stream, then half-closes.
async fn send_stream(mut w: impl AsyncWriteExt + Unpin, seed: u64) {
    let mut rng = Rng(seed);
    let mut buf = vec![0u8; CHUNK];
    for _ in 0..LEN / CHUNK {
        rng.fill(&mut buf);
        w.write_all(&buf).await.unwrap();
    }
    w.shutdown().await.unwrap();
}

/// Reads to EOF and returns (byte count, hash).
async fn receive_stream(mut r: impl AsyncReadExt + Unpin) -> (usize, u64) {
    let mut h = Fnv::new();
    let mut n = 0;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let got = r.read(&mut buf).await.unwrap();
        if got == 0 {
            return (n, h.0);
        }
        h.update(&buf[..got]);
        n += got;
    }
}

/// 64 MiB each way, at the same time, hashed end to end.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sixty_four_mib_both_directions_at_once_keep_their_checksums() {
    const UP_SEED: u64 = 1;
    const DOWN_SEED: u64 = 2;
    within(120, async {
        let listener = bind().await;
        let backend_addr = listener.local_addr().unwrap();
        let backend = tokio::spawn(async move {
            let (s, _) = listener.accept().await.unwrap();
            let (r, w) = s.into_split();
            let (_, got) = tokio::join!(send_stream(w, DOWN_SEED), receive_stream(r));
            got
        });
        let proxy = start_proxy(config(vec![backend_addr])).await;

        let c = TcpStream::connect(proxy.local_addr()).await.unwrap();
        let (r, w) = c.into_split();
        let (_, down) = tokio::join!(send_stream(w, UP_SEED), receive_stream(r));
        let up = backend.await.unwrap();

        assert_eq!(
            up,
            (LEN, expected_hash(UP_SEED, LEN, CHUNK)),
            "client to backend"
        );
        assert_eq!(
            down,
            (LEN, expected_hash(DOWN_SEED, LEN, CHUNK)),
            "backend to client"
        );
        wait_finished(&proxy, 1).await;
        let s = proxy.stats();
        assert_eq!(s.bytes_client_to_backend, LEN as u64);
        assert_eq!(s.bytes_backend_to_client, LEN as u64);
    })
    .await;
}
