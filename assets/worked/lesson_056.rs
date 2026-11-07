const DAY:u64=24_000;
const SEASON:u64=3*DAY;
const YEAR:u64=4*SEASON;
fn calendar(tick:u64)->(u64,u64,u64) {
    (tick/YEAR, (tick/SEASON)%4, tick%DAY)
}
#[test]
fn exact_calendar_boundaries() {
    assert_eq!(calendar(0),(0,0,0));
    assert_eq!(calendar(SEASON-1),(0,0,DAY-1));
    assert_eq!(calendar(SEASON),(0,1,0));
    assert_eq!(calendar(YEAR),(1,0,0));
}
