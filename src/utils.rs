use chrono::{Days, Duration, NaiveDate, NaiveDateTime, NaiveTime};

// TODO: ...

pub fn add_1_day(date: NaiveDate) -> NaiveDate {
    date.checked_add_days(Days::new(1)).unwrap()
}

pub fn add_minutes_to_date_time(date_time: NaiveDateTime, minutes: i64) -> NaiveDateTime {
    date_time
        .checked_add_signed(Duration::minutes(minutes))
        .unwrap()
}

pub fn count_days_between_two_dates(date_1: NaiveDate, date_2: NaiveDate) -> usize {
    usize::try_from((date_2 - date_1).num_days())
        .unwrap_or_else(|_| panic!("date_2 ({date_2}) must not be before date_1 ({date_1})"))
        + 1
}

pub fn create_date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

pub fn create_time(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

pub fn create_date_time(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
    NaiveDateTime::new(create_date(year, month, day), create_time(hour, minute))
}

/// Computes the number of threads to use in a nested parallel region.
/// If `num_threads == 0`, the global pool is used and nesting is allowed.
pub fn inner_threads(num_threads: usize, in_parallel: bool) -> usize {
    if num_threads == 0 {
        0
    } else if in_parallel {
        1
    } else {
        num_threads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_1_day() {
        let date = create_date(2026, 1, 31);
        assert_eq!(add_1_day(date), create_date(2026, 2, 1));

        let date = create_date(2026, 12, 31);
        assert_eq!(add_1_day(date), create_date(2027, 1, 1));

        let date = create_date(2024, 2, 28); // leap year
        assert_eq!(add_1_day(date), create_date(2024, 2, 29));
    }

    #[test]
    fn test_add_minutes_to_date_time() {
        let dt = create_date_time(2026, 6, 15, 9, 0);
        assert_eq!(
            add_minutes_to_date_time(dt, 90),
            create_date_time(2026, 6, 15, 10, 30)
        );

        // crosses midnight
        assert_eq!(
            add_minutes_to_date_time(dt, 900),
            create_date_time(2026, 6, 16, 0, 0)
        );

        // negative minutes
        assert_eq!(
            add_minutes_to_date_time(dt, -30),
            create_date_time(2026, 6, 15, 8, 30)
        );
    }

    #[test]
    fn test_count_days_between_two_dates() {
        let d1 = create_date(2026, 1, 1);
        let d2 = create_date(2026, 1, 1);
        assert_eq!(count_days_between_two_dates(d1, d2), 1);

        let d1 = create_date(2026, 1, 1);
        let d2 = create_date(2026, 1, 7);
        assert_eq!(count_days_between_two_dates(d1, d2), 7);

        // across month boundary
        let d1 = create_date(2026, 1, 30);
        let d2 = create_date(2026, 2, 1);
        assert_eq!(count_days_between_two_dates(d1, d2), 3);
    }

    #[test]
    #[should_panic(expected = "must not be before")]
    fn test_count_days_between_two_dates_reversed_panics() {
        let d1 = create_date(2026, 1, 7);
        let d2 = create_date(2026, 1, 1);
        count_days_between_two_dates(d1, d2);
    }

    #[test]
    fn test_create_date() {
        assert_eq!(
            create_date(2026, 6, 15),
            NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()
        );
    }

    #[test]
    fn test_create_time() {
        assert_eq!(
            create_time(9, 30),
            NaiveTime::from_hms_opt(9, 30, 0).unwrap()
        );
    }

    #[test]
    fn test_create_date_time() {
        let dt = create_date_time(2026, 6, 15, 9, 30);
        assert_eq!(dt.date(), create_date(2026, 6, 15));
        assert_eq!(dt.time(), create_time(9, 30));
    }

    #[test]
    fn test_inner_threads_global_pool_allows_nesting() {
        assert_eq!(inner_threads(0, false), 0);
        assert_eq!(inner_threads(0, true), 0);
    }

    #[test]
    fn test_inner_threads_caps_when_nested() {
        assert_eq!(inner_threads(4, true), 1);
        assert_eq!(inner_threads(1, true), 1);
    }

    #[test]
    fn test_inner_threads_uses_full_when_not_nested() {
        assert_eq!(inner_threads(4, false), 4);
        assert_eq!(inner_threads(1, false), 1);
    }
}
