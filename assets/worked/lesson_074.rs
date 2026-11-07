const HEADER_LEN: usize = 14;
fn encode(seed: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(HEADER_LEN);
    bytes.extend_from_slice(b"RSTY");
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&seed.to_le_bytes());
    bytes
}
fn decode(bytes: &[u8]) -> Result<u64, &'static str> {
    if bytes.len() != HEADER_LEN { return Err("invalid header length"); }
    if &bytes[0..4] != b"RSTY" { return Err("wrong file family"); }
    let version = u16::from_le_bytes([bytes[4],bytes[5]]);
    if version != 1 { return Err("unsupported format version"); }
    let mut seed = [0_u8;8];
    seed.copy_from_slice(&bytes[6..14]);
    Ok(u64::from_le_bytes(seed))
}
fn main() {
    let bytes = encode(42);
    assert_eq!(bytes.len(),14);
    assert_eq!(decode(&bytes),Ok(42));
    for end in 0..14 { assert!(decode(&bytes[..end]).is_err()); }
    println!("header round-trip and truncation checks pass");
}
