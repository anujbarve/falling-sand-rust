fn can_regrow(soil:bool, light:bool, moist:bool,
              grass_neighbour:bool, dormant_root:bool)->bool {
    soil && light && moist && (grass_neighbour || dormant_root)
}
#[test]
fn recovery_needs_a_source_and_good_conditions() {
    assert!(!can_regrow(true,true,true,false,false));
    assert!(can_regrow(true,true,true,false,true));
    assert!(!can_regrow(true,false,true,false,true));
    assert!(!can_regrow(true,true,false,false,true));
}
