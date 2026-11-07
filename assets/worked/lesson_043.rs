#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell { material: u8, temperature: i16 }
fn canonical_digest(cells: &[Cell]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for cell in cells {
        for byte in [cell.material, cell.temperature.to_le_bytes()[0],
                     cell.temperature.to_le_bytes()[1]] {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}
#[test]
fn state_fields_affect_the_digest() {
    let a = [Cell { material: 1, temperature: 20 }];
    let b = [Cell { material: 1, temperature: 21 }];
    assert_eq!(canonical_digest(&a), canonical_digest(&a));
    assert_ne!(canonical_digest(&a), canonical_digest(&b));
}
