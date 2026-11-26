//! A line-oriented TCP server on tokio with a connection limit and a bounded
//! line length.
//!
//! Edit only this file (add modules if you like). The public API below must
//! not change: the given tests use it as written.

use std::future::Future;

use tokio::net::TcpListener;

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

/// Serves the line protocol on `listener` until `shutdown` resolves. Then it
/// stops accepting, waits until every open connection has ended, and returns
/// the final counters.
///
/// Contract: see `exercises/08-async-line-server.md`, Requirements.
#[allow(unused_variables)]
pub async fn serve(
    listener: TcpListener,
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Stats {
    todo!("accept loop, per-connection task, shutdown")
}
