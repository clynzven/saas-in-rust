//! The availability engine: which start times can a customer book?
//!
//! It's a pure function. No database, no clock: everything it needs is passed
//! in. That makes it easy to test exhaustively, including daylight saving.

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;
use uuid::Uuid;

/// Someone is already busy from `start` (inclusive) to `end` (exclusive).
#[derive(Clone, Debug)]
pub struct Busy {
    pub staff_id: Uuid,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

pub struct Query<'a> {
    /// The day the customer is looking at, in the business's calendar.
    pub date: NaiveDate,
    pub tz: Tz,
    /// Opening hours for that weekday, or None if closed.
    pub hours: Option<(NaiveTime, NaiveTime)>,
    pub duration: TimeDelta,
    /// Offer a start time every `step` (e.g. every 15 minutes).
    pub step: TimeDelta,
    /// Nothing before this instant: now plus the minimum notice.
    pub not_before: DateTime<Utc>,
    pub staff: &'a [Uuid],
    pub busy: &'a [Busy],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    /// Staff who are free for the whole slot. Never empty.
    pub free_staff: Vec<Uuid>,
}

/// Turn a wall-clock time into an instant. In a daylight-saving gap the time
/// doesn't exist (None); in an overlap it happens twice and we take the first.
pub fn to_utc(tz: Tz, local: NaiveDateTime) -> Option<DateTime<Utc>> {
    tz.from_local_datetime(&local)
        .earliest()
        .map(|t| t.with_timezone(&Utc))
}

pub fn slots(q: &Query) -> Vec<Slot> {
    let Some((opens, closes)) = q.hours else {
        return vec![];
    };
    let Some(close_at) = to_utc(q.tz, q.date.and_time(closes)) else {
        return vec![];
    };

    let mut out = Vec::new();
    let mut local = q.date.and_time(opens);
    while local < q.date.and_time(closes) {
        if let Some(start) = to_utc(q.tz, local) {
            // Durations are real elapsed time, so a slot crossing a clock change is still 30 minutes long.
            let end = start + q.duration;
            if end <= close_at && start >= q.not_before {
                let free_staff: Vec<Uuid> = q
                    .staff
                    .iter()
                    .copied()
                    .filter(|id| {
                        !q.busy
                            .iter()
                            .any(|b| b.staff_id == *id && b.start < end && start < b.end)
                    })
                    .collect();
                if !free_staff.is_empty() {
                    out.push(Slot {
                        start,
                        end,
                        free_staff,
                    });
                }
            }
        }
        local += q.step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn utc(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn query<'a>(staff: &'a [Uuid], busy: &'a [Busy]) -> Query<'a> {
        Query {
            date: NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
            tz: chrono_tz::UTC,
            hours: Some((t(9, 0), t(11, 0))),
            duration: TimeDelta::minutes(30),
            step: TimeDelta::minutes(30),
            not_before: utc("2026-01-01T00:00:00Z"),
            staff,
            busy,
        }
    }

    fn starts(s: &[Slot]) -> Vec<String> {
        s.iter()
            .map(|s| s.start.format("%H:%M").to_string())
            .collect()
    }

    #[test]
    fn whole_morning_free() {
        let staff = [Uuid::new_v4()];
        assert_eq!(
            starts(&slots(&query(&staff, &[]))),
            ["09:00", "09:30", "10:00", "10:30"]
        );
    }

    #[test]
    fn closed_days_have_no_slots() {
        let staff = [Uuid::new_v4()];
        let q = Query {
            hours: None,
            ..query(&staff, &[])
        };
        assert!(slots(&q).is_empty());
    }

    #[test]
    fn busy_time_is_skipped_per_staff_member() {
        let (ana, ben) = (Uuid::new_v4(), Uuid::new_v4());
        let busy = [Busy {
            staff_id: ana,
            start: utc("2026-12-01T09:15:00Z"),
            end: utc("2026-12-01T10:00:00Z"),
        }];
        // Ana alone: 09:00 and 09:30 overlap her booking.
        assert_eq!(starts(&slots(&query(&[ana], &busy))), ["10:00", "10:30"]);
        // With Ben on the team every slot is still bookable, with him.
        let both = slots(&query(&[ana, ben], &busy));
        assert_eq!(both.len(), 4);
        assert_eq!(both[0].free_staff, [ben]);
    }

    #[test]
    fn minimum_notice() {
        let staff = [Uuid::new_v4()];
        let q = Query {
            not_before: utc("2026-12-01T09:45:00Z"),
            ..query(&staff, &[])
        };
        assert_eq!(starts(&slots(&q)), ["10:00", "10:30"]);
    }

    #[test]
    fn long_services_must_finish_before_closing() {
        let staff = [Uuid::new_v4()];
        let q = Query {
            duration: TimeDelta::minutes(90),
            ..query(&staff, &[])
        };
        assert_eq!(starts(&slots(&q)), ["09:00", "09:30"]);
    }

    #[test]
    fn spring_forward_in_auckland() {
        // On 27 Sep 2026 clocks in New Zealand jump from 02:00 to 03:00.
        let staff = [Uuid::new_v4()];
        let q = Query {
            date: NaiveDate::from_ymd_opt(2026, 9, 27).unwrap(),
            tz: chrono_tz::Pacific::Auckland,
            hours: Some((t(1, 0), t(4, 0))),
            ..query(&staff, &[])
        };
        let local: Vec<String> = slots(&q)
            .iter()
            .map(|s| s.start.with_timezone(&q.tz).format("%H:%M").to_string())
            .collect();
        // 02:00 and 02:30 don't exist that night, so they're never offered.
        assert_eq!(local, ["01:00", "01:30", "03:00", "03:30"]);
        // 01:30 + 30 real minutes = 03:00 on the clock.
        assert_eq!(
            slots(&q)[1]
                .end
                .with_timezone(&q.tz)
                .format("%H:%M")
                .to_string(),
            "03:00"
        );
    }

    #[test]
    fn fall_back_in_auckland() {
        // On 5 Apr 2026 clocks go from 03:00 back to 02:00: 02:xx happens twice.
        let staff = [Uuid::new_v4()];
        let q = Query {
            date: NaiveDate::from_ymd_opt(2026, 4, 5).unwrap(),
            tz: chrono_tz::Pacific::Auckland,
            hours: Some((t(1, 0), t(4, 0))),
            ..query(&staff, &[])
        };
        let s = slots(&q);
        // Six wall-clock starts; each is a distinct instant, strictly increasing.
        assert_eq!(s.len(), 6);
        assert!(s.windows(2).all(|w| w[0].start < w[1].start));
    }

    proptest! {
        /// Whatever the bookings, no slot ever overlaps a busy period of a
        /// staff member it lists, and every slot fits inside opening hours.
        #[test]
        fn slots_never_clash(
            busy_spec in proptest::collection::vec((0usize..3, 0i64..600, 5i64..120), 0..12),
            duration in prop_oneof![Just(15i64), Just(30), Just(45), Just(60), Just(90)],
            opens_h in 6u32..12,
            len_h in 1u32..10,
        ) {
            let staff = [Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)];
            let day = utc("2026-12-01T00:00:00Z");
            let busy: Vec<Busy> = busy_spec.iter().map(|&(who, start_min, len)| Busy {
                staff_id: staff[who],
                start: day + TimeDelta::minutes(start_min + 360),
                end: day + TimeDelta::minutes(start_min + 360 + len),
            }).collect();
            let q = Query {
                hours: Some((t(opens_h, 0), t((opens_h + len_h).min(23), 0))),
                duration: TimeDelta::minutes(duration),
                step: TimeDelta::minutes(15),
                ..query(&staff, &busy)
            };
            let open_at = day + TimeDelta::hours(opens_h.into());
            let close_at = day + TimeDelta::hours((opens_h + len_h).min(23).into());
            for s in slots(&q) {
                prop_assert!(s.start >= open_at && s.end <= close_at);
                prop_assert_eq!(s.end - s.start, TimeDelta::minutes(duration));
                prop_assert!(!s.free_staff.is_empty());
                for id in &s.free_staff {
                    prop_assert!(!busy.iter().any(|b| b.staff_id == *id && b.start < s.end && s.start < b.end));
                }
            }
        }
    }
}
