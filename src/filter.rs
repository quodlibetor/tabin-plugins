//! Functions related to determining if the current Event should be Handled

use Event;
use Filter::{Disabled, NotRefresh, Active};

pub enum Filter<'a> {
    Disabled,
    TooSoon(u64,  // current
            u64), // required
    NotRefresh(u64, // interval
               u64), // checks until next
    Active(&'a Event),
}

trait DisplayError<'a> {
    fn display(&'a self);
}

trait ErrorParts<'a> {
    fn lib(&'a self) -> String;
    fn msg(&'a self) -> String;
}

impl<'a, T:ErrorParts<'a>> DisplayError<'a> for T {
    fn display(&'a self) {
        println!("[{}] {}", self.lib(), self.msg());
    }
}

impl<'a> ErrorParts<'a> for Filter<'a> {
    fn lib(&'a self) -> String { "iron_fan".into_string() }
    fn msg(&'a self) -> String {
        match *self {
            Filter::Disabled => format!("check is disabled"),
            Filter::TooSoon(current, required) =>
                format!("not enough occurrences ({} < {})",
                         current, required),
            Filter::NotRefresh(interval, next) =>
                format!("only handling every {} occurrences (next in {})",
                         interval, next),
            Filter::Active(_) =>
                format!("safe to handle event")
        }
    }
}

macro_rules! iron_filter{
    ($f:expr) => {
        match $f {
            Active(event) => event,
            _ => return $f
        }
    }
}

/// Run all filters in sequence
///
/// * `filter_disabled`
/// * `filter_repeated`
/// * `filter_silenced` (todo)
/// * `filter_dependencies` (todo)
///
/// in sequence.
pub fn run_filters<'a>(event: &'a Event) -> Filter<'a> {
    let mut event = iron_filter!(filter_disabled(event));
    event = iron_filter!(filter_repeated(event));

    Active(event)
}

/// Filter checks tha are self configured as disabled.
///
/// Checks self-register as disabled by setting `"alert": false` in their
/// configuration.
pub fn filter_disabled<'a>(event: &'a Event) -> Filter<'a> {
    match event.check.alert {
        Some(alert) => if alert { Active(event) } else { Disabled },
        None => Active(event)
    }
}

/// Allow events that:
///
/// * Are of any action other than `"create"`
/// * Have occured exactly `min_occurrences` times
/// * Are occuring exactly on the `refresh` border
///
/// Filter everything else.
pub fn filter_repeated<'a>(event: &'a Event) -> Filter<'a> {
    match event.action {
        Some(ref action) => if action.as_slice() != "create" {
            return Active(event)
        },
        None => {}
    };

    let min_occurrences = event.check.occurrences.unwrap_or(1);
    if event.occurrences == min_occurrences {
        return Active(event)
    }

    let interval = event.check.interval;
    let refresh = event.check.refresh;

    if event.occurrences < min_occurrences {
        return Filter::TooSoon(event.occurrences, min_occurrences);
    }
    if event.occurrences > min_occurrences {
        // number of checks required to hit "refresh" duration
        let intervals = refresh / interval;
        let next = event.occurrences % intervals;
        if intervals != 0 && next != 0 {
            return NotRefresh(interval, next);
        }
    }

    Active(event)
}
