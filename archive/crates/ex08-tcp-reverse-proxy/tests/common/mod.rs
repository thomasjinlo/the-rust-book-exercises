//! Shared helpers for the given tests. Do not edit.
#![allow(dead_code)]

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ex08_tcp_reverse_proxy::{Connector, Proxy, ProxyConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

/// Fails the test instead of hanging when `fut` does not finish. Every test body runs inside it.
pub async fn within<F: Future>(secs: u64, fut: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(secs), fut)
        .await
        .expect("test body exceeded its watchdog")
}

/// Polls `cond` every 5 ms until it holds. Fails after `secs` seconds.
pub async fn wait_until(secs: u64, what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs);
    while !cond() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

pub async fn bind() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").await.unwrap()
}

/// A port that refuses connections: bound, then dropped.
pub async fn refused_addr() -> SocketAddr {
    let l = bind().await;
    l.local_addr().unwrap()
}

pub async fn start_proxy(config: ProxyConfig) -> Proxy {
    Proxy::start(bind().await, config)
}

pub async fn start_proxy_with<C: Connector>(config: ProxyConfig, connector: C) -> Proxy {
    Proxy::start_with_connector(bind().await, config, connector)
}

/// Waits until `completed + no_backend_available >= n`, i.e. `n` connections have fully ended.
pub async fn wait_finished(proxy: &Proxy, n: u64) {
    wait_until(30, "connections to finish", || {
        let s = proxy.stats();
        s.completed + s.no_backend_available >= n
    })
    .await;
}

pub struct TestBackend {
    pub addr: SocketAddr,
    /// Connections accepted so far.
    pub accepted: Arc<AtomicUsize>,
}

impl TestBackend {
    pub fn accepted(&self) -> usize {
        self.accepted.load(SeqCst)
    }
}

/// Echoes every byte; on EOF it shuts down its write side.
pub async fn spawn_echo() -> TestBackend {
    let listener = bind().await;
    let addr = listener.local_addr().unwrap();
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                return;
            };
            count.fetch_add(1, SeqCst);
            tokio::spawn(async move {
                let (mut r, mut w) = s.split();
                let _ = tokio::io::copy(&mut r, &mut w).await;
                let _ = w.shutdown().await;
            });
        }
    });
    TestBackend { addr, accepted }
}

/// Writes `greeting`, then closes without reading.
pub async fn spawn_greeter(greeting: &'static [u8]) -> TestBackend {
    let listener = bind().await;
    let addr = listener.local_addr().unwrap();
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                return;
            };
            count.fetch_add(1, SeqCst);
            let _ = s.write_all(greeting).await;
        }
    });
    TestBackend { addr, accepted }
}

/// Accepts connections and keeps them open without ever reading or writing.
pub async fn spawn_silent() -> TestBackend {
    let listener = bind().await;
    let addr = listener.local_addr().unwrap();
    let accepted = Arc::new(AtomicUsize::new(0));
    let count = accepted.clone();
    tokio::spawn(async move {
        let mut held = Vec::new();
        loop {
            let Ok((s, _)) = listener.accept().await else {
                return;
            };
            count.fetch_add(1, SeqCst);
            held.push(s);
        }
    });
    TestBackend { addr, accepted }
}

/// Connects, sends `payload`, half-closes, and returns everything the peer sends back.
pub async fn roundtrip(addr: SocketAddr, payload: &[u8]) -> Vec<u8> {
    let mut c = TcpStream::connect(addr).await.unwrap();
    let (mut r, mut w) = c.split();
    let write = async {
        w.write_all(payload).await.unwrap();
        w.shutdown().await.unwrap();
    };
    let read = async {
        let mut out = Vec::new();
        r.read_to_end(&mut out).await.unwrap();
        out
    };
    tokio::join!(write, read).1
}

/// How a `Fake` connector treats one backend address.
#[derive(Clone)]
pub enum Mode {
    /// Connects for real.
    Pass,
    /// Fails immediately with `ConnectionRefused`.
    Refuse,
    /// Never completes.
    Hang,
    /// Sleeps, then connects for real.
    Delay(Duration),
    /// Waits for `notify_one`, then fails with `ConnectionRefused`.
    GatedRefuse(Arc<Notify>),
    /// Waits for `notify_one`, then connects for real.
    GatedPass(Arc<Notify>),
}

struct FakeInner {
    modes: Mutex<HashMap<SocketAddr, Mode>>,
    log: Mutex<Vec<SocketAddr>>,
    probes: Mutex<Vec<std::net::TcpStream>>,
}

/// A connector that records every attempt and can be told how to treat each address.
#[derive(Clone)]
pub struct Fake(Arc<FakeInner>);

impl Fake {
    pub fn new() -> Self {
        Fake(Arc::new(FakeInner {
            modes: Mutex::default(),
            log: Mutex::default(),
            probes: Mutex::default(),
        }))
    }

    pub fn set(&self, addr: SocketAddr, mode: Mode) {
        self.0.modes.lock().unwrap().insert(addr, mode);
    }

    /// Every connect attempt so far, in call order.
    pub fn log(&self) -> Vec<SocketAddr> {
        self.0.log.lock().unwrap().clone()
    }

    pub fn attempts(&self, addr: SocketAddr) -> usize {
        self.log().iter().filter(|a| **a == addr).count()
    }

    /// Duplicated handles of every backend socket handed to the proxy. Socket options are
    /// shared between duplicates, so a test can read what the proxy set.
    pub fn probes(&self) -> Vec<std::net::TcpStream> {
        self.0
            .probes
            .lock()
            .unwrap()
            .iter()
            .map(|s| s.try_clone().unwrap())
            .collect()
    }

    async fn real(&self, addr: SocketAddr) -> io::Result<TcpStream> {
        let stream = TcpStream::connect(addr).await?;
        let std = stream.into_std()?;
        self.0.probes.lock().unwrap().push(std.try_clone()?);
        TcpStream::from_std(std)
    }
}

fn refused() -> io::Error {
    io::Error::from(io::ErrorKind::ConnectionRefused)
}

impl Connector for Fake {
    async fn connect(&self, addr: SocketAddr) -> io::Result<TcpStream> {
        self.0.log.lock().unwrap().push(addr);
        let mode = self
            .0
            .modes
            .lock()
            .unwrap()
            .get(&addr)
            .cloned()
            .unwrap_or(Mode::Pass);
        match mode {
            Mode::Pass => self.real(addr).await,
            Mode::Refuse => Err(refused()),
            Mode::Hang => std::future::pending().await,
            Mode::Delay(d) => {
                tokio::time::sleep(d).await;
                self.real(addr).await
            }
            Mode::GatedRefuse(gate) => {
                gate.notified().await;
                Err(refused())
            }
            Mode::GatedPass(gate) => {
                gate.notified().await;
                self.real(addr).await
            }
        }
    }
}

/// A seeded generator (splitmix64) so every run sees the same data.
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform enough for tests: `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    pub fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
}

/// FNV-1a over a byte stream. Order-sensitive, so it detects loss, duplication and reordering.
#[derive(Clone, Copy)]
pub struct Fnv(pub u64);

impl Fnv {
    pub fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    pub fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// The hash of the `len`-byte stream that `Rng(seed)` produces in `chunk`-sized pieces.
pub fn expected_hash(seed: u64, len: usize, chunk: usize) -> u64 {
    let mut rng = Rng(seed);
    let mut h = Fnv::new();
    let mut buf = vec![0u8; chunk];
    let mut left = len;
    while left > 0 {
        let n = left.min(chunk);
        rng.fill(&mut buf[..n]);
        h.update(&buf[..n]);
        left -= n;
    }
    h.0
}
