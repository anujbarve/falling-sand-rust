fn encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i=0;
    while i<data.len() {
        let value=data[i]; let mut count=1_usize;
        while i+count<data.len() && data[i+count]==value && count<255 { count+=1; }
        out.extend_from_slice(&[count as u8,value]); i+=count;
    }
    out
}
fn decode(data: &[u8], max: usize) -> Result<Vec<u8>, &'static str> {
    if data.len()%2!=0 { return Err("truncated run"); }
    let mut out=Vec::new();
    for pair in data.chunks_exact(2) {
        let count=pair[0] as usize;
        if count==0 { return Err("empty run"); }
        let end=out.len().checked_add(count).ok_or("length overflow")?;
        if end>max { return Err("output limit"); }
        out.resize(end,pair[1]);
    }
    Ok(out)
}
fn main() {
    let input=vec![7;4096]; let packed=encode(&input);
    assert_eq!(decode(&packed,4096),Ok(input));
    assert!(decode(&packed,4095).is_err());
    assert!(decode(&[1],4096).is_err());
    println!("4096 uniform bytes -> {} bytes",packed.len());
}
