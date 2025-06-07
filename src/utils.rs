use chrono::{Days, Duration, NaiveDate, NaiveDateTime, NaiveTime};

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
