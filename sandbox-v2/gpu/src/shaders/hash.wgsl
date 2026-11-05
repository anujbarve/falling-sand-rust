// Per-cell hash shared by rules.wgsl and lava.wgsl. Mirrors
// reference::cell_hash (murmur3 fmix32 chain, u32 wrapping math).
fn mix32(v: u32) -> u32 {
    var h = v;
    h ^= h >> 16u;
    h = h * 0x85EBCA6Bu;
    h ^= h >> 13u;
    h = h * 0xC2B2AE35u;
    return h ^ (h >> 16u);
}

// Per-cell hash of (tick, x, y, seed). Bit 0: diagonal side; bit 1: sideways.
fn cell_hash(tick: u32, x: u32, y: u32, seed: u32) -> u32 {
    var h = mix32(seed ^ tick);
    h = mix32(h ^ x);
    return mix32(h ^ y);
}
