//! A line-oriented TCP server on tokio with a connection limit.
//!
//! Replace the `todo!()`. Keep the public signature. Add any private items you need.

use tokio::net::TcpListener;

/// Serves the line protocol on `listener` until the task running it is dropped or aborted.
/// At most `max_connections` clients are served at once.
pub async fn serve(listener: TcpListener, max_connections: usize) {
    let _ = (listener, max_connections);
    todo!()
}
