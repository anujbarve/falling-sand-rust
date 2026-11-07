fn hash(seed:u64, x:i64)->f64 {
    let mut z=seed ^ (x as u64).wrapping_mul(0x9e3779b97f4a7c15);
    z=(z^(z>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z=(z^(z>>27)).wrapping_mul(0x94d049bb133111eb);
    z ^= z>>31;
    (z>>11) as f64 / ((1_u64<<53) as f64)
}
fn height(seed:u64,x:i64)->f64 {
    let cell=x.div_euclid(64);
    let t=x.rem_euclid(64) as f64/64.0;
    let eased=t*t*(3.0-2.0*t);
    let a=hash(seed,cell); let b=hash(seed,cell+1);
    100.0+40.0*(a+(b-a)*eased)
}
#[test]
fn terrain_is_addressed_by_coordinate() {
    let forward:Vec<_>=(-65..65).map(|x|height(7,x)).collect();
    let mut reverse:Vec<_>=(-65..65).rev().map(|x|height(7,x)).collect();
    reverse.reverse();
    assert_eq!(forward,reverse);
    assert!(height(7,-1).is_finite());
}
