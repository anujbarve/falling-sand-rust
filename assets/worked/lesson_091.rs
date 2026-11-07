fn next_eruption(seed: u64, feature: u64, now: u64) -> Option<u64> {
    const PERIOD: u64 = 72_000;
    let offset = (seed ^ feature.wrapping_mul(0x9E3779B97F4A7C15)) % PERIOD;
    let start = (now / PERIOD).checked_mul(PERIOD)?;
    let candidate = start.checked_add(offset)?;
    if candidate > now { Some(candidate) }
    else { candidate.checked_add(PERIOD) }
}
fn main() {
    let seed = 17;
    let id = 42;
    let first = next_eruption(seed,id,0).unwrap();
    let after_absence = next_eruption(seed,id,72_000 * 10).unwrap();
    assert!(after_absence > 72_000 * 10);
    assert_eq!(next_eruption(seed,id,0),Some(first));
    assert_eq!(next_eruption(seed,id,first),first.checked_add(72_000));
    println!("one stable schedule; no historical eruption queue");
}
