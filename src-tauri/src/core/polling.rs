use chrono::{DateTime, Duration, NaiveDate, Utc};
use rand::Rng;
use std::collections::VecDeque;

pub const AVAILABLE_MIN_INTERVAL_SECONDS: i64 = 180;
pub const FAV_REFRESH_SECONDS: i64 = 15 * 60;
pub const OMAEP_DAY_MIN_INTERVAL_SECONDS: i64 = 5 * 60;
pub const BACKGROUND_REQUEST_LIMIT: usize = 5;
pub const BACKGROUND_WINDOW_SECONDS: i64 = 10 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

pub fn ranges_for_dates(mut dates: Vec<NaiveDate>) -> Vec<DateRange> {
    dates.sort();
    dates.dedup();
    let mut ranges = vec![];
    let mut remaining = dates.as_slice();
    while let Some(first) = remaining.first().copied() {
        let end = first + Duration::days(15);
        let count = remaining.iter().take_while(|date| **date <= end).count();
        ranges.push(DateRange { from: first, to: end });
        remaining = &remaining[count..];
    }
    ranges
}

pub fn next_available_delay() -> Duration {
    Duration::seconds(AVAILABLE_MIN_INTERVAL_SECONDS + rand::thread_rng().gen_range(0..=60))
}

pub fn backoff_delay(failures: u32, consecutive_timeouts: u32) -> Duration {
    if consecutive_timeouts >= 2 {
        return Duration::minutes(30);
    }
    let shift = failures.saturating_sub(1).min(5);
    Duration::minutes(1 << shift).min(Duration::minutes(30))
}

pub fn reserve_background_request(log: &mut VecDeque<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    let cutoff = now - Duration::seconds(BACKGROUND_WINDOW_SECONDS);
    while log.front().is_some_and(|time| *time < cutoff) {
        log.pop_front();
    }
    if log.len() >= BACKGROUND_REQUEST_LIMIT {
        return false;
    }
    log.push_back(now);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parse_date;

    #[test]
    fn ranges_never_exceed_sixteen_days() {
        let dates = vec![
            parse_date("01.11.2026").unwrap(),
            parse_date("20.11.2026").unwrap(),
            parse_date("05.12.2026").unwrap(),
        ];
        let ranges = ranges_for_dates(dates);
        assert_eq!(ranges.len(), 3);
        assert!(ranges.iter().all(|range| (range.to - range.from).num_days() <= 15));
    }

    #[test]
    fn backoff_grows_and_detects_blocked_ip() {
        assert_eq!(backoff_delay(1, 0), Duration::minutes(1));
        assert_eq!(backoff_delay(2, 0), Duration::minutes(2));
        assert_eq!(backoff_delay(8, 0), Duration::minutes(30));
        assert_eq!(backoff_delay(2, 2), Duration::minutes(30));
    }

    #[test]
    fn background_budget_is_sliding() {
        let now = Utc::now();
        let mut log = VecDeque::new();
        for _ in 0..5 {
            assert!(reserve_background_request(&mut log, now));
        }
        assert!(!reserve_background_request(&mut log, now));
        assert!(reserve_background_request(&mut log, now + Duration::minutes(11)));
    }
}
