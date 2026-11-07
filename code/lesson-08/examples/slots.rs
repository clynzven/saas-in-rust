//! Print the bookable slots for one day: `cargo run --example slots`
use chrono::{NaiveDate, NaiveTime, TimeDelta};
use slotbook::availability::{Busy, Query, slots, to_utc};
use uuid::Uuid;

fn main() {
    let tz = chrono_tz::Pacific::Auckland;
    let date = NaiveDate::from_ymd_opt(2026, 12, 1).unwrap();
    let at = |h, m| to_utc(tz, date.and_time(NaiveTime::from_hms_opt(h, m, 0).unwrap())).unwrap();
    let (ana, ben) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let busy = [
        Busy {
            staff_id: ana,
            start: at(9, 0),
            end: at(10, 0),
        },
        Busy {
            staff_id: ben,
            start: at(9, 30),
            end: at(10, 30),
        },
    ];
    let q = Query {
        date,
        tz,
        hours: Some((
            NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
        )),
        duration: TimeDelta::minutes(45),
        step: TimeDelta::minutes(15),
        not_before: at(0, 0), // in the app: now + minimum notice
        staff: &[ana, ben],
        busy: &busy,
    };
    println!("45-minute slots on {date} in {tz} (Ana busy 9:00-10:00, Ben busy 9:30-10:30):");
    for s in slots(&q) {
        let who: Vec<&str> = s
            .free_staff
            .iter()
            .map(|id| if *id == ana { "Ana" } else { "Ben" })
            .collect();
        println!(
            "  {} - {}  free: {}",
            s.start.with_timezone(&tz).format("%H:%M"),
            s.end.with_timezone(&tz).format("%H:%M"),
            who.join(", ")
        );
    }
}
