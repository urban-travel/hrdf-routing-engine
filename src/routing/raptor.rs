//! RAPTOR, Round-bAsed Public Transit Optimized Router (Delling, Pajor, Werneck, 2012).
//!
//! Rather than searching a graph like Dijkstra, RAPTOR works in rounds: after round k, each stop
//! has the earliest time it can be reached with at most k trips (k - 1 transfers).
//!
//! - Round 0: the seeds (where the search starts), then the stops within walking distance.
//! - Round k: every pattern (trips calling at the same stops, see `network.rs`) serving a stop
//!   improved in round k - 1 is followed stop by stop. At each stop, get off the trip being
//!   ridden if it arrives earlier than known, and get on an earlier trip if the stop, as reached
//!   in round k - 1, allows it. Then walk from the stops just improved.
//! - The search stops when a round improves nothing, or after `max_rounds` rounds.
//!
//! It is fast because a round only scans the patterns touched by the previous one, in flat arrays
//! read in order. The latest departure is found by the same code on a network whose times are
//! negated (see `Network::reversed`): there, the earliest "arrival" is the latest departure.
//!
//! Each stop has two labels: arrived by trip, after which a transfer time applies, and arrived on
//! foot (or seeded), after which it does not. Neither dominates the other: a later arrival on
//! foot can still catch a trip that an earlier arrival by trip misses. The journey is rebuilt
//! afterwards by following back how each label got its time (`Parent`).

use super::{
    Router,
    network::{Network, Pattern, Trip},
};

pub const INF: i32 = i32::MAX;
const BY_TRIP: usize = 0;
const ON_FOOT: usize = 1;

/// How a label got its time.
#[derive(Clone, Copy, Debug)]
enum Parent {
    None,
    Seed,
    /// Rode `trip` of `pattern` from position `from` to `to`, boarding with the label of the stop
    /// at `from` (`on_foot` or by trip) as it was after round `round`.
    Trip {
        pattern: u32,
        trip: u32,
        from: u16,
        to: u16,
        round: u8,
        on_foot: bool,
    },
    /// Walked from the by-trip label of `from`, set in the same round.
    Walk {
        from: u32,
        minutes: i16,
    },
}

/// A part of a journey, in travel order. Stops are stop indexes, positions are journey indexes.
#[derive(Clone, Copy, Debug)]
pub enum Leg {
    Ride { trip: Trip, first: u16, last: u16 },
    Walk { from: u32, to: u32, minutes: i16 },
}

/// The result of [`run`]: the best time of every label, and how it got it.
pub struct Search {
    /// Label `2 * stop + slot`: arrival in a forward network, negated departure in a backward one.
    labels: Vec<i32>,
    round: Vec<u8>,
    /// Every improvement of a label, newest first through `head` and `Improvement::previous`.
    log: Vec<Improvement>,
    head: Vec<u32>,
    target: Option<u32>,
    /// Times must stay below this.
    limit: i32,
}

const NONE: u32 = u32::MAX;

struct Improvement {
    parent: Parent,
    round: u8,
    previous: u32,
}

/// Labels improved in the current round.
struct Marked {
    labels: Vec<u32>,
    is_marked: Vec<bool>,
}

impl Marked {
    fn mark(&mut self, label: usize) {
        if !std::mem::replace(&mut self.is_marked[label], true) {
            self.labels.push(label as u32);
        }
    }
}

/// Finds the best times from `seeds` (stop, time) with at most `max_rounds` trips. With a
/// `target`, labels that cannot improve the target are pruned.
pub fn run(
    router: &Router,
    net: &Network,
    seeds: &[(u32, i32)],
    target: Option<u32>,
    max_rounds: usize,
    limit: i32,
) -> Search {
    let n = 2 * net.stop_patterns.len();
    let mut search = Search {
        labels: vec![INF; n],
        round: vec![0; n],
        log: Vec::new(),
        head: vec![NONE; n],
        target,
        limit,
    };
    let mut marked = Marked {
        labels: Vec::new(),
        is_marked: vec![false; n],
    };
    // Round 0: the seeds, then walking from them.
    for &(stop, time) in seeds {
        search.improve(
            2 * stop as usize + ON_FOOT,
            time,
            0,
            Parent::Seed,
            &mut marked,
        );
    }
    search.walk(router, net, 0, &mut marked);

    // Positions to scan per pattern: from its first to its last improved stop, then for as long
    // as a trip is ridden.
    let mut first_pos = vec![u32::MAX; net.patterns.len()];
    let mut last_pos = vec![0; net.patterns.len()];
    let mut queue = Vec::new();
    // Only labels improved in the previous round can board a trip not ridden before. Their time
    // and round at the start of the round are kept, as this round may improve them again.
    let mut improved = Vec::new();
    let mut is_improved = vec![false; n];
    let (mut prev_time, mut prev_round) = (vec![INF; n], vec![0; n]);
    for k in 1..=max_rounds {
        if marked.labels.is_empty() {
            break;
        }
        // 1. The patterns to scan: those calling at a stop improved in the previous round.
        improved
            .iter()
            .for_each(|&label: &u32| is_improved[label as usize] = false);
        improved.clear();
        std::mem::swap(&mut improved, &mut marked.labels);
        for &label in &improved {
            let label = label as usize;
            marked.is_marked[label] = false;
            is_improved[label] = true;
            (prev_time[label], prev_round[label]) = (search.labels[label], search.round[label]);
            for &(pattern, pos) in &net.stop_patterns[label / 2] {
                let first = &mut first_pos[pattern as usize];
                if *first == u32::MAX {
                    queue.push(pattern);
                }
                *first = (*first).min(pos);
                last_pos[pattern as usize] = last_pos[pattern as usize].max(pos);
            }
        }

        // 2. Follow each pattern stop by stop.
        for p in queue.drain(..) {
            let start = std::mem::replace(&mut first_pos[p as usize], u32::MAX) as usize;
            let last = std::mem::take(&mut last_pos[p as usize]) as usize;
            let pattern = &net.patterns[p as usize];
            // The trip being ridden: (trip in pattern, boarding position, slot, round of label).
            let mut riding: Option<(usize, usize, usize, u8)> = None;
            for pos in start..pattern.len {
                if riding.is_none() && pos > last {
                    break;
                }
                let stop = net.stop(pattern, pos) as usize;
                // Get off: the trip being ridden may reach this stop earlier than known so far.
                if let Some((trip, from, slot, round)) = riding {
                    let event = net.event(pattern, trip, pos);
                    let label = 2 * stop + BY_TRIP;
                    if event.can_alight && event.arrival < search.labels[label] {
                        let (trip, from, to) =
                            ((pattern.first_trip + trip) as u32, from as u16, pos as u16);
                        let on_foot = slot == ON_FOOT;
                        let parent = Parent::Trip {
                            pattern: p,
                            trip,
                            from,
                            to,
                            round,
                            on_foot,
                        };
                        search.improve(label, event.arrival, k, parent, &mut marked);
                    }
                }
                if pos + 1 == pattern.len {
                    continue;
                }
                // Get on: with the stop's time from the previous round, an earlier trip than the
                // one ridden may be caught here.
                for slot in [BY_TRIP, ON_FOOT] {
                    let label = 2 * stop + slot;
                    if !is_improved[label] {
                        continue;
                    }
                    let arrived_with = match search.parent(label, prev_round[label]) {
                        Parent::Trip { trip, .. } if slot == BY_TRIP => {
                            Some(net.trips[trip as usize])
                        }
                        _ => None,
                    };
                    let end = riding.map_or(pattern.trips, |(trip, ..)| trip);
                    let ready = prev_time[label];
                    if let Some(trip) =
                        earliest_trip(router, net, pattern, pos, end, ready, arrived_with)
                    {
                        riding = Some((trip, pos, slot, prev_round[label]));
                    }
                }
            }
        }
        // 3. Walk from the stops this round's trips improved.
        search.walk(router, net, k, &mut marked);
    }
    search
}

/// The first trip of the pattern before `end` that can be boarded at `pos` when ready at `ready`,
/// after arriving with `arrived_with` (None: no transfer time).
fn earliest_trip(
    router: &Router,
    net: &Network,
    pattern: &Pattern,
    pos: usize,
    end: usize,
    ready: i32,
    arrived_with: Option<Trip>,
) -> Option<usize> {
    // Earlier trips leave even earlier, so if trip `end` is missed, all are.
    if end < pattern.trips && net.event(pattern, end, pos).departure < ready {
        return None;
    }
    let (mut lo, mut hi) = (0, end);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if net.event(pattern, mid, pos).departure < ready {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    // The minimum transfer time depends on both trips, so check them in order.
    (lo..end).find(|&trip| {
        let event = net.event(pattern, trip, pos);
        let earliest = match arrived_with {
            None => Some(ready),
            Some(arriving) => {
                let departing = net.trips[pattern.first_trip + trip];
                router
                    .transfer_minutes(net, net.stop(pattern, pos), arriving, departing)
                    .map(|m| ready + m)
            }
        };
        event.can_board && earliest.is_some_and(|t| event.departure >= t)
    })
}

impl Search {
    pub fn time(&self, stop: u32) -> Option<i32> {
        Some(self.best(stop as usize)).filter(|&t| t != INF)
    }

    pub fn stop_count(&self) -> usize {
        self.labels.len() / 2
    }

    fn best(&self, stop: usize) -> i32 {
        self.labels[2 * stop].min(self.labels[2 * stop + 1])
    }

    /// Sets a better time (found in round `k`). Times not below the target's best are dropped:
    /// a journey through them cannot reach the target earlier (target pruning).
    fn improve(&mut self, label: usize, time: i32, k: usize, parent: Parent, marked: &mut Marked) {
        let bound = self
            .target
            .map_or(self.limit, |t| self.limit.min(self.best(t as usize)));
        if time < self.labels[label] && time < bound {
            self.labels[label] = time;
            self.round[label] = k as u8;
            self.log.push(Improvement {
                parent,
                round: k as u8,
                previous: self.head[label],
            });
            self.head[label] = (self.log.len() - 1) as u32;
            marked.mark(label);
        }
    }

    /// How `label` got the time it had after `round`.
    fn parent(&self, label: usize, round: u8) -> Parent {
        let mut i = self.head[label];
        while i != NONE && self.log[i as usize].round > round {
            i = self.log[i as usize].previous;
        }
        if i == NONE {
            Parent::None
        } else {
            self.log[i as usize].parent
        }
    }

    /// Footpaths from the labels improved by trips (or seeded) in round `k`.
    fn walk(&mut self, router: &Router, net: &Network, k: usize, marked: &mut Marked) {
        for i in 0..marked.labels.len() {
            let from = marked.labels[i] as usize / 2;
            let time = self.labels[marked.labels[i] as usize];
            for &(to, minutes) in router.footpaths(net.backward, from as u32) {
                let parent = Parent::Walk {
                    from: from as u32,
                    minutes,
                };
                self.improve(
                    2 * to as usize + ON_FOOT,
                    time + i32::from(minutes),
                    k,
                    parent,
                    marked,
                );
            }
        }
    }

    /// The legs of the best journey between a seed and `stop`, in travel order: the parents are
    /// followed back from `stop` to a seed.
    pub fn legs(&self, net: &Network, stop: u32) -> Vec<Leg> {
        let stop = stop as usize;
        let key = |label: usize| (self.labels[label], self.round[label]);
        let mut label = if key(2 * stop + ON_FOOT) < key(2 * stop) {
            2 * stop + ON_FOOT
        } else {
            2 * stop
        };
        let mut round = self.round[label];
        let mut legs = Vec::new();
        loop {
            match self.parent(label, round) {
                Parent::None | Parent::Seed => break,
                Parent::Trip {
                    pattern,
                    trip,
                    from,
                    to,
                    round: board_round,
                    on_foot,
                } => {
                    let p = &net.patterns[pattern as usize];
                    let t = trip as usize - p.first_trip;
                    let (a, b) = (
                        net.event(p, t, from.into()).pos,
                        net.event(p, t, to.into()).pos,
                    );
                    let (first, last) = if net.backward { (b, a) } else { (a, b) };
                    legs.push(Leg::Ride {
                        trip: net.trips[trip as usize],
                        first,
                        last,
                    });
                    label = 2 * net.stop(p, from.into()) as usize + usize::from(on_foot);
                    round = board_round;
                }
                Parent::Walk { from, minutes } => {
                    let to = (label / 2) as u32;
                    let (a, b) = if net.backward { (to, from) } else { (from, to) };
                    legs.push(Leg::Walk {
                        from: a,
                        to: b,
                        minutes,
                    });
                    label = 2 * from as usize + BY_TRIP;
                }
            }
        }
        // Backtracking a backward search already walks in travel order.
        if !net.backward {
            legs.reverse();
        }
        legs
    }
}
