fn next_row(row:i32,floor_row:i32)->i32 {
    if row<floor_row-1 {(row+3).min(floor_row-1)} else {row}
}
#[cfg(test)]
mod tests {
    use super::next_row;
    #[test]
    fn falls_and_stops_without_crossing_floor() {
        assert_eq!(next_row(5,160),8);
        assert_eq!(next_row(158,160),159);
        assert_eq!(next_row(159,160),159);
    }
}
