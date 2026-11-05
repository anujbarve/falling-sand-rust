// Fire pass: ignition and burn countdown, in place on dst (each cell reads
// only itself). Mirrors reference_thermal::ThermalReference::fire.
@compute @workgroup_size(8, 8)
fn stage_fire(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= params.w || gid.y >= params.h) { return; }
    let i = gid.y * params.w + gid.x;
    var c = dst[i];
    let burn = BURN_TICKS[tid(c.mat)];
    if (burn != 0u && c.temp >= IGNITE_K) {
        c.fuel = f32(burn);
        c.mat = FIRE;
        c.temp = max(c.temp, FIRE_K);
    } else if (c.mat == FIRE) {
        if (!(c.fuel >= 1.0)) {
            c.fuel = FIRE_TICKS;
            c.temp = max(c.temp, FIRE_K);
        } else {
            c.fuel -= 1.0;
            if (c.fuel < 1.0) {
                c.mat = ASH;
                c.fuel = 0.0;
            } else {
                c.temp = max(c.temp, FIRE_K);
            }
        }
    } else {
        return;
    }
    dst[i] = c;
}
