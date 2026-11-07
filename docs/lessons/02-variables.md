# 02 — Give a cell some state

**Result:** the terminal reports a sand cell's position before and after one downward step. **Prerequisite:** module 01.

## Start with a probe

A program must remember where a grain is before updating its position. Where should that position live while the program runs?

1. In the terminal window's title.
2. In a value stored by the program.
3. In the dependency version number.
0. I don't know.

Wait, then probe whether the learner can predict assignment and arithmetic. Do not assume knowledge from another language.

## Plan the lesson

Start with named values, then changing a value, then types and a coordinate convention. Our reason for learning variables is concrete: the game needs to remember where material is.

## Work

Replace the terminal example in `src/main.rs` with this small experiment, adding one statement at a time:

```rust
fn main() {
    let column: i32 = 12;
    let mut row: i32 = 5;
    let cell_size: i32 = 4;

    println!("Before: column={column}, row={row}");
    row += 1;
    println!("After: column={column}, row={row}");
    println!("Screen position: {}, {}", column * cell_size, row * cell_size);
}
```

Before running, predict all three lines. Discuss `let`, the binding name, `: i32`, `=`, and the ending semicolon separately. `i32` is a signed 32-bit integer type. `let mut` allows reassignment; without `mut`, this reassignment is rejected. `+= 1` adds one and stores the result back.

Try removing `mut`, read the resulting error, then repair it. Next try changing `column` as well: decide where another `mut` is necessary.

## Coordinates for this teaching slice

A cell coordinate counts grid positions. A screen coordinate counts drawing pixels in our low-DPI window. Multiplying by 4 converts one cell into a 4-pixel-wide block.

For these early screen/grid examples, column increases rightward and row increases downward, starting at the top left. This is a temporary renderer convention. The game design uses world altitude increasing upward; we will add an explicit conversion when world coordinates arrive. Increasing `row` currently means falling.

## Exit check

Make the cell fall three rows using one arithmetic statement. Predict and then verify its screen position. Change cell size temporarily to see which output changes, then restore 4 before continuing.

Explain why `row` needs `mut` and `cell_size` does not. If that answer is unclear, practise another numeric example before introducing a window.

Next: [03 — Window](03-window.md).

Source: [Variables and mutability](https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html).
