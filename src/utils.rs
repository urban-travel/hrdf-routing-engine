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
    usize::try_from((date_2 - date_1).num_days()).unwrap() + 1
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

pub fn compute_remaining_threads(num_threads: usize, used_threads: usize) -> usize {
    if used_threads > num_threads && num_threads != 0 {
        panic!(
            "Error used_threads: {used_threads} cannot be larger than the num_threads: {num_threads}"
        );
    }
    let remaining_threads = num_threads as isize - used_threads as isize;
    if num_threads == 0 {
        0
    } else if remaining_threads > 1 {
        remaining_threads as usize
    } else {
        1usize
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
    fn test_compute_remaining_threads_zero_total() {
        assert_eq!(compute_remaining_threads(0, 0), 0);
        assert_eq!(compute_remaining_threads(0, 10), 0);
    }

    #[test]
    fn test_compute_remaining_threads_minimum_one() {
        assert_eq!(compute_remaining_threads(4, 3), 1);
        assert_eq!(compute_remaining_threads(4, 4), 1);
        assert_eq!(compute_remaining_threads(1, 0), 1);
    }

    #[test]
    fn test_compute_remaining_threads_many_remaining() {
        assert_eq!(compute_remaining_threads(8, 2), 6);
        assert_eq!(compute_remaining_threads(10, 0), 10);
    }

    #[test]
    #[should_panic(expected = "used_threads: 5 cannot be larger than the num_threads: 4")]
    fn test_compute_remaining_threads_panics_when_used_exceeds_total() {
        compute_remaining_threads(4, 5);
    }
}
