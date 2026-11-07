# 04 — Draw one cell; let functions borrow it

**Result:** a sand-coloured 4 × 4 pixel cell; pressing R resets its row. **Prerequisite:** module 03.

## Start with a probe

The cell is at column 12, row 5, with a size of 4 pixels. Where is its top-left drawing position?

1. `(12, 5)`.
2. `(48, 20)`.
3. `(16, 9)`.
0. I don't know.

Wait and establish coordinate conversion before adding borrowing. Later probe whether passing a value into a function necessarily leaves it available to the caller; distinguish prior knowledge of copying and moving.

## Plan the lesson

Group coordinates into one cell, draw it, then give another function permission to reset it. Learn ownership and borrowing because drawing and simulation both need access to the same cell.

## Work

Keep the imports and `window_conf` from module 03. Above `main`, add:

```rust
struct Cell {
    column: i32,
    row: i32,
}

fn draw_cell(cell: &Cell) {
    let size = 4.0;
    draw_rectangle(
        cell.column as f32 * size,
        cell.row as f32 * size,
        size,
        size,
        Color::from_rgba(226, 182, 121, 255),
    );
}

fn reset_cell(cell: &mut Cell) {
    cell.row = 5;
}
```

Replace the attributed entry function with:

```rust
#[macroquad::main(window_conf)]
async fn main() {
    let mut cell = Cell { column: 160, row: 30 };

    loop {
        if is_key_pressed(KeyCode::R) {
            reset_cell(&mut cell);
        }

        clear_background(Color::from_rgba(15, 26, 51, 255));
        draw_cell(&cell);
        next_frame().await;
    }
}
```

A simulation cell is this 4 × 4 physical-pixel block in the low-DPI window. It is not a single physical pixel. The art style's smaller art pixels and crest details come later.

## Build the understanding

`Cell` is a struct: one value with named fields. Here `main` owns the value bound to `cell`. Our custom struct has not implemented `Copy`; passing it by value to a function would move it. Drawing should inspect it while keeping it available for the next frame, so `&Cell` supplies a shared borrow.

Resetting must change the original, so it receives `&mut Cell`. The owner is mutable and lends mutable access for that call. Shared borrows allow reading; a mutable borrow gives exclusive access while it is in use. Do not teach that references always last until the end of the enclosing block: Rust can end a borrow after its last use.

Introduce `as f32` only at the drawing boundary: grid coordinates remain integers, while this drawing API accepts floating-point arguments. Dot syntax accesses the borrowed cell's fields.

First check the shared borrow, then the mutable borrow. Try assigning `cell.row = 10` inside `draw_cell` and inspect the compiler error; undo it. Do not fix it by making every function take a mutable borrow.

## Exit check

Change reset so it also sets the column to 10. Predict where the cell appears before and after R. Explain why drawing accepts `&Cell`, reset accepts `&mut Cell`, and the original still exists after both calls.

Next: [05 — Falling on ticks](05-falling-on-ticks.md).

Sources: [Ownership](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html), [References and borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html), [draw_rectangle](https://docs.rs/macroquad/0.4.16/macroquad/shapes/fn.draw_rectangle.html).
