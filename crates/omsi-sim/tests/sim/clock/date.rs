use omsi_sim::testing::clock::*;

#[test]
fn dates_from_elsewhere_stay_in_range() {
    let mut c = SimClock::default();
    c.set_date(2024, 3, 1);
    assert_eq!((c.year, c.day_of_year), (2024, 61));
    c.set_date(2023, 2, 31);
    assert_eq!(c.day_of_year, 59, "the day kept within its month");
    c.set_date(i32::MAX, i32::MAX, i32::MAX);
    assert!(c.day_of_year <= 366 && c.year <= 9999);
}
