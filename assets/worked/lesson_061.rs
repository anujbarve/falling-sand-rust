fn seep(upper:&mut u8,lower:&mut u8,capacity:u8,limit:u8)->u8 {
    let free=capacity.saturating_sub(*lower);
    let moved=(*upper).min(free).min(limit);
    *upper-=moved; *lower+=moved; moved
}
#[test]
fn paired_seepage_conserves_and_respects_capacity() {
    let (mut a,mut b)=(80,49);
    assert_eq!(seep(&mut a,&mut b,50,4),1);
    assert_eq!((a,b),(79,50));
    assert_eq!(u16::from(a)+u16::from(b),129);
    assert_eq!(seep(&mut a,&mut b,0,4),0);
}
