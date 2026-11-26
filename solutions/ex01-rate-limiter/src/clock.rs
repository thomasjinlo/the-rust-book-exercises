//! Clocks. Provided code: do not edit.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// A source of monotonic time in nanoseconds.
pub trait Clock {
    fn now_ns(&self) -> u64;
}

/// Nanoseconds elapsed since this clock was constructed.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_ns(&self) -> u64 {
        // Saturates after ~584 years.
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
}

/// A clock that only moves when told to. Clones share one time source.
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    ns: Arc<AtomicU64>,
}

impl ManualClock {
    pub fn new(start_ns: u64) -> Self {
        Self {
            ns: Arc::new(AtomicU64::new(start_ns)),
        }
    }

    /// Sets the time. May move backwards, to simulate a misbehaving source.
    pub fn set(&self, ns: u64) {
        self.ns.store(ns, Ordering::SeqCst);
    }

    /// Adds `ns`, saturating at `u64::MAX`.
    pub fn advance(&self, ns: u64) {
        let _ = self
            .ns
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |t| {
                Some(t.saturating_add(ns))
            });
    }
}

impl Clock for ManualClock {
    fn now_ns(&self) -> u64 {
        self.ns.load(Ordering::SeqCst)
    }
}
