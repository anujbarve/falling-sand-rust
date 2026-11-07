use std::collections::HashSet;
fn picks(start:usize,stride:usize,count:usize)->Vec<usize> {
    assert!(stride%2==1 && count<=4096);
    (0..count).map(|i|(start.wrapping_add(i.wrapping_mul(stride)))&4095).collect()
}
#[test]
fn odd_stride_produces_distinct_chunk_picks() {
    for stride in [1,3,17,4095] {
        let result=picks(81,stride,256);
        assert_eq!(result.iter().copied().collect::<HashSet<_>>().len(),256);
        assert!(result.iter().all(|&i|i<4096));
    }
}
