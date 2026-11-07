fn ceil_div(a:u64,b:u64)->u64 { a/b + u64::from(a%b != 0) }
fn allocation(mut world:u64, mut ticks:u64)->Vec<u64> {
    assert!(ticks>0 && world>=ticks);
    let mut result=Vec::new();
    while ticks>0 {
        let m=ceil_div(world,ticks);
        result.push(m); world-=m; ticks-=1;
    }
    result
}
#[test]
fn integer_allocations_preserve_the_sum() {
    assert_eq!(allocation(10,3),vec![4,3,3]);
    assert_eq!(allocation(3,3),vec![1,1,1]);
    let plan=allocation(10,3);
    let spent=plan[0];
    assert_eq!(10-spent,6);
}
