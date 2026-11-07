fn choose_side(left:Option<usize>,right:Option<usize>,tick:u64)->Option<usize> {
    let (first,second)=if tick%2==0 {(left,right)} else {(right,left)};
    first.or(second)
}
fn main() {
    assert_eq!(choose_side(Some(3),Some(5),0),Some(3));
    assert_eq!(choose_side(Some(3),Some(5),1),Some(5));
    assert_eq!(choose_side(None,Some(5),0),Some(5));
    assert_eq!(choose_side(None,None,1),None);
    println!("deterministic tie-break verified");
}
