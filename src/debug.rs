use std::time::Instant;

use chrono::NaiveDateTime;

use crate::{Router, plan_journey};

/// Journeys to plan: description, departure and arrival stops, departure time.
#[rustfmt::skip]
const JOURNEYS: [(&str, i32, i32, &str); 12] = [
    ("Petit-Lancy, Les Esserts => Onex, Bandol", 8587418, 8593027, "2025-06-01 12:30"),
    ("Petit-Lancy, Les Esserts => Genève-Aéroport", 8587418, 8501026, "2025-02-09 14:02"),
    ("Avully, village => Pont-Céard, gare", 8587031, 8593189, "2025-07-13 16:43"),
    ("Petit-Lancy, Les Esserts => Vevey, Palud", 8587418, 8595120, "2025-09-17 05:59"),
    ("Genève, gare Cornavin => Avusy, village", 8587057, 8587032, "2025-10-18 20:10"),
    ("Genève => Bern, Bierhübeli", 8501008, 8590028, "2025-11-22 06:59"),
    ("Genève => Zürich HB", 8501008, 8503000, "2025-04-09 08:04"),
    ("Zürich HB => Lugano, Genzana", 8503000, 8575310, "2025-06-15 12:10"),
    ("Chancy, Douane => Campocologno", 8587477, 8509368, "2025-05-29 17:29"),
    ("Chancy, Douane => Sevelen, Post", 8587477, 8581989, "2025-09-10 13:37"),
    ("Genève => Paris gare de Lyon", 8501008, 8768600, "2025-04-28 08:29"),
    ("Genève => Lausanne", 8501008, 8501120, "2025-04-28 08:20"),
];

/// Plans and prints journeys of timetable 2025, to check that they still run.
pub fn run_debug(router: &Router) {
    let line = "-".repeat(96);
    println!("\n{line}\n--- Debug\n{line}\n");
    let started = Instant::now();
    for (i, (name, from, to, at)) in JOURNEYS.into_iter().enumerate() {
        println!("{}. {name}", i + 1);
        let at = NaiveDateTime::parse_from_str(at, "%Y-%m-%d %H:%M").unwrap();
        plan_journey(router, from, to, at, 10, true);
        println!("==============================================");
    }
    println!("\n{:.2?}", started.elapsed());
}
