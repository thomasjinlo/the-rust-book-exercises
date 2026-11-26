//! Token-bucket rate limiting with integer math and an injected clock.

mod bucket;
mod clock;
mod keyed;

pub use bucket::{ConfigError, TokenBucket};
pub use clock::{Clock, ManualClock, SystemClock};
pub use keyed::KeyedLimiter;
