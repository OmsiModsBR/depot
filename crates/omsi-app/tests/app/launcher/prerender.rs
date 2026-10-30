use omsi_launcher_ui::vehicles::PrerenderPhase;
use openomsi_game::testing::launcher::prerender::*;
use openomsi_game::testing::showroom::Look;
use std::time::Duration;
use std::time::Instant;

#[test]
fn a_run_with_nothing_to_look_at_ends_at_once_and_cancel_goes_back_to_idle() {
    let mut p = Prerender::default();
    assert_eq!(p.status().phase, PrerenderPhase::Idle);
    p.looking();
    assert_eq!(p.status().phase, PrerenderPhase::Looking);
    assert!(p.after_content);
    p.start(&[], Look::default());
    let start = Instant::now();
    while p.status().phase == PrerenderPhase::Looking && start.elapsed().as_secs() < 5 {
        p.update(0.016, false);
        std::thread::yield_now();
    }
    assert_eq!(p.status().phase, PrerenderPhase::Done);
    assert_eq!(p.status().total, 0);
    p.looking();
    p.cancel();
    assert_eq!(p.status().phase, PrerenderPhase::Idle);
    assert!(!p.after_content);
}

#[test]
fn a_run_waits_for_the_reading_it_asked_for_and_drops_an_empty_one() {
    let mut p = Prerender::default();
    // (nothing asked: nothing to start)
    assert!(!p.content_ready(false, false));
    p.looking();
    // the old list is not the new one: while no reading was seen, the run waits
    assert!(!p.content_ready(false, false));
    assert!(p.after_content);
    assert!(!p.content_ready(true, false));
    assert!(p.content_ready(false, false));
    assert!(!p.after_content);
    // a reading that ends with no vehicles drops the request, and the page stops saying
    // it looks
    p.looking();
    assert!(!p.content_ready(true, true));
    assert!(!p.content_ready(false, true));
    assert!(!p.after_content);
    assert_eq!(p.status().phase, PrerenderPhase::Idle);
    // a reading that never starts drops it too
    p.looking();
    p.asked_at = Instant::now() - READING_STARTS - Duration::from_secs(1);
    assert!(!p.content_ready(false, false));
    assert_eq!(p.status().phase, PrerenderPhase::Idle);
}

#[test]
fn a_game_stops_the_helper_and_the_picture_in_flight_starts_its_time_again() {
    let mut p = Prerender::default();
    p.phase = PrerenderPhase::Running;
    let look = Look {
        bus: "Vehicles/A/a.bus".into(),
        ..Default::default()
    };
    p.current = Some((look, Instant::now() - PER_PICTURE - Duration::from_secs(60)));
    p.update(0.016, true);
    assert!(p.paused);
    assert_eq!(p.status().phase, PrerenderPhase::Running);
    // the game is over: the time of the picture is counted from now, not given up on
    p.update(0.016, false);
    assert_eq!(p.status().failed, 0);
    assert!(p.current.as_ref().unwrap().1.elapsed() < Duration::from_secs(5));
}
