//! A line-oriented TCP server on tokio with a connection limit, an idle
//! timeout, a bounded line length and graceful shutdown.
//!
//! Edit only this file (add modules if you like). The public API below must
//! not change: the given tests use it as written.

use std::future::Future;
use std::time::Duration;

use tokio::net::TcpListener;

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

/// Serves the line protocol on `listener` until `shutdown` resolves, then
/// drains connections and returns the final counters.
///
/// Contract: see `exercises/07-async-line-server.md`, requirements R1-R16.
#[allow(unused_variables)]
pub async fn serve(
    listener: TcpListener,
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Stats {
    todo!("accept loop, per-connection task, shutdown")
}
