//! A layer-4 TCP reverse proxy with round-robin balancing and passive health checking.
//!
//! Reference solution.

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::{Instant, sleep, timeout};

/// How long `Proxy::shutdown` lets in-flight connections finish before aborting them.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Opens outbound connections to backends. Tests replace it to inject delays and failures.
pub trait Connector: Send + Sync + 'static {
    /// Opens a TCP connection to `addr`. The proxy applies `connect_timeout` itself by
    /// dropping the returned future, so implementations must be cancel-safe.
    fn connect(&self, addr: SocketAddr) -> impl Future<Output = io::Result<TcpStream>> + Send;
}

/// Connects with `TcpStream::connect`. Complete; do not edit.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioConnector;

impl Connector for TokioConnector {
    async fn connect(&self, addr: SocketAddr) -> io::Result<TcpStream> {
        TcpStream::connect(addr).await
    }
}

/// Proxy settings. See the requirements in the exercise document for exact semantics.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Backend addresses in round-robin order. Index 0 serves the first accepted connection.
    pub backends: Vec<SocketAddr>,
    /// Upper bound for one connect attempt to one backend. Expiry counts as a failure.
    pub connect_timeout: Duration,
    /// Consecutive failures that eject a backend. A value of 0 behaves like 1.
    pub eject_after_failures: u32,
    /// How long an ejected backend is skipped before one trial connection is allowed.
    pub eject_cooldown: Duration,
}

impl ProxyConfig {
    /// Defaults: 3 s connect timeout, eject after 3 failures, 10 s cooldown. Complete; do not edit.
    pub fn new(backends: Vec<SocketAddr>) -> Self {
        Self {
            backends,
            connect_timeout: Duration::from_secs(3),
            eject_after_failures: 3,
            eject_cooldown: Duration::from_secs(10),
        }
    }
}

/// A snapshot of the proxy counters. All values only ever grow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProxyStats {
    /// Connections returned by `accept`.
    pub accepted: u64,
    /// Connections that reached a backend and whose forwarding has ended (clean or error).
    /// Connections aborted by `shutdown` are not counted.
    pub completed: u64,
    /// Failed connect attempts (refused, other I/O error, or timeout), summed over all backends.
    pub backend_connect_failures: u64,
    /// Client connections closed because no backend could be reached.
    pub no_backend_available: u64,
    /// Payload bytes written to backends.
    pub bytes_client_to_backend: u64,
    /// Payload bytes written to clients.
    pub bytes_backend_to_client: u64,
}

#[derive(Default)]
struct Counters {
    accepted: AtomicU64,
    completed: AtomicU64,
    backend_connect_failures: AtomicU64,
    no_backend_available: AtomicU64,
    bytes_client_to_backend: AtomicU64,
    bytes_backend_to_client: AtomicU64,
}

#[derive(Default)]
struct HealthState {
    failures: u32,
    /// `Some` from ejection until a success. A past instant means "cooldown over, trial due".
    ejected_until: Option<Instant>,
    trial_in_flight: bool,
}

struct Backend {
    addr: SocketAddr,
    health: Mutex<HealthState>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Permit {
    Normal,
    Trial,
}

/// Held for the duration of one connect attempt. Dropping it without reporting a result
/// releases a trial claim, so an aborted task cannot leave a backend ejected forever.
struct Attempt<'a> {
    shared: &'a Shared,
    backend: &'a Backend,
    permit: Permit,
    reported: bool,
}

impl Attempt<'_> {
    fn success(mut self) {
        self.reported = true;
        *self.backend.health.lock().unwrap() = HealthState::default();
    }

    fn failure(mut self) {
        self.reported = true;
        let mut h = self.backend.health.lock().unwrap();
        h.failures = h.failures.saturating_add(1);
        let threshold = self.shared.config.eject_after_failures.max(1);
        if self.permit == Permit::Trial || h.failures == threshold {
            h.ejected_until = Some(Instant::now() + self.shared.config.eject_cooldown);
            h.trial_in_flight = false;
        }
    }
}

impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if !self.reported && self.permit == Permit::Trial {
            let mut h = self.backend.health.lock().unwrap();
            h.ejected_until = Some(Instant::now() + self.shared.config.eject_cooldown);
            h.trial_in_flight = false;
        }
    }
}

struct Shared {
    config: ProxyConfig,
    backends: Vec<Backend>,
    connector: Box<dyn ErasedConnector>,
    stats: Counters,
}

impl Shared {
    fn acquire<'a>(&'a self, backend: &'a Backend) -> Option<Attempt<'a>> {
        let mut h = backend.health.lock().unwrap();
        let permit = match h.ejected_until {
            None => Permit::Normal,
            Some(until) if Instant::now() < until => return None,
            Some(_) if h.trial_in_flight => return None,
            Some(_) => {
                h.trial_in_flight = true;
                Permit::Trial
            }
        };
        Some(Attempt {
            shared: self,
            backend,
            permit,
            reported: false,
        })
    }
}

/// `Connector` has an `impl Future` return type, so it is not object safe. This adapter
/// boxes the future so `Shared` does not need a type parameter.
trait ErasedConnector: Send + Sync {
    fn connect(
        &self,
        addr: SocketAddr,
    ) -> std::pin::Pin<Box<dyn Future<Output = io::Result<TcpStream>> + Send + '_>>;
}

impl<C: Connector> ErasedConnector for C {
    fn connect(
        &self,
        addr: SocketAddr,
    ) -> std::pin::Pin<Box<dyn Future<Output = io::Result<TcpStream>> + Send + '_>> {
        Box::pin(Connector::connect(self, addr))
    }
}

/// A running proxy.
pub struct Proxy {
    local_addr: SocketAddr,
    shared: Arc<Shared>,
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl Proxy {
    /// Starts a proxy that connects with `TokioConnector`. Must be called inside a Tokio runtime.
    pub fn start(listener: TcpListener, config: ProxyConfig) -> Proxy {
        Self::start_with_connector(listener, config, TokioConnector)
    }

    /// Same as `start`, with a custom connector.
    pub fn start_with_connector<C: Connector>(
        listener: TcpListener,
        config: ProxyConfig,
        connector: C,
    ) -> Proxy {
        let local_addr = listener.local_addr().expect("listener has a local address");
        let backends = config
            .backends
            .iter()
            .map(|&addr| Backend {
                addr,
                health: Mutex::default(),
            })
            .collect();
        let shared = Arc::new(Shared {
            config,
            backends,
            connector: Box::new(connector),
            stats: Counters::default(),
        });
        let (stop, stop_rx) = watch::channel(false);
        let task = tokio::spawn(accept_loop(listener, shared.clone(), stop_rx));
        Proxy {
            local_addr,
            shared,
            stop,
            task,
        }
    }

    /// The address the proxy listens on.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// A snapshot of the counters.
    pub fn stats(&self) -> ProxyStats {
        let s = &self.shared.stats;
        // `completed` is read first: a connection counted there has already added its bytes.
        let completed = s.completed.load(SeqCst);
        ProxyStats {
            accepted: s.accepted.load(SeqCst),
            completed,
            backend_connect_failures: s.backend_connect_failures.load(SeqCst),
            no_backend_available: s.no_backend_available.load(SeqCst),
            bytes_client_to_backend: s.bytes_client_to_backend.load(SeqCst),
            bytes_backend_to_client: s.bytes_backend_to_client.load(SeqCst),
        }
    }

    /// Stops accepting (the listening socket is closed immediately), lets in-flight
    /// connections finish for up to `SHUTDOWN_GRACE`, aborts the rest, and returns once
    /// every task the proxy spawned has ended.
    pub async fn shutdown(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }
}

async fn accept_loop(listener: TcpListener, shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    let mut connections = JoinSet::new();
    let next = AtomicUsize::new(0);
    loop {
        tokio::select! {
            // Fires on shutdown and when the `Proxy` handle is dropped.
            _ = stop.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((client, _)) => {
                    shared.stats.accepted.fetch_add(1, SeqCst);
                    // The start index is fixed here, in accept order, not inside the task.
                    let start = next.fetch_add(1, SeqCst);
                    connections.spawn(handle(client, start, shared.clone()));
                }
                // EMFILE and similar are transient; back off instead of spinning.
                Err(_) => sleep(Duration::from_millis(10)).await,
            },
            // Reaps finished tasks so the set does not grow with connection count.
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
        }
    }
    drop(listener);
    let drain = async { while connections.join_next().await.is_some() {} };
    if timeout(SHUTDOWN_GRACE, drain).await.is_err() {
        connections.abort_all();
        while connections.join_next().await.is_some() {}
    }
}

async fn handle(client: TcpStream, start: usize, shared: Arc<Shared>) {
    let n = shared.backends.len();
    for offset in 0..n {
        let backend = &shared.backends[(start + offset) % n];
        let Some(attempt) = shared.acquire(backend) else {
            continue;
        };
        let connect = shared.connector.connect(backend.addr);
        match timeout(shared.config.connect_timeout, connect).await {
            Ok(Ok(upstream)) => {
                attempt.success();
                forward(client, upstream, &shared.stats).await;
                return;
            }
            _ => {
                shared.stats.backend_connect_failures.fetch_add(1, SeqCst);
                attempt.failure();
            }
        }
    }
    // Counted before the close so a client that sees EOF also sees the counter.
    shared.stats.no_backend_available.fetch_add(1, SeqCst);
    drop(client);
}

async fn forward(mut client: TcpStream, mut upstream: TcpStream, stats: &Counters) {
    let _ = client.set_nodelay(true);
    let _ = upstream.set_nodelay(true);
    // copy_bidirectional forwards EOF as shutdown(Write) and keeps the other direction open.
    if let Ok((to_backend, to_client)) = copy_bidirectional(&mut client, &mut upstream).await {
        stats.bytes_client_to_backend.fetch_add(to_backend, SeqCst);
        stats.bytes_backend_to_client.fetch_add(to_client, SeqCst);
    }
    stats.completed.fetch_add(1, SeqCst);
}
