//! Validated UTC instants shared by native and browser execution.

use std::fmt;
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

/// A nanosecond-precision instant with a printable four-digit UTC year.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moment(OffsetDateTime);

impl Moment {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        // The library normalizes leap seconds and truncates excess precision.
        // Reject both rather than silently changing the supplied instant.
        if text.as_bytes().get(17..19) == Some(b"60") {
            return Err("time of our lord does not support leap seconds");
        }
        if text.as_bytes().get(19) == Some(&b'.')
            && text.as_bytes()[20..]
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count()
                > 9
        {
            return Err("time of our lord supports at most 9 fractional second digits");
        }
        let value = OffsetDateTime::parse(text, &Rfc3339).map_err(|_| {
            "time of our lord requires a valid RFC 3339 timestamp with a timezone, such as 2026-12-25T00:00:00Z"
        })?;
        Self::from_nanos(value.unix_timestamp_nanos())
    }

    pub fn now() -> Result<Self, &'static str> {
        #[cfg(not(target_arch = "wasm32"))]
        let nanos = {
            use std::time::{SystemTime, UNIX_EPOCH};
            match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(duration) => duration.as_nanos() as i128,
                Err(error) => -(error.duration().as_nanos() as i128),
            }
        };
        #[cfg(target_arch = "wasm32")]
        let nanos = {
            let milliseconds = current_time_millis();
            if !milliseconds.is_finite() {
                return Err("the host clock did not return a valid time");
            }
            (milliseconds as i128)
                .checked_mul(1_000_000)
                .ok_or("the host clock is outside the supported time range")?
        };
        Self::from_nanos(nanos)
    }

    fn from_nanos(nanos: i128) -> Result<Self, &'static str> {
        let value = OffsetDateTime::from_unix_timestamp_nanos(nanos)
            .map_err(|_| "time of our lord is outside the supported UTC year range (0000–9999)")?;
        if !(0..=9999).contains(&value.year()) {
            return Err("time of our lord is outside the supported UTC year range (0000–9999)");
        }
        Ok(Self(value.to_offset(UtcOffset::UTC)))
    }

    pub fn seconds_since(self, earlier: Self) -> f64 {
        // Subtract before converting so nearby instants retain their precision.
        (self.0.unix_timestamp_nanos() - earlier.0.unix_timestamp_nanos()) as f64 / 1_000_000_000.0
    }
}

impl fmt::Display for Moment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.format(&Rfc3339).map_err(|_| fmt::Error)?)
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
export function current_time_millis() { return Date.now(); }
")]
extern "C" {
    fn current_time_millis() -> f64;
}
