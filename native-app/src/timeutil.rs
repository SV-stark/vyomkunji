#![allow(dead_code)] // helpers are shared across the host and GUI binaries
// Small date/time helpers (no external crates).
use std::time::{SystemTime, UNIX_EPOCH};

fn unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Days since 1970-01-01 from a civil (y, m, d) date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Civil (y, m, d) from days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (y + i64::from(m <= 2), m, d)
}

/// ISO-8601 UTC timestamp, e.g. 2026-10-20T18:04:11Z
pub fn now_iso() -> String {
    let secs = unix_secs();
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        mo,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Wall-clock time only, e.g. 18:04:11
pub fn now_clock() -> String {
    let secs = unix_secs();
    let rem = secs.rem_euclid(86400);
    format!("{:02}:{:02}:{:02}", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Days remaining until a "YYYY-MM-DD" date (negative if past).
pub fn days_until(date: &str) -> Option<i64> {
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    let today = unix_secs().div_euclid(86400);
    Some(days_from_civil(y, m, d) - today)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_civil_roundtrip() {
        let days = days_from_civil(2026, 10, 21);
        let (y, m, d) = civil_from_days(days);
        assert_eq!((y, m, d), (2026, 10, 21));
    }

    #[test]
    fn test_now_iso_format() {
        let iso = now_iso();
        assert_eq!(iso.len(), 20);
        assert!(iso.ends_with('Z'));
        assert_eq!(&iso[10..11], "T");
    }

    #[test]
    fn test_days_until() {
        assert!(days_until("invalid-date").is_none());
        assert!(days_until("2026-10-21").is_some());
    }
}
