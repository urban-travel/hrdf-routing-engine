use hrdf_parser::{DataStorage, Model};

use super::models::RouteResult;

impl RouteResult {
    #[rustfmt::skip]
    pub fn print(&self, data_storage: &DataStorage) {
        for section in self.sections() {
            let journey = section.journey(data_storage);

            if journey.is_none() {
                let stop = data_storage.stops().find(section.arrival_stop_id()).unwrap_or_else(|| panic!("Stop {:?} not found.", section.arrival_stop_id()));
                println!("Approx. {}-minute walk to {}", section.duration().unwrap(), stop.name());
                continue;
            }

            let journey = journey.unwrap();
            println!("Journey #{}", journey.id());

            let mut route_iter = journey.route().iter().peekable();

            while route_iter.peek().unwrap().stop_id() != section.departure_stop_id() {
                route_iter.next();
            }

            let mut route = Vec::new();

            loop {
                route.push(route_iter.next().unwrap());

                if route.last().unwrap().stop_id() == section.arrival_stop_id() {
                    break;
                }
            }

            println!("  Departure at: {}", section.departure_at().unwrap().format("%Y-%m-%d %H:%M"));

            for (i, route_entry) in route.iter().enumerate() {
                let arrival_time = if i == 0 {
                    " ".repeat(5)
                } else {
                    format!("{}", route_entry.arrival_time().as_ref().unwrap().format("%H:%M"))
                };

                let departure_time = if i == route.len() - 1 {
                    " ".repeat(5)
                } else {
                    format!("{}", route_entry.departure_time().as_ref().unwrap().format("%H:%M"))
                };

                let stop = route_entry.stop(data_storage);

                println!(
                    "    {:0>7} {: <36} {} - {}",
                    stop.id(),
                    stop.name(),
                    arrival_time,
                    departure_time,
                );
            }

            println!("  Arrival at: {}", section.arrival_at().unwrap().format("%Y-%m-%d %H:%M"));
        }
    }
}
