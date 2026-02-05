use std::fmt::Display;

use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct JourneyArgs {
    pub departure_stop_id: i32,
    pub arrival_stop_id: i32,
    pub departure_at: NaiveDateTime,
    pub max_num_explorable_connections: i32,
    pub verbose: bool,
}

impl Display for JourneyArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "departure_stop_id: {}, arrival_stop_id: {}, departure_at: {}",
            self.departure_stop_id, self.arrival_stop_id, self.departure_at
        )
    }
}
