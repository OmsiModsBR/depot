//! A line's tours: the number a new one takes and the copies a repeat makes.

use super::super::interface::output::Tour;

/// Copies of `base` every `every` minutes after it, as long as the copy's first departure is
/// not after `until` (minutes of the day); numbered on from the highest. Returns how many.
pub fn repeat_tour(tours: &mut Vec<Tour>, base: &Tour, every: f32, until: f32) -> usize {
    let first = base.trips.first().map(|t| t.departure).unwrap_or(0.0);
    let mut made = 0;
    let mut k = 1.0;
    while first + every * k <= until + 1e-3 && made < 500 {
        let mut t = base.clone();
        for x in &mut t.trips {
            x.departure += every * k;
        }
        t.number = next_number(tours);
        tours.push(t);
        made += 1;
        k += 1.0;
    }
    made
}

/// A new tour's number: one past the highest that is a number.
pub fn next_number(tours: &[Tour]) -> String {
    (tours
        .iter()
        .filter_map(|t| t.number.trim().parse::<i64>().ok())
        .max()
        .unwrap_or(0)
        + 1)
    .to_string()
}
