//! Rate limiting with a token bucket and a caller-supplied clock.

mod bucket;
mod clock;
mod keyed;

pub use bucket::{ConfigError, TokenBucket};
pub use clock::{Clock, ManualClock, SystemClock};
pub use keyed::KeyedLimiter;
