// use hrdf_parser::{DataStorage, Model};

// use super::models::Journey;

// impl Journey {
//     #[rustfmt::skip]
//     pub fn print(&self, data_storage: &DataStorage) {
//         for leg in self.legs() {
//             let trip = leg.trip(data_storage);

//             if trip.is_none() {
//                 let stop = data_storage.stops().find(leg.arrival_stop_id());
//                 println!("Approx. {}-minute walk to {}", leg.duration().unwrap(), stop.name());
//                 continue;
//             }

//             let trip = trip.unwrap();
//             println!(" Trip #{}", trip.id());

//             let mut route_iter = trip.route().into_iter().peekable();

//             while route_iter.peek().unwrap().stop_id() != leg.departure_stop_id() {
//                 route_iter.next();
//             }

//             let mut route = Vec::new();

//             loop {
//                 route.push(route_iter.next().unwrap());

//                 if route.last().unwrap().stop_id() == leg.arrival_stop_id() {
//                     break;
//                 }
//             }

//             println!("  Departure at: {}", leg.departure_at().unwrap().format("%Y-%m-%d %H:%M"));

//             for (i, route_entry) in route.iter().enumerate() {
//                 let arrival_time = if i == 0 {
//                     " ".repeat(5)
//                 } else {
//                     format!("{}", route_entry.arrival_time().as_ref().unwrap().format("%H:%M"))
//                 };

//                 let departure_time = if i == route.len() - 1 {
//                     " ".repeat(5)
//                 } else {
//                     format!("{}", route_entry.departure_time().as_ref().unwrap().format("%H:%M"))
//                 };

//                 let stop = route_entry.stop(data_storage);

//                 println!(
//                     "    {:0>7} {: <36} {} - {}",
//                     stop.id(),
//                     stop.name(),
//                     arrival_time,
//                     departure_time,
//                 );
//             }

//             println!("  Arrival at: {}", leg.arrival_at().unwrap().format("%Y-%m-%d %H:%M"));
//         }
//     }
// }
