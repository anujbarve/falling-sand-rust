use std::collections::{HashSet, VecDeque};
fn connected(start: usize, rigid: &[bool], limit: usize) -> (usize, bool) {
    if start >= rigid.len() || !rigid[start] { return (0,false); }
    let mut queue = VecDeque::from([start]);
    let mut seen = HashSet::new();
    while let Some(i) = queue.pop_front() {
        if !seen.insert(i) { continue; }
        if seen.len() > limit { return (seen.len(),true); }
        if i > 0 && rigid[i-1] { queue.push_back(i-1); }
        if i+1 < rigid.len() && rigid[i+1] { queue.push_back(i+1); }
    }
    (seen.len(),false)
}
#[test]
fn loops_do_not_duplicate_group_members() {
    assert_eq!(connected(1, &[true,true,true,false,true], 10), (3,false));
    assert_eq!(connected(1, &[true,true,true], 2), (3,true));
}
