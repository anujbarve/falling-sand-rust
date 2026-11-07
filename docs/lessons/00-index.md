# Rust from zero: build the ecological sandbox

These first five modules start a fresh learning project called `game/`. They build toward one sand cell falling onto a floor. Existing code in `archive/` is reference history, not prerequisite reading.

This is a curriculum draft, not approval to implement the full game. The game design still requires milestone-plan approval before implementation. Learners create the small teaching project as exercises; these files do not create it automatically.

| Module | Work you finish | Rust you need |
|---|---|---|
| [01 — Setup](01-setup.md) | Run your own program | Terminal, Cargo, source, `main` |
| [02 — Variables](02-variables.md) | Describe and update a cell in the terminal | `let`, `mut`, numbers, types, arithmetic |
| [03 — Window](03-window.md) | Open and redraw a night-coloured window | Dependencies, function calls, loops |
| [04 — One cell](04-cell-and-borrowing.md) | Draw a sand cell and reset its position | Structs, ownership, shared and mutable borrowing |
| [05 — Falling](05-falling-on-ticks.md) | Drop that cell onto a floor at 20 ticks/second | Conditions, functions, elapsed time, bounded movement |

Aim for one small working result per sitting; split a module whenever a prerequisite needs more time. Borrowing and ticking may each take several sittings. Progress is demonstrated by predicting and changing the program, not by finishing a page.

## How to use these lessons with a tutor

At the start, ask which operating system the learner is using and whether they have used a terminal or any programming language. The goal is already known: learn Rust from zero while building this game.

Each lesson supplies a starting probe, not a complete diagnosis. Ask one question at a time and wait. Grade only after the learner answers; use `0. I don't know` as an honest gap. Adapt follow-up questions until relevant existing knowledge and gaps are clear. Never reveal a quiz answer alongside its question.

Then present a small lesson plan and dependency map and wait for agreement before teaching. For every concept: motivate the problem, establish it from what the learner understands, connect it to the current program, then check with another question. Follow the workspace `teach` skill during the live lesson. Use the `visualize` skill when a picture will clarify a concept; do not add unverified diagrams just for decoration.

The pages below are the preparation and exercise sequence. Reveal examples progressively during a live session rather than asking the learner to copy a whole file without understanding it. Finish by asking the learner to make the exit-check change unaided. No answer key is included before their attempts.

## Boundaries of the first five modules

Use macroquad 0.4.16, matching the game design. The first window uses low-DPI rendering so a 4-unit rectangle is a 4-physical-pixel cell. The final high-DPI art buffer, resizing, viewport budget and camera come later. Do not advertise this window as the production renderer.

Module 05 implements a single falling cell with the game's simplified downward rule, not physical acceleration. It has a fixed 20 TPS clock at 1x only. Fast-forward, random ticks, chunks, material grids and threading wait until there is a working foundation to extend. This is progress toward M0, not completion of M0 or its performance acceptance criteria.

After module 05, the natural next slice is a small grid with air/sand/stone, occupancy checks, mouse painting and multiple falling cells.
