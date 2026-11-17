use std::time::{SystemTime, UNIX_EPOCH};

/// Unix time in milliseconds. A clock before 1970 is treated as 0 rather than panicking.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
