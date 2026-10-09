//! Minimum transfer times, by HRDF precedence: journey pair (UMSTEIGZ, and DURCHBI through
//! services at 0 minutes) > lines at the stop (UMSTEIGL) > administrations at the stop
//! (UMSTEIGV) > stop (UMSTEIGB) > lines anywhere > administrations anywhere > default.

use chrono::{Duration, NaiveDate};
use hrdf_parser::{LineSelector, Timetable};
use rustc_hash::FxHashMap;

use super::network::Trip;

const ANY_STOP: u32 = u32::MAX;
const ANY_ADMIN: u16 = u16::MAX;

/// ponytail: KMINFO 0 ("no transfer here") is parsed but not enforced: it marks half of the Swiss
/// stops. Enable it if the OJP comparison shows it matters.
const ENFORCE_KMINFO: bool = false;

pub struct TransferRules {
    /// (stop, from journey, to journey) -> (minutes, bit field) in precedence order.
    pairs: FxHashMap<(u32, u32, u32), Vec<(i16, i32)>>,
    stop_has_pairs: Vec<bool>,
    /// UMSTEIGL rules (indexes, in file order) by (stop or ANY_STOP, from and to administration
    /// or ANY_ADMIN).
    lines: FxHashMap<(u32, u16, u16), Vec<u32>>,
    stop_has_lines: Vec<bool>,
    any_stop_lines: bool,
    stop_has_admins: Vec<bool>,
    /// (stop or ANY_STOP, from administration, to administration) -> minutes.
    admins: FxHashMap<(u32, u16, u16), i16>,
    journey_admin: Vec<u16>,
    journey_is_ic: Vec<bool>,
    /// (IC to IC, any other) per stop.
    stop_minutes: Vec<Option<(i16, i16)>>,
    stop_allows_transfer: Vec<bool>,
    default: (i16, i16),
}

impl TransferRules {
    pub fn new(tt: &Timetable, stop_ids: &[i32], stop_index: &FxHashMap<i32, u32>) -> Self {
        fn intern<'a>(ids: &mut FxHashMap<&'a str, u16>, name: &'a str) -> u16 {
            let next = ids.len() as u16;
            *ids.entry(name).or_insert(next)
        }
        let mut admin_ids = FxHashMap::default();
        let journey_admin = tt
            .journeys
            .iter()
            .map(|j| intern(&mut admin_ids, &j.admin))
            .collect();

        let mut journeys_by_key: FxHashMap<(i32, &str), Vec<u32>> = FxHashMap::default();
        for (i, j) in tt.journeys.iter().enumerate() {
            journeys_by_key
                .entry((j.number, &j.admin))
                .or_default()
                .push(i as u32);
        }
        // Through services first: they win over a journey pair rule for the same journeys.
        let through = tt
            .through_services
            .iter()
            .map(|t| (t.stop_id, &t.from, &t.to, 0, t.bit_field));
        let pair_rules = (tt.journey_transfer_times.iter())
            .map(|t| (t.stop_id, &t.from, &t.to, t.minutes, t.bit_field));
        let mut pairs: FxHashMap<_, Vec<_>> = FxHashMap::default();
        for (stop_id, from, to, minutes, bit_field) in through.chain(pair_rules) {
            let from = journeys_by_key.get(&(from.0, from.1.as_str()));
            let to = journeys_by_key.get(&(to.0, to.1.as_str()));
            let (Some(&stop), Some(from), Some(to)) = (stop_index.get(&stop_id), from, to) else {
                continue;
            };
            for &a in from {
                for &b in to {
                    pairs
                        .entry((stop, a, b))
                        .or_default()
                        .push((minutes, bit_field));
                }
            }
        }
        let mut stop_has_pairs = vec![false; stop_ids.len()];
        pairs
            .keys()
            .for_each(|&(stop, ..)| stop_has_pairs[stop as usize] = true);

        let stop_of =
            |id: Option<i32>| id.map_or(Some(ANY_STOP), |id| stop_index.get(&id).copied());
        let mut lines: FxHashMap<_, Vec<_>> = FxHashMap::default();
        for (i, t) in tt.line_transfer_times.iter().enumerate() {
            let Some(stop) = stop_of(t.stop_id) else {
                continue;
            };
            let from = t
                .from
                .admin
                .as_deref()
                .map_or(ANY_ADMIN, |a| intern(&mut admin_ids, a));
            let to =
                t.to.admin
                    .as_deref()
                    .map_or(ANY_ADMIN, |a| intern(&mut admin_ids, a));
            lines.entry((stop, from, to)).or_default().push(i as u32);
        }
        let mut stop_has_lines = vec![false; stop_ids.len()];
        lines
            .keys()
            .filter(|k| k.0 != ANY_STOP)
            .for_each(|&(stop, ..)| stop_has_lines[stop as usize] = true);

        let mut admins = FxHashMap::default();
        for t in &tt.admin_transfer_times {
            let Some(stop) = stop_of(t.stop_id) else {
                continue;
            };
            let (from, to) = (
                intern(&mut admin_ids, &t.from),
                intern(&mut admin_ids, &t.to),
            );
            admins.entry((stop, from, to)).or_insert(t.minutes);
        }

        let mut stop_has_admins = vec![false; stop_ids.len()];
        admins
            .keys()
            .filter(|k| k.0 != ANY_STOP)
            .for_each(|&(stop, ..)| stop_has_admins[stop as usize] = true);

        Self {
            pairs,
            stop_has_pairs,
            any_stop_lines: lines.keys().any(|k| k.0 == ANY_STOP),
            lines,
            stop_has_lines,
            stop_has_admins,
            admins,
            journey_admin,
            journey_is_ic: tt.journeys.iter().map(|j| j.transport == "IC").collect(),
            stop_minutes: stop_ids
                .iter()
                .map(|id| tt.stops[id].transfer_time)
                .collect(),
            stop_allows_transfer: stop_ids
                .iter()
                .map(|id| tt.stops[id].transfer_allowed)
                .collect(),
            default: tt.default_transfer_time,
        }
    }

    /// Minutes needed at `stop` between arriving with `from` and leaving with `to`, or None when
    /// the change is not allowed. `date` is the day trips are relative to.
    pub fn minutes(
        &self,
        tt: &Timetable,
        date: NaiveDate,
        stop: u32,
        from: Trip,
        to: Trip,
    ) -> Option<i32> {
        let s = stop as usize;
        if self.stop_has_pairs[s]
            && let Some(rules) = self.pairs.get(&(stop, from.journey, to.journey))
        {
            let service_date = date + Duration::days(from.day.into());
            if let Some(&(minutes, _)) = rules.iter().find(|(_, b)| tt.runs_on(*b, service_date)) {
                return Some(minutes.into());
            }
        }
        if ENFORCE_KMINFO && !self.stop_allows_transfer[s] {
            return None;
        }
        let (from, to) = (from.journey as usize, to.journey as usize);
        let (a, b) = (self.journey_admin[from], self.journey_admin[to]);
        let lines = |stop| self.line_minutes(tt, stop, from, to, a, b);
        let ic = self.journey_is_ic[from] && self.journey_is_ic[to];
        let pick = |(ic_minutes, other)| if ic { ic_minutes } else { other };
        let minutes = (self.stop_has_lines[s].then(|| lines(stop)).flatten())
            .or_else(|| {
                self.stop_has_admins[s]
                    .then(|| self.admins.get(&(stop, a, b)).copied())
                    .flatten()
            })
            .or_else(|| self.stop_minutes[s].map(pick))
            .or_else(|| self.any_stop_lines.then(|| lines(ANY_STOP)).flatten())
            .or_else(|| self.admins.get(&(ANY_STOP, a, b)).copied())
            .unwrap_or_else(|| pick(self.default));
        Some(minutes.into())
    }

    /// The first UMSTEIGL rule at `stop` matching both journeys.
    fn line_minutes(
        &self,
        tt: &Timetable,
        stop: u32,
        from: usize,
        to: usize,
        a: u16,
        b: u16,
    ) -> Option<i16> {
        let matches = |selector: &LineSelector, journey: usize| {
            let j = &tt.journeys[journey];
            selector
                .transport
                .as_ref()
                .is_none_or(|t| *t == j.transport)
                && selector
                    .line
                    .as_ref()
                    .is_none_or(|l| j.line.is_some_and(|i| tt.lines[i as usize] == *l))
                && selector.direction.is_none_or(|d| j.direction == Some(d))
        };
        let keys = [
            (a, b),
            (a, ANY_ADMIN),
            (ANY_ADMIN, b),
            (ANY_ADMIN, ANY_ADMIN),
        ];
        let rules = keys
            .iter()
            .filter_map(|&(x, y)| self.lines.get(&(stop, x, y)))
            .flatten();
        let rule = rules.copied().filter(|&i| {
            let rule = &tt.line_transfer_times[i as usize];
            matches(&rule.from, from) && matches(&rule.to, to)
        });
        rule.min()
            .map(|i| tt.line_transfer_times[i as usize].minutes)
    }
}
