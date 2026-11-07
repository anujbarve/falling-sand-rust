fn exchange(a:i32,b:i32)->(i32,i32) {
    let transfer=(a-b)/4;
    (a-transfer,b+transfer)
}
fn main() {
    for (a,b) in [(100,0),(0,100),(20,20),(23,20)] {
        let (next_a,next_b)=exchange(a,b);
        assert_eq!(next_a+next_b,a+b);
        assert!((next_a-next_b).abs()<=(a-b).abs());
        println!("({a},{b}) -> ({next_a},{next_b})");
    }
}
