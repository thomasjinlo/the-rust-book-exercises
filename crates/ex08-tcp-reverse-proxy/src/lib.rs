//! A layer-4 TCP reverse proxy with round-robin balancing and passive health checking.
//!
//! Edit this file (and add modules under `src/` if you want). Keep the public API
//! below unchanged: the given tests use only it.

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};

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

/// A running proxy. Replace the placeholder field with whatever state you need.
pub struct Proxy {
    _placeholder: (),
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
        let _ = (listener, config, connector);
        todo!("spawn the accept loop and return a handle")
    }

    /// The address the proxy listens on.
    pub fn local_addr(&self) -> SocketAddr {
        todo!()
    }

    /// A snapshot of the counters.
    pub fn stats(&self) -> ProxyStats {
        todo!()
    }

    /// Stops accepting (the listening socket is closed immediately), lets in-flight
    /// connections finish for up to `SHUTDOWN_GRACE`, aborts the rest, and returns once
    /// every task the proxy spawned has ended.
    pub async fn shutdown(self) {
        todo!()
    }
}
