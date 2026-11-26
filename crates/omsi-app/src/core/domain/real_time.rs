//! Real-time sync, the arithmetic: a device's calendar date and time of day, and how far
//! one clock is ahead of another, across midnight too.

use omsi_sim::SimClock;

/// This device's local calendar date and time of day.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Now {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    /// Seconds since local midnight.
    pub secs: f64,
}

/// The days from year 1 to the given day (a running number to subtract dates with).
fn day_number(year: i32, day_of_year: i32) -> i64 {
    let y = year as i64 - 1;
    365 * y + y / 4 - y / 100 + y / 400 + day_of_year as i64
}

/// Seconds `real` is ahead of `mine` (negative: behind), across midnight too.
pub(crate) fn gap(mine: &SimClock, real: &SimClock) -> f64 {
    (day_number(real.year, real.day_of_year) - day_number(mine.year, mine.day_of_year)) as f64
        * 86400.0
        + (real.time - mine.time)
}
