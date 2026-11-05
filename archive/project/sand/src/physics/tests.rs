//! Tests for the physics engine: CPU vs GPU equality, conservation, physics checks.

use super::*;
use cpu::CpuSim;

    fn run(world: &mut World, steps: usize) {
        let mut sim = CpuSim::new(world);
        for _ in 0..steps {
            sim.step(world);
        }
    }

    #[test]
    fn single_thread_and_rayon_give_identical_worlds() {
        let mut a = scene(4, 256, 160);
        let mut b = a.clone();
        let mut sim_a = CpuSim::new(&a);
        let mut sim_b = CpuSim::new(&b);
        for _ in 0..200 {
            sim_a.step(&mut a);
            sim_b.step_parallel(&mut b);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn gpu_gives_identical_world_to_cpu() {
        let mut cpu_world = scene(4, 256, 160);
        let mut gpu_world = cpu_world.clone();
        let mut gpu = match gpu::GpuSim::new(&gpu_world) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                return;
            }
        };
        run(&mut cpu_world, 200);
        gpu.step(200);
        gpu.download(&mut gpu_world);
        assert_eq!(cpu_world.frame, gpu_world.frame);
        let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
        assert_eq!(diff, 0, "{diff} cells differ between CPU and GPU");
    }

    fn gpu_or_skip(world: &World) -> Option<gpu::GpuSim> {
        match gpu::GpuSim::new(world) {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                None
            }
        }
    }

    /// Poll `f` (which never blocks) until it returns Some, for up to 5 s.
    fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> Option<T> {
        let start = std::time::Instant::now();
        loop {
            if let Some(v) = f() {
                return Some(v);
            }
            if start.elapsed() > std::time::Duration::from_secs(5) {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn per_pass_times_are_reported_when_timestamps_exist() {
        let world = World::new(256, 256);
        let Some(mut gpu) = gpu_or_skip(&world) else { return };
        if !gpu.has_timestamps() {
            return;
        }
        gpu.step(200);
        gpu.wait();
        let t = wait_for(|| gpu.pass_times_ms()).expect("timestamps arrive");
        assert!(t.step_ms > 0.0 && t.wake_ms >= 0.0);
    }

    /// A scene that reaches a true fixed point quickly: still water filling
    /// whole rows of a tank, and a block of sand that falls into a walled
    /// bin and lands flat (a free heap would creep down its slope for many
    /// seconds of world time).
    fn settling_scene(w: usize, h: usize) -> World {
        let mut g = World::new(w, h);
        let (wi, hi) = (w as i32, h as i32);
        let floor = hi - 1;
        g.fill(wi / 2, hi / 2, wi / 2 + 3, floor, STONE);
        g.fill(wi - 3, hi / 2, wi, floor, STONE);
        g.fill(wi / 2 + 3, hi * 3 / 4, wi - 3, floor, WATER);
        g.fill(wi / 8 - 3, hi / 4, wi / 8, floor, STONE);
        g.fill(wi / 3, hi / 4, wi / 3 + 3, floor, STONE);
        g.fill(wi / 8, hi / 4, wi / 3, hi / 3, SAND);
        g
    }

    #[test]
    fn block_can_move_is_never_wrong() {
        // If block_can_move says no, move_block must leave the block alone
        // for every random number.
        let mut seen_stuck = 0;
        for s in 0..20_000u32 {
            let cells: [u32; 4] = std::array::from_fn(|k| {
                let r = hash3(s, k as u32, 77);
                let m = r % MATERIAL_COUNT as u32;
                // Half the cells at rest, the rest with a random speed/life.
                let a = if r & (1 << 31) != 0 { 0 } else { pcg(r) & 255 };
                pack(m, a, AMBIENT + ((r >> 8) % 4096) as i32 * 4 - 8192)
            });
            if block_can_move(&cells) {
                continue;
            }
            seen_stuck += 1;
            for t in 0..64 {
                let mut moved = cells;
                move_block(&mut moved, hash3(s, t, 5));
                assert_eq!(moved, cells, "block said stuck but moved: {cells:x?}");
            }
        }
        assert!(seen_stuck > 1000);
    }

    #[test]
    fn sleeping_gives_identical_world_to_no_sleeping() {
        // Odd sizes on purpose: partial chunks at the right and bottom edges.
        for index in 0..SCENES.len() {
            let mut reference = scene(index, 257, 161);
            let mut world = reference.clone();
            let mut plain = CpuSim::new(&reference);
            let mut sim = CpuSim::new(&world);
            for step in 0..800 {
                if step == 500 {
                    // Paint behind the simulator's back, like the viewer does.
                    for w in [&mut world, &mut reference] {
                        w.fill(20, 10, 30, 20, WATER);
                        w.fill(200, 100, 205, 104, LAVA);
                    }
                }
                plain.step_no_sleep(&mut reference);
                if step % 3 == 0 { sim.step_parallel(&mut world) } else { sim.step(&mut world) }
                if step % 100 == 99 {
                    assert_eq!(world, reference, "scene {index}, step {step}");
                }
            }
        }
    }

    #[test]
    fn gpu_gives_identical_world_to_cpu_at_odd_size() {
        for index in [0, 4] {
            let mut cpu_world = scene(index, 257, 161);
            let mut gpu_world = cpu_world.clone();
            let Some(mut gpu) = gpu_or_skip(&gpu_world) else { return };
            let mut sim = CpuSim::new(&cpu_world);
            for _ in 0..1500 {
                sim.step_no_sleep(&mut cpu_world);
            }
            gpu.step(1500);
            gpu.download(&mut gpu_world);
            let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
            assert_eq!(diff, 0, "scene {index}: {diff} cells differ between CPU and GPU");
            assert_eq!(cpu_world.frame, gpu_world.frame);
        }
    }

    #[test]
    fn settled_scene_sleeps_on_cpu_and_gpu() {
        let start = settling_scene(512, 320);
        let mut world = start.clone();
        let mut sim = CpuSim::new(&world);
        let total = sim.chunk_count();
        assert_eq!(total, 16 * 10);
        assert_eq!(sim.awake_chunks(), total, "everything starts awake");
        for _ in 0..9000 {
            sim.step_parallel(&mut world);
        }
        let cpu_awake = sim.awake_chunks();
        assert!(cpu_awake * 10 < total, "only {cpu_awake} of {total} chunks should still be awake");

        // Exactly what an unsleeping run gives.
        let mut reference = start.clone();
        let mut plain = CpuSim::new(&reference);
        for _ in 0..9000 {
            plain.step_no_sleep(&mut reference);
        }
        assert_eq!(world, reference);

        // The GPU agrees, cell for cell and chunk for chunk.
        let Some(mut gpu) = gpu_or_skip(&start) else { return };
        assert_eq!(gpu.chunk_count(), total);
        gpu.step(9000);
        let mut gpu_world = start.clone();
        gpu.download(&mut gpu_world);
        assert_eq!(gpu_world, world);
        assert_eq!(gpu.awake_chunks(), cpu_awake);
        // A fresh upload wakes everything again.
        gpu.upload(&gpu_world);
        assert_eq!(gpu.awake_chunks(), total);
    }

    #[test]
    fn pipelined_readback_and_paint_match_cpu() {
        let mut cpu_world = scene(4, 256, 160);
        let mut shown = cpu_world.clone();
        let mut gpu = match gpu::GpuSim::new(&cpu_world) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                return;
            }
        };
        let mut sim = CpuSim::new(&cpu_world);
        for round in 0..20u32 {
            // Paint the same cells on both sides: a stripe of fire on row 10.
            if round == 7 {
                let edits: Vec<(usize, u32)> =
                    (40..60).map(|x| (10 * 256 + x, pack(FIRE, default_aux(FIRE), default_temp(FIRE)))).collect();
                for &(i, c) in &edits {
                    cpu_world.cells[i] = c;
                }
                gpu.write_cells(&edits);
            }
            for _ in 0..5 {
                sim.step(&mut cpu_world);
            }
            gpu.step_and_readback(5);
            gpu.latest(&mut shown); // may or may not have something new: never blocks
            assert!(shown.frame <= cpu_world.frame);
        }
        gpu.wait();
        assert!(gpu.latest(&mut shown).is_some(), "the last readback should have arrived");
        assert_eq!(shown.frame, cpu_world.frame);
        assert_eq!(shown, cpu_world);

        // An upload makes readbacks still in flight stale: they must not show.
        gpu.step_and_readback(3);
        let fresh = scene(0, 256, 160);
        gpu.upload(&fresh);
        gpu.wait();
        let before = shown.clone();
        assert!(gpu.latest(&mut shown).is_none(), "a readback from before the upload was shown");
        assert_eq!(shown, before);
        gpu.step_and_readback(0);
        gpu.wait();
        assert!(gpu.latest(&mut shown).is_some());
        assert_eq!(shown, fresh);

        if gpu.has_timestamps() {
            gpu.step(10);
            gpu.wait();
            let ms = wait_for(|| gpu.gpu_time_ms()).expect("timestamps should come back after a wait");
            assert!(ms > 0.0 && ms < 1000.0, "implausible GPU time {ms} ms/step");
        }
    }

    #[test]
    fn pipelined_readback_rect_matches_full_readback_inside_rect() {
        let start = scene(4, 300, 200);
        let mut gpu = match gpu::GpuSim::new(&start) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                return;
            }
        };
        gpu.step(100);
        let mut full = World::new(300, 200);
        gpu.download(&mut full);

        // An interior rectangle, a rectangle that is the whole grid (direct copy
        // path), and one that touches the bottom-right corner.
        for rect in [(37u32, 20u32, 120u32, 90u32), (0, 0, 300, 200), (299, 199, 1, 1)] {
            let (x0, y0, w, h) = rect;
            let mut shown = World::new(300, 200);
            gpu.step_and_readback_rect(0, rect);
            gpu.wait();
            let got = gpu.latest(&mut shown).expect("the rectangle readback should have arrived");
            assert_eq!(got, rect, "latest should return the rectangle it copied");
            assert_eq!(shown.frame, full.frame);
            for y in y0..y0 + h {
                for x in x0..x0 + w {
                    let i = (y * 300 + x) as usize;
                    assert_eq!(shown.cells[i], full.cells[i], "cell ({x},{y}) differs in rect {rect:?}");
                }
            }
        }
    }

    /// The GPU works in tiles; sizes that are odd or not a multiple of the
    /// tile size exercise the tile and grid edges, where bugs hide.
    #[test]
    fn gpu_matches_cpu_on_odd_sizes() {
        for (w, h, scene_index) in [(257, 161, 4), (100, 37, 4), (33, 18, 1), (64, 16, 2), (3, 3, 0)] {
            let start = scene(scene_index, w, h);
            let mut gpu = match gpu::GpuSim::new(&start) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("no GPU adapter, skipping: {e}");
                    return;
                }
            };
            let mut cpu_world = start.clone();
            let mut gpu_world = start.clone();
            let mut sim = CpuSim::new(&cpu_world);
            // Check after an odd and an even number of steps (both offsets).
            for steps in [1, 300] {
                for _ in 0..steps {
                    sim.step(&mut cpu_world);
                }
                gpu.step(steps);
                gpu.download(&mut gpu_world);
                assert_eq!(cpu_world.frame, gpu_world.frame);
                let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
                assert_eq!(diff, 0, "{w}x{h}: {diff} cells differ between CPU and GPU at frame {}", cpu_world.frame);
            }
        }
    }

    #[test]
    fn conduction_conserves_heat() {
        // Only stone (no phase change below 1200°C, no gas cooling).
        let mut world = World::new(64, 64);
        world.fill(0, 0, 64, 64, STONE);
        for y in 20..30 {
            for x in 20..30 {
                world.cells[y * 64 + x] = pack(STONE, 0, celsius(1000));
            }
        }
        let total = |w: &World| w.cells.iter().map(|&c| temp(c) as i64).sum::<i64>();
        let before = total(&world);
        run(&mut world, STEPS_PER_SECOND as usize); // 1 s
        let after = total(&world);
        assert_eq!(before, after, "random rounding must still conserve heat exactly");
        // Real granite is slow: in 1 s heat only gets about one cell (5 mm) out.
        let next_to_hot = temp(world.cells[64 * 25 + 19]);
        assert!(next_to_hot > AMBIENT + celsius(5), "the neighbour should warm up (got {next_to_hot})");
        assert_eq!(temp(world.cells[64 * 25 + 5]), AMBIENT, "but not 15 cells away");
    }

    #[test]
    fn oil_floats_on_water() {
        // Start upside down: water on top of oil.
        let mut world = World::new(32, 64);
        world.fill(0, 20, 32, 43, WATER);
        world.fill(0, 43, 32, 63, OIL);
        run(&mut world, 10 * STEPS_PER_SECOND as usize); // 10 s
        let mean_y = |m: u32| {
            let ys: Vec<usize> = (0..world.cells.len()).filter(|&i| material(world.cells[i]) == m).map(|i| i / 32).collect();
            ys.iter().sum::<usize>() as f64 / ys.len() as f64
        };
        assert!(mean_y(OIL) < mean_y(WATER), "oil should end up above water");
    }

    #[test]
    fn lava_heats_water_and_turns_to_stone() {
        // Real 1150°C lava quenches in water: it shatters into gravel or
        // sets into stone within seconds, heating the water around it.
        // (Real sea-entry steam comes from boiling in a sub-millimetre film
        // at the lava surface, which 5 mm cells do not resolve.)
        let mut world = scene(1, 200, 120);
        let mut sim = CpuSim::new(&world);
        let mut hottest_water = AMBIENT;
        for _ in 0..12 * STEPS_PER_SECOND {
            sim.step(&mut world);
            for &c in &world.cells {
                if material(c) == WATER {
                    hottest_water = hottest_water.max(temp(c));
                }
            }
        }
        let hottest = hottest_water / T_SCALE;
        assert!(hottest > 60, "water touching lava should get hot (hottest {hottest}°C)");
        let start = scene(1, 200, 120);
        assert!(world.count(LAVA) < start.count(LAVA), "lava should solidify");
        assert!(world.count(STONE) + world.count(SAND) > start.count(STONE), "into stone or gravel");
    }

    #[test]
    fn submerged_lava_boils_surrounding_water() {
        let mut world = World::new(80, 60);
        world.fill(0, 10, 80, 59, WATER);
        world.fill(28, 20, 52, 47, LAVA);
        let mut sim = CpuSim::new(&world);
        for _ in 0..STEPS_PER_SECOND {
            sim.step(&mut world);
        }
        assert!(world.count(STEAM) > 100, "submerged lava should produce a visible steam cloud within one second");
        assert!(world.count(LAVA) > 0, "lava should not vanish instantly");
    }

    #[test]
    fn ice_melts_in_hot_water() {
        // A 5 mm ice cell in 80°C water: melting interface ~1500 W/m²K, so it
        // takes ~10-20 s to take up its fusion heat. In 1 s it is still ice.
        let mut world = World::new(16, 16);
        world.fill(0, 0, 16, 15, WATER);
        for c in world.cells.iter_mut() {
            if material(*c) == WATER {
                *c = pack(WATER, 0, celsius(80));
            }
        }
        world.set(8, 8, ICE);
        let mut sim = CpuSim::new(&world);
        let mut melted_at = None;
        for step in 1..=40 * STEPS_PER_SECOND {
            sim.step(&mut world);
            if melted_at.is_none() && world.count(ICE) == 0 {
                melted_at = Some(step as f64 / STEPS_PER_SECOND as f64);
            }
        }
        let t = melted_at.expect("the ice should melt within 40 s");
        assert!(t > 1.0, "melting should take its fusion heat, not be instant (melted at {t:.1} s)");
    }

    #[test]
    fn boiling_water_reads_at_most_100c() {
        // Latent heat sits in boiling water's field; its real temperature is 100°C.
        let world_cell = pack(WATER, 0, celsius(100) + LATENT / 2);
        assert_eq!(sens(world_cell), celsius(100));
        assert_eq!(sens(pack(WATER, 0, -LATENT_F / 2)), 0, "freezing water is at 0°C");
        assert_eq!(sens(pack(STEAM, 0, celsius(100) - LATENT / 2)), celsius(100), "wet steam is at 100°C");
        assert_eq!(sens(pack(STEAM, 0, celsius(140))), celsius(140), "superheated steam");
        assert_eq!(sens(pack(ICE, 0, LATENT_F / 2)), 0, "melting ice is at 0°C");
    }

    #[test]
    fn lava_at_bottom_keeps_water_boiling_through_hot_crust() {
        let mut world = World::new(120, 90);
        world.fill(0, 20, 120, 89, WATER);
        world.fill(40, 66, 80, 89, LAVA);
        world.fill(48, 58, 72, 66, LAVA);
        let mut sim = CpuSim::new(&world);
        // After the first burst, heat comes through the rock crust (~12 W per
        // crust cell), enough for ~1.7 steam parcels per second, each of which
        // condenses in ~2 s. So a few parcels exist at any time: check that
        // boiling never stops, rather than a fixed count.
        for _ in 0..2 * STEPS_PER_SECOND {
            sim.step(&mut world);
        }
        let mut steps_with_steam = 0;
        for _ in 0..STEPS_PER_SECOND {
            sim.step(&mut world);
            steps_with_steam += (world.count(STEAM) > 0) as u32;
        }
        assert!(
            steps_with_steam * 10 >= STEPS_PER_SECOND * 8,
            "hot crust around bottom lava should keep making steam (steam present in {steps_with_steam} of {STEPS_PER_SECOND} steps)"
        );
        assert!(world.count(LAVA) > 0, "the lava under the crust is still molten");
    }

    #[test]
    fn lava_ignites_wood() {
        // A painted flame lasts seconds and does not light a plank (true in
        // real life too). Lava sitting on wood mostly radiates into it and
        // gets its surface to 300°C within seconds.
        let mut world = World::new(64, 64);
        world.fill(10, 40, 50, 63, WOOD);
        world.fill(20, 30, 40, 40, LAVA);
        let wood_before = world.count(WOOD);
        run(&mut world, 10 * STEPS_PER_SECOND as usize);
        assert!(world.count(WOOD) < wood_before, "the wood should burn within 10 s");
    }

    #[test]
    fn heat_between_different_materials_is_conserved_on_average() {
        // Hot stone in cold water, no gas anywhere (gases lose heat to the
        // room on purpose). Energy = sum of heat capacity * temperature.
        let mut world = World::new(64, 64);
        world.fill(0, 0, 64, 64, WATER);
        world.fill(24, 24, 40, 40, STONE);
        for y in 24..40 {
            for x in 24..40 {
                world.cells[y * 64 + x] = pack(STONE, 0, celsius(90));
            }
        }
        let energy = |w: &World| {
            w.cells.iter().map(|&c| CAP[material(c) as usize] as f64 * (temp(c) - AMBIENT) as f64).sum::<f64>()
        };
        let before = energy(&world);
        run(&mut world, 5 * STEPS_PER_SECOND as usize);
        let after = energy(&world);
        let drift = (after - before).abs() / before;
        assert!(drift < 0.01, "energy drifted by {:.2}%", drift * 100.0);
        // And it really moved: the water next to the stone warmed up.
        // (Slowly: a 5 mm water cell beside 90°C rock has a time constant of
        // ~100 s, so it gains only a few degrees in 5 s.)
        assert!(temp(world.cells[32 * 64 + 23]) > celsius(21), "water beside the stone should warm");
    }

    #[test]
    fn water_heated_from_below_turns_over() {
        // Hot water under cold water. Conduction alone can only even the
        // two out; with thermal expansion the hot (lighter) water rises, so
        // the top ends up warmer than the bottom.
        let mut world = World::new(32, 66);
        world.fill(0, 1, 32, 65, WATER);
        for y in 33..65 {
            for x in 0..32 {
                world.cells[y * 32 + x] = pack(WATER, 0, celsius(80));
            }
        }
        run(&mut world, 10 * STEPS_PER_SECOND as usize);
        let mean = |rows: std::ops::Range<usize>| {
            let n = rows.len() * 32;
            rows.flat_map(|y| (0..32).map(move |x| y * 32 + x)).map(|i| temp(world.cells[i]) as f64).sum::<f64>()
                / n as f64
                / T_SCALE as f64
        };
        let (top, bottom) = (mean(1..33), mean(33..65));
        assert!(top > bottom + 10.0, "top {top:.1}°C should be warmer than bottom {bottom:.1}°C");
    }

    #[test]
    fn lava_in_open_air_crusts_over() {
        // Radiation to the surroundings cools exposed lava; air alone would
        // take far longer. Lava resting on the floor sets into stone.
        let mut world = World::new(64, 64);
        world.fill(16, 54, 48, 63, LAVA);
        let stone_before = world.count(STONE);
        run(&mut world, 30 * STEPS_PER_SECOND as usize);
        assert!(world.count(STONE) > stone_before + 20, "a crust should form within 30 s");
    }

    /// Steps until a grain dropped from rest reaches the floor, averaged
    /// over several grains (the speed-up is random rounding).
    fn mean_landing_step(world: &mut World, xs: &[usize], landing_y: usize, max_steps: usize) -> f64 {
        let w = world.width;
        let mut sim = CpuSim::new(world);
        let mut landed = vec![None; xs.len()];
        for step in 1..=max_steps {
            sim.step(world);
            for (k, &x) in xs.iter().enumerate() {
                if landed[k].is_none() && material(world.cells[landing_y * w + x]) == SAND {
                    landed[k] = Some(step);
                }
            }
            if landed.iter().all(|l| l.is_some()) {
                break;
            }
        }
        let steps: Vec<usize> = landed.iter().map(|l| l.expect("every grain should land")).collect();
        steps.iter().sum::<usize>() as f64 / steps.len() as f64
    }

    #[test]
    fn free_fall_takes_the_right_time() {
        // Drop grains 238 cells (1.19 m) through air: t = sqrt(2h/g) = 0.493 s.
        let mut world = World::new(64, 260);
        let xs: Vec<usize> = (4..64).step_by(8).collect();
        for &x in &xs {
            world.set(x as i32, 20, SAND);
        }
        let h = (258 - 20) as f64 * CELL_METERS;
        let expected = (2.0 * h / GRAVITY).sqrt() * STEPS_PER_SECOND as f64;
        let got = mean_landing_step(&mut world, &xs, 258, 3000);
        let err = (got - expected).abs() / expected;
        assert!(err < 0.1, "fell in {got:.0} steps, expected {expected:.0} ({:.1}% off)", err * 100.0);
    }

    #[test]
    fn sand_sinks_slowly_through_water() {
        // Through 0.6 m of water at its ~0.1 m/s terminal speed: about 6.4 s,
        // where the same drop through air takes 0.35 s.
        let mut world = World::new(32, 142);
        world.fill(0, 20, 32, 141, WATER);
        let xs = [8, 16, 24];
        for &x in &xs {
            world.set(x as i32, 19, SAND);
        }
        let got = mean_landing_step(&mut world, &xs, 140, 20 * STEPS_PER_SECOND as usize) / STEPS_PER_SECOND as f64;
        assert!((4.5..9.0).contains(&got), "sank in {got:.2} s");
    }

    #[test]
    fn phase_transition_latent_heat() {
        // Boiling water stores latent heat above 100°C: it only turns to steam
        // after absorbing LATENT (541 K of water heat), and steam keeps the
        // temperature it had after giving that heat up.
        let mut cells = vec![pack(WATER, 0, celsius(100))];
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), WATER, "water at 100°C has not yet absorbed its latent heat");

        cells[0] = pack(WATER, 0, celsius(100) + LATENT);
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), STEAM);
        assert_eq!(temp(out), celsius(100));

        // Freezing: water at 0°C stays water until it has lost its fusion heat (80 K).
        cells[0] = pack(WATER, 0, celsius(-2));
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), WATER, "water at -2 is still in its freezing plateau");

        cells[0] = pack(WATER, 0, -LATENT_F - 1);
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), ICE);
        assert_eq!(temp(out), -1, "ice holds the fusion heat as an offset");

        // Melting: ice at 0°C stays ice until it has absorbed its fusion heat.
        cells[0] = pack(ICE, 0, 1);
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), ICE, "melting ice is still ice");
        assert_eq!(sens(out), 0);
        cells[0] = pack(ICE, 0, LATENT_F + 1);
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), WATER);
        assert_eq!(temp(out), 1);

        // A steam parcel stays steam (at 100°C) while it gives back its latent
        // heat; once it has given it all back it is water again.
        cells[0] = pack(STEAM, 0, celsius(49));
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), STEAM, "a wet steam parcel at field 49°C still holds latent heat");
        assert_eq!(sens(out), celsius(100));
        cells[0] = pack(STEAM, 0, celsius(100) - LATENT - celsius(1));
        let (out, _) = heat_cell(&cells, 1, 1, 0, 0, 0);
        assert_eq!(material(out), WATER);
        assert!(temp(out) <= celsius(99) && temp(out) >= celsius(98), "condensed water at ~99°C");
    }

    #[test]
    fn fire_requires_oxygen_and_suffocates_underground() {
        // Fire surrounded by stone (no AIR or LAVA) suffocates into SMOKE
        let buried_cells = vec![
            pack(STONE, 0, AMBIENT), pack(STONE, 0, AMBIENT), pack(STONE, 0, AMBIENT),
            pack(STONE, 0, AMBIENT), pack(FIRE, 50, celsius(900)), pack(STONE, 0, AMBIENT),
            pack(STONE, 0, AMBIENT), pack(STONE, 0, AMBIENT), pack(STONE, 0, AMBIENT),
        ];
        let (suffocated, _) = heat_cell(&buried_cells, 3, 3, 1, 1, 0);
        assert_eq!(material(suffocated), SMOKE, "fire deprived of air/oxygen must extinguish into smoke");

        // Wood buried without air does not ignite even at high temperature
        let buried_hot_wood = vec![
            pack(STONE, 0, celsius(400)),
            pack(WOOD, 0, celsius(350)),
            pack(STONE, 0, celsius(400)),
        ];
        let (wood_out, _) = heat_cell(&buried_hot_wood, 3, 1, 1, 0, 0);
        assert_eq!(material(wood_out), WOOD, "wood without oxygen/air must not ignite despite T >= 300°C");
    }

    #[test]
    fn fast_sand_deflects_down_incline() {
        let mut world = World::new(32, 40);
        // Fill a solid wedge down and to the left on odd coordinate steps
        for i in 0..15 {
            for y in (21 + i)..40 {
                world.set(25 - i, y, STONE);
            }
        }
        let start_x = 21;
        let start_y = 20;
        world.cells[start_y * 32 + start_x] = pack(SAND, 150, AMBIENT);

        let mut sim = CpuSim::new(&world);
        for _ in 0..600 {
            sim.step(&mut world);
        }

        let final_pos = (0..world.cells.len()).find(|&i| material(world.cells[i]) == SAND).expect("sand grain must exist");
        let fx = final_pos % 32;
        let fy = final_pos / 32;

        assert!(fx < start_x, "fast sand hitting slope must deflect down the slope (x < start_x)");
        assert!(fy > start_y, "sand must progress downward along the incline");
    }

    #[test]
    fn steam_expansion_burst_and_obstacle_venting() {
        let mut world = World::new(32, 40);
        // Stone ceiling from x=0 to 18 at y=15 (gap at x=19..32)
        for x in 0..18 {
            world.set(x, 15, STONE);
        }
        // Place steam trapped under ceiling at (14..18, 16..17)
        for y in 16..18 {
            for x in 14..18 {
                world.cells[y * 32 + x] = pack(STEAM, 85, celsius(120));
            }
        }

        let mut sim = CpuSim::new(&world);
        for _ in 0..600 {
            sim.step(&mut world);
        }

        let steam_above_ceiling = (0..world.cells.len())
            .filter(|&i| {
                let y = i / 32;
                y < 15 && material(world.cells[i]) == STEAM
            })
            .count();

        assert!(
            steam_above_ceiling > 0,
            "steam trapped under obstacle must expand, divert diagonally, and vent past the ceiling gap into upper space"
        );
    }

// ---------------------------------------------------------------------------
// Jets, exhaust, freezers, metal and coal.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod new_materials {
    use super::*;

    fn set(w: &mut World, x: usize, y: usize, m: u32, a: u32, t: i32) {
        let i = y * w.width + x;
        w.cells[i] = pack(m, a, t);
    }

    fn cells_of(w: &World, m: u32) -> Vec<(usize, usize, u32)> {
        (0..w.cells.len())
            .filter(|&i| material(w.cells[i]) == m)
            .map(|i| (i % w.width, i / w.width, w.cells[i]))
            .collect()
    }

    #[test]
    fn jet_exhaust_only_goes_where_the_nozzle_points() {
        for d in 0..4u32 {
            // Big enough that the stream does not reach the walls in 150 steps.
            let mut w = World::new(121, 121);
            set(&mut w, 60, 60, JET, d, JET_T);
            run(&mut w, 150);
            let (dx, dy) = dir_step(d);
            let ex = cells_of(&w, EXHAUST);
            assert!(ex.len() > 25, "direction {d}: exhaust should stream out, got {}", ex.len());
            for &(x, y, _) in &ex {
                let forward = (x as i32 - 60) * dx + (y as i32 - 60) * dy;
                assert!(forward > 0, "direction {d}: exhaust at ({x},{y}) is not ahead of the nozzle");
            }
            // The nozzle itself never moves or changes its direction.
            assert_eq!(material(w.cells[60 * 121 + 60]), JET);
            assert_eq!(aux(w.cells[60 * 121 + 60]) & 3, d);
        }
    }

    #[test]
    fn jet_makes_a_dense_continuous_plume() {
        // A steady right-facing gas jet: along its axis, most cells near the
        // nozzle hold flame, so the plume is a stream, not dots.
        let mut w = World::new(121, 121);
        set(&mut w, 60, 60, JET, 1, JET_T);
        run(&mut w, 300);
        let filled = (65..=90).filter(|&x| material(w.cells[60 * 121 + x]) == EXHAUST).count();
        assert!(filled >= 13, "plume along the axis: {filled} of 26 cells hold exhaust");
    }

    #[test]
    fn jet_runs_forever_and_exhaust_dies_when_it_stops() {
        let mut w = World::new(41, 41);
        set(&mut w, 20, 20, JET, 1, JET_T);
        run(&mut w, 1200 * 10);
        assert!(!cells_of(&w, EXHAUST).is_empty(), "a jet keeps exhausting for as long as it runs");
        // Stop the jet: the exhaust counts down its life and cools to smoke.
        set(&mut w, 20, 20, AIR, 0, AMBIENT);
        run(&mut w, 1200 * 6);
        assert!(cells_of(&w, EXHAUST).is_empty(), "exhaust is gone once its life runs out");
    }

    #[test]
    fn freezer_and_jet_hold_their_temperature() {
        let mut w = World::new(40, 40);
        w.fill(4, 4, 8, 8, FREEZER);
        for y in 4..=8 {
            for x in 4..=8 {
                set(&mut w, x, y, FREEZER, 0, FREEZER_T);
            }
        }
        // Hot lava right next to the freezer and a jet: they must not drift.
        w.fill(9, 4, 14, 8, LAVA);
        set(&mut w, 20, 20, JET, 0, JET_T);
        run(&mut w, 2000);
        let fz = w.cells[6 * 40 + 6];
        assert_eq!(material(fz), FREEZER);
        assert_eq!(temp(fz), FREEZER_T, "a freezer is a fixed cold source");
        assert_eq!(temp(w.cells[20 * 40 + 20]), JET_T, "a jet nozzle is a fixed hot source");
    }

    #[test]
    fn metal_carries_heat_much_faster_than_stone() {
        // Pure conduction: a solid block with no air around it. Steel's
        // diffusivity is about ten times stone's, so after 20 s heat has
        // reached 3 cells (1.5 cm) in steel and barely 1 cell in stone.
        let mut at = |m: u32| {
            let mut w = World::new(16, 16);
            for c in w.cells.iter_mut() {
                *c = pack(m, 0, AMBIENT);
            }
            for y in 0..16 {
                w.cells[y * 16] = pack(m, 0, celsius(900));
            }
            run(&mut w, 24_000);
            temp(w.cells[8 * 16 + 3]) as f64 / T_SCALE as f64
        };
        let (metal, stone) = (at(METAL), at(STONE));
        assert!(metal > stone + 10.0, "metal 1.5 cm along: {metal:.1}°C, stone: {stone:.1}°C");
    }

    #[test]
    fn freezer_freezes_a_sealed_water_pool() {
        // A pool on a freezer plate, walled in so it cannot slide away. A
        // cold plate in water is slow (about 20 s to the first ice at 10x contact).
        let mut w = World::new(30, 30);
        w.fill(0, 0, 1, 30, STONE);
        w.fill(29, 0, 30, 30, STONE);
        w.fill(0, 29, 30, 30, STONE);
        w.fill(1, 24, 29, 29, FREEZER);
        w.fill(1, 16, 29, 24, WATER);
        let mut sim = CpuSim::new(&w);
        for _ in 0..40 * STEPS_PER_SECOND {
            sim.step(&mut w);
        }
        assert!(w.count(ICE) > 0, "a freezer plate freezes the water on it within 40 s");
        assert_eq!(temp(w.cells[24 * 30 + 5]), FREEZER_T, "the plate stays at -30°C");
    }

    #[test]
    fn coal_lights_next_to_fire_and_burns() {
        let mut w = World::new(20, 20);
        set(&mut w, 10, 10, COAL, 0, celsius(360));
        set(&mut w, 11, 10, FIRE, 40, celsius(900));
        run(&mut w, 200);
        assert_eq!(material(w.cells[10 * 20 + 10]), FIRE, "hot coal next to fire catches");
    }

    #[test]
    fn gpu_matches_cpu_with_jets_freezers_metal_and_coal() {
        let mut cpu_world = World::new(96, 64);
        for (d, (x, y)) in [(0u32, (20usize, 40usize)), (1, (40, 30)), (2, (60, 20)), (3, (76, 44))] {
            set(&mut cpu_world, x, y, JET, d, JET_T);
        }
        for y in 50..56 {
            for x in 4..16 {
                set(&mut cpu_world, x, y, FREEZER, 0, FREEZER_T);
            }
        }
        cpu_world.fill(20, 56, 60, 60, METAL);
        for x in 22..30 {
            set(&mut cpu_world, x, 50, COAL, 0, celsius(400));
        }
        set(&mut cpu_world, 31, 50, FIRE, 60, celsius(900));
        cpu_world.fill(0, 8, 95, 10, WATER);
        cpu_world.fill(6, 44, 14, 50, WATER); // resting on the freezer plate
        cpu_world.fill(48, 14, 52, 20, LAVA);
        let mut gpu_world = cpu_world.clone();
        let mut gpu = match gpu::GpuSim::new(&gpu_world) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("no GPU adapter, skipping: {e}");
                return;
            }
        };
        run(&mut cpu_world, 600);
        gpu.step(600);
        gpu.download(&mut gpu_world);
        let diff = (0..cpu_world.cells.len()).filter(|&i| cpu_world.cells[i] != gpu_world.cells[i]).count();
        assert_eq!(diff, 0, "{diff} cells differ between CPU and GPU");
        assert!(!cells_of(&cpu_world, EXHAUST).is_empty(), "the scene really ran jets");
    }
}



