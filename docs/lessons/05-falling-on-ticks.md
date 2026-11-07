# 05 — Make the cell fall on ticks and stop at a floor

**Result:** one sand cell falls at 20 ticks per second and lands without crossing the floor. **Prerequisite:** module 04, including mutable borrowing.

## Start with a probe

Suppose we add one to the row every rendered frame. One computer draws 30 frames per second; another draws 120. After one second, which falls farther?

1. The 120 FPS computer.
2. Both travel equally far.
3. The 30 FPS computer.
0. I don't know.

Wait and explore the learner's reasoning. The problem motivates ticks: drawing frequency should not decide the cell's falling rate.

## Plan the lesson

First implement one downward step as a function. Then prevent it crossing a floor. Finally call it from an elapsed-time clock, independently of drawing. Confirm those three stages before adding code.

## Work: one update

Keep `Cell`, `draw_cell`, `reset_cell` and `window_conf` from module 04. Add:

```rust
fn tick_cell(cell: &mut Cell, floor_row: i32) {
    if cell.row < floor_row - 1 {
        cell.row = (cell.row + 3).min(floor_row - 1);
    }
}
```

Call this once before the frame loop and inspect the new position. Then remove that trial call. Make sure the learner can predict the function for rows 30, 158 and 159 when `floor_row` is 160.

The row identifies the cell's top edge. A floor starting at row 160 allows the grain's top edge to reach row 159. Clamping the proposed destination prevents crossing this horizontal floor. This only handles one flat floor: a material grid will later check every traversed cell to prevent tunnelling through arbitrary obstacles.

The rule moves up to three cells per tick, matching the game design's simplified downward movement. It is constant-speed falling, not an acceleration model.

## Work: connect time to updates

Replace the entry function with this version:

```rust
#[macroquad::main(window_conf)]
async fn main() {
    let mut cell = Cell { column: 160, row: 5 };
    let floor_row = 160;
    let tick_period = 1.0_f64 / 20.0;
    let mut elapsed = 0.0_f64;

    loop {
        if is_key_pressed(KeyCode::R) {
            reset_cell(&mut cell);
            elapsed = 0.0;
        }

        elapsed += f64::from(get_frame_time());
        let mut ticks_this_frame = 0;
        while elapsed >= tick_period && ticks_this_frame < 2 {
            tick_cell(&mut cell, floor_row);
            elapsed -= tick_period;
            ticks_this_frame += 1;
        }
        if elapsed >= tick_period {
            elapsed %= tick_period;
        }

        clear_background(Color::from_rgba(15, 26, 51, 255));
        draw_rectangle(
            0.0,
            floor_row as f32 * 4.0,
            screen_width(),
            screen_height() - floor_row as f32 * 4.0,
            Color::from_rgba(29, 48, 80, 255),
        );
        draw_cell(&cell);
        next_frame().await;
    }
}
```

Introduce the clock in pieces. `get_frame_time` supplies elapsed seconds since the last frame. The accumulator remembers time that has not yet paid for a tick. `while` allows multiple due updates; subtracting the period retains the fractional remainder. At 1x, the period is $1/20 = 0.05$ seconds.

The two-tick cap bounds catch-up work in this tiny demo. Excess whole-tick debt is deliberately discarded, so a long stall slows the demo instead of causing endless catch-up. This is a teaching clock with floating-point elapsed seconds, no calendar and no fast-forward. Production will add the spec's integer world/step counters, time budget, pause behavior and multiplier policy; this demo is not a replacement for that scheduler.

## Checks and exercises

1. Run `cargo fmt` and `cargo check`, then `cargo run`. The cell must stop with its bottom edge touching the floor; R must restart the fall.
2. Predict the falling speed: 3 cells/tick at 20 ticks/second gives 60 cells/second. At 4 pixels/cell, that is 240 pixels/second until landing. From row 5 to row 159, landing takes 52 ticks, about 2.6 seconds of simulated time.
3. Move the floor up by ten rows. Change only `floor_row`; verify both the floor drawing and landing position move together.
4. Ask the learner to trace two frames of 0.03 seconds each: how many ticks have run, and how much time remains? Then trace a long frame and the cap. Grade after their attempt.
5. Change rendering load or use a debugger stall. Ordinary frame variation should not tie movement directly to FPS; after a long stall, discarded debt should not keep the cell racing to catch up.

## Exit check

Explain the difference between updating `cell.row` and drawing the cell. Implement a new `restart_cell` position without changing the tick function. Demonstrate that neither an overshooting last step nor repeated ticks push the grain through the floor.

You now have the smallest visible piece of the sandbox: state, drawing, borrowed mutation and scheduled movement. Multiple cells, occupancy and painting are the next slice; chunks and full M0 performance proof remain ahead.

Sources: [macroquad timing API](https://docs.rs/macroquad/0.4.16/macroquad/time/index.html), [Rust control flow](https://doc.rust-lang.org/book/ch03-05-control-flow.html), [game design](../superpowers/specs/2026-11-06-game-design.md).
