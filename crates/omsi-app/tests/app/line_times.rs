use openomsi_game::testing::line_times::*;

fn wait(c: &TimesCache, date: i32) -> std::sync::Arc<LineTimes> {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(t) = c.get(date) {
            return t;
        }
        assert!(std::time::Instant::now() < end, "the read never arrived");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn the_timetable_is_read_in_a_thread_once_per_date() {
    let reads = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let n = reads.clone();
    let c = TimesCache::new(move |_| {
        n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(30));
        LineTimes::default()
    });
    assert!(c.peek(20260101).is_none(), "peek never starts a read");
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(c.get(20260101).is_none(), "the first call does not wait");
    let a = wait(&c, 20260101);
    let b = c.get(20260101).expect("cached");
    assert!(
        std::sync::Arc::ptr_eq(&a, &b),
        "the same object, no new read"
    );
    assert!(c.peek(20260101).is_some());
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
    // another date is read again
    assert!(c.peek(20260102).is_none());
    let _ = wait(&c, 20260102);
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn a_read_that_dies_leaves_an_empty_timetable_for_its_date() {
    let c = TimesCache::new(|_| panic!("a broken file"));
    let t = wait(&c, 20260101);
    assert!(t.line("1").is_none());
    let again = c.get(20260101).expect("not tried again");
    assert!(std::sync::Arc::ptr_eq(&t, &again));
}

#[test]
fn an_answer_for_a_past_date_is_dropped_and_the_current_one_asked() {
    let c = TimesCache::new(|d| {
        if d == 1 {
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        LineTimes::default()
    });
    assert!(c.get(1).is_none());
    assert!(c.get(2).is_none(), "date 1 is still being read");
    let _ = wait(&c, 2);
    assert!(c.peek(1).is_none());
}

#[test]
fn two_departures_at_one_time_are_told_apart_by_their_trip() {
    let (a, b) = (
        Dep {
            t: 3600.0,
            mask: 1023,
            trip: 0,
            profile: 0,
        },
        Dep {
            t: 3600.0,
            mask: 1023,
            trip: 1,
            profile: 0,
        },
    );
    let pick = b.key();
    assert!(!a.is(&pick) && b.is(&pick));
    // a bare time is the first departure at that time
    let bare = DepKey {
        t: 3600,
        trip: None,
        profile: None,
    };
    assert!(a.is(&bare) && b.is(&bare));
    assert!(!Dep { t: 3660.0, ..a }.is(&bare));
}

#[test]
fn two_profiles_of_one_trip_at_one_time_are_told_apart() {
    let (a, b) = (
        Dep {
            t: 3600.0,
            mask: 1023,
            trip: 0,
            profile: 0,
        },
        Dep {
            t: 3600.0,
            mask: 1023,
            trip: 0,
            profile: 1,
        },
    );
    let pick = b.key();
    assert!(!a.is(&pick) && b.is(&pick));
}

#[test]
fn the_school_period_bit_follows_the_holiday_ranges() {
    let cal = omsi_map::Calendar {
        ranges: vec![omsi_map::HolidayRange {
            start: 20260701,
            end: 20260731,
            name: "summer".into(),
        }],
        holidays: vec![],
    };
    assert_eq!(
        period_bit(&cal, 20260715),
        1 << 8,
        "inside a range: school holidays"
    );
    assert_eq!(period_bit(&cal, 20260901), 1 << 9, "outside: term time");
    assert_eq!(
        period_bit(&cal, 0),
        (1 << 8) | (1 << 9),
        "no date: nothing is left out"
    );
    // a tour of the other period is left out, one for both (the default mask) is kept
    assert_eq!(0b11_0000_0000 & period_bit(&cal, 20260715), 1 << 8);
    assert_eq!((1 << 9 | 0b1111111) & period_bit(&cal, 20260715), 0);
    assert_ne!(ALL_DAYS & period_bit(&cal, 20260715), 0);
}
