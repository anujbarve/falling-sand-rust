fn effective_probability(p:f64,m:f64)->f64 {
    assert!((0.0..=1.0).contains(&p) && m>=1.0);
    1.0-(1.0-p).powf(m)
}
fn stage_progress(current:u16, increment:u16, cap:u16)->u16 {
    current.saturating_add(increment).min(cap)
}
#[test]
fn scaled_updates_keep_bounds() {
    assert_eq!(effective_probability(0.0,32.0),0.0);
    assert_eq!(effective_probability(1.0,32.0),1.0);
    assert!((effective_probability(0.5,2.0)-0.75).abs()<1e-12);
    assert_eq!(stage_progress(240,40,255),255);
}
