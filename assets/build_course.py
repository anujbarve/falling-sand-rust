#!/usr/bin/env python3
"""Build the offline teaching documents. Python standard library only."""
from pathlib import Path
import html
import json
import re
from collections import defaultdict
from deep_render import load_deep_lessons, render_deep_lesson

ROOT = Path(__file__).resolve().parent.parent
E = html.escape
COURSE = json.loads((ROOT / 'assets/course.json').read_text())
DEEP = load_deep_lessons()
SOURCES = {
    'install': ('Install Rust', 'https://doc.rust-lang.org/book/ch01-01-installation.html'),
    'cargo': ('Cargo workflow', 'https://doc.rust-lang.org/book/ch01-03-hello-cargo.html'),
    'variables': ('Variables and mutability', 'https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html'),
    'types': ('Rust data types', 'https://doc.rust-lang.org/book/ch03-02-data-types.html'),
    'functions': ('Functions and expressions', 'https://doc.rust-lang.org/book/ch03-03-how-functions-work.html'),
    'control': ('Control flow', 'https://doc.rust-lang.org/book/ch03-05-control-flow.html'),
    'ownership': ('Ownership and moves', 'https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html'),
    'borrowing': ('References and borrowing', 'https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html'),
    'structs': ('Defining structs', 'https://doc.rust-lang.org/book/ch05-01-defining-structs.html'),
    'enums': ('Enums and matching', 'https://doc.rust-lang.org/book/ch06-00-enums.html'),
    'option': ('Option and match', 'https://doc.rust-lang.org/book/ch06-02-match.html'),
    'vec': ('Vectors and their access rules', 'https://doc.rust-lang.org/book/ch08-01-vectors.html'),
    'collections': ('Hash maps', 'https://doc.rust-lang.org/book/ch08-03-hash-maps.html'),
    'modules': ('Rust modules and visibility', 'https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html'),
    'testing': ('Writing tests', 'https://doc.rust-lang.org/book/ch11-01-writing-tests.html'),
    'integers': ('i32 Euclidean division', 'https://doc.rust-lang.org/std/primitive.i32.html#method.div_euclid'),
    'instant': ('Measuring elapsed time', 'https://doc.rust-lang.org/std/time/struct.Instant.html'),
    'threads': ('Scoped threads', 'https://doc.rust-lang.org/std/thread/fn.scope.html'),
    'slices': ('Disjoint mutable slices', 'https://doc.rust-lang.org/std/primitive.slice.html#method.split_at_mut'),
    'traits': ('Traits as shared behavior', 'https://doc.rust-lang.org/book/ch10-02-traits.html'),
    'channels': ('Messages between threads', 'https://doc.rust-lang.org/book/ch16-02-message-passing.html'),
    'errors': ('Recoverable errors', 'https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html'),
    'io': ('File reads and writes', 'https://doc.rust-lang.org/std/io/index.html'),
    'fs': ('Filesystem rename guarantees', 'https://doc.rust-lang.org/std/fs/fn.rename.html'),
    'vecdeque': ('A queue for bounded searches', 'https://doc.rust-lang.org/std/collections/struct.VecDeque.html'),
    'macroquad': ('macroquad 0.4.16', 'https://docs.rs/macroquad/0.4.16/macroquad/index.html'),
    'shapes': ('Rectangle drawing', 'https://docs.rs/macroquad/0.4.16/macroquad/shapes/fn.draw_rectangle.html'),
    'time': ('macroquad time functions', 'https://docs.rs/macroquad/0.4.16/macroquad/time/index.html'),
    'input': ('macroquad input', 'https://docs.rs/macroquad/0.4.16/macroquad/input/index.html'),
    'texture': ('Texture upload and filtering', 'https://docs.rs/macroquad/0.4.16/macroquad/texture/struct.Texture2D.html'),
    'audio': ('macroquad clip audio', 'https://docs.rs/macroquad/0.4.16/macroquad/audio/index.html'),
    'release': ('Cargo release profiles', 'https://doc.rust-lang.org/cargo/reference/profiles.html'),
    'design': ('Game design: rules and acceptance', '../docs/superpowers/specs/2026-11-06-game-design.md'),
    'art': ('Art style: approved look and open decisions', '../docs/superpowers/specs/2026-11-06-art-style.md'),
}

def slug(text):
    return re.sub(r'[^a-z0-9]+', '-', text.lower()).strip('-')

def name(row):
    return f"{row['number']:04d}-{slug(row['title'])}.html"


def source_link(key):
    label, url = SOURCES[key]
    return f'<a href="{E(url)}">{E(label)}</a>'


def shell(title, content, page_class=''):
    return f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="Learn Rust by building a calm ecological sandbox, one small working result at a time.">
<title>{E(title)} · Rusty</title><link rel="stylesheet" href="../assets/course.css">
<script src="../assets/course.js" defer></script><script src="../assets/lab-widgets.js" defer></script></head>
<body class="{page_class}"><a class="skip" href="#main">Skip to content</a>
<header class="site-header"><a class="brand" href="../lessons/index.html">RUSTY <span>/ a game, built slowly</span></a><a href="../reference/index.html">Reference shelf ↗</a></header>
<main id="main">{content}</main><footer class="site-footer">Build it. Explain it. Change one thing. Ask whenever a step is unclear.<br>
<a href="../MISSION.md">Mission</a> · <a href="../RESOURCES.md">Sources</a> · <a href="../docs/superpowers/specs/2026-11-06-game-design.md">Game design</a></footer></body></html>'''

WINDOW = '''use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Rusty — learning".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        window_resizable: false,
        sample_count: 1,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(Color::from_rgba(15, 26, 51, 255));
        next_frame().await;
    }
}'''
SNIPPETS = {
'Create and run the game package': ('src/main.rs — a complete first program', '''fn main() {
    println!("Our ecological sandbox begins here.");
}'''),
'Name the cell position': ('src/main.rs — run and predict the output', '''fn main() {
    let column = 12;
    let row = 5;
    println!("Cell: {column}, {row}");
    println!("Drawing x: {}", column * 4);
}'''),
'Change state deliberately': ('src/main.rs — change only the binding mutability', '''fn main() {
    let mut row = 5;
    println!("Before: {row}");
    row += 1;
    println!("After: {row}");
}'''),
'Choose numeric types and units': ('src/main.rs — keep the conversion at a boundary', '''fn main() {
    let column: i32 = 12;
    let cell_size: f32 = 4.0;
    let screen_x: f32 = column as f32 * cell_size;
    println!("{screen_x}");
}'''),
'Extract a coordinate conversion function': ('src/main.rs — the final expression is returned', '''fn screen_position(cell: i32) -> f32 {
    cell as f32 * 4.0
}

fn main() {
    println!("{}", screen_position(12));
}'''),
'Decide whether a cell may fall': ('src/main.rs — change row to try the boundary', '''fn main() {
    let row = 158;
    let floor_row = 160;
    if row < floor_row - 1 {
        println!("moving");
    } else {
        println!("resting");
    }
}'''),
'Repeat a small update': ('src/main.rs — trace before running', '''fn main() {
    let mut row = 5;
    while row < 8 {
        row += 1;
        println!("{row}");
    }
}'''),
'Open the first window': ('src/main.rs — complete low-DPI teaching window', WINDOW),
'Group coordinates with a struct': ('src/main.rs — a terminal experiment before integration', '''struct Cell {
    column: i32,
    row: i32,
}

fn main() {
    let cell = Cell { column: 12, row: 5 };
    println!("{}, {}", cell.column, cell.row);
}'''),
'Discover a move before borrowing': ('src/main.rs — intentionally fails: repair only after explaining why', '''struct Cell {
    row: i32,
}

fn consume_cell(cell: Cell) {
    println!("{}", cell.row);
}

fn main() {
    let cell = Cell { row: 5 };
    consume_cell(cell);
    println!("{}", cell.row);
}'''),
'Borrow state to draw and reset': ('src/main.rs — complete borrowing experiment before graphical integration', '''struct Cell {
    row: i32,
}

fn inspect_cell(cell: &Cell) {
    println!("{}", cell.row);
}

fn reset_cell(cell: &mut Cell) {
    cell.row = 5;
}

fn main() {
    let mut cell = Cell { row: 30 };
    inspect_cell(&cell);
    reset_cell(&mut cell);
    inspect_cell(&cell);
}'''),
'Turn coordinates into safe indices': ('src/main.rs — checked row-major mapping', '''fn index(column: i32, row: i32, width: i32, height: i32) -> Option<usize> {
    if column < 0 || row < 0 || column >= width || row >= height {
        return None;
    }
    Some(row as usize * width as usize + column as usize)
}

fn main() {
    assert_eq!(index(31, 17, 32, 18), Some(575));
    assert_eq!(index(-1, 0, 32, 18), None);
}'''),
'Borrow disjoint slices before parallel work': ('src/main.rs — two valid disjoint mutable borrows', '''fn main() {
    let mut values = [0, 0, 0, 0];
    let (left, right) = values.split_at_mut(2);
    left[0] = 1;
    right[0] = 2;
    assert_eq!(values, [1, 0, 2, 0]);
}'''),
}

# One complete continuation used by the three early graphical exercises.
FALLING = '''use macroquad::prelude::*;

struct Cell {
    column: i32,
    row: i32,
}

fn draw_cell(cell: &Cell) {
    draw_rectangle(cell.column as f32 * 4.0, cell.row as f32 * 4.0,
        4.0, 4.0, Color::from_rgba(226, 182, 121, 255));
}

fn reset_cell(cell: &mut Cell) {
    cell.row = 5;
}

fn tick_cell(cell: &mut Cell, floor_row: i32) {
    if cell.row < floor_row - 1 {
        cell.row = (cell.row + 3).min(floor_row - 1);
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Rusty — one falling cell".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        window_resizable: false,
        sample_count: 1,
        ..Default::default()
    }
}

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
        draw_rectangle(0.0, floor_row as f32 * 4.0, screen_width(),
            screen_height() - floor_row as f32 * 4.0,
            Color::from_rgba(29, 48, 80, 255));
        draw_cell(&cell);
        next_frame().await;
    }
}'''
SNIPPETS['Stop a falling grain at a floor'] = ('src/main.rs — complete integration of the earlier pieces', FALLING)

STAGES = {
'Start': ('First steps', 'Install Rust, learn the syntax you need, and land one grain on a floor.'),
'M0': ('Foundation', 'A paintable, chunked world with sand and water; measure before expanding.'),
'M1': ('Water & heat', 'Reactions, exposure and deterministic updates; threads only when measured necessary.'),
'M2': ('Fire, weather & support', 'A house can burn, shelter its contents and collapse when its support is gone.'),
'M3': ('Time & ecology', 'Seasons, bounded random work, decay, renewal and long-run balance.'),
'M4': ('A persistent world', 'Seeded biomes, streaming, exact snapshots and recoverable saves.'),
'M5': ('The vertical slice', 'Approved pixel art, readable UI, sound and the 60-second house scene.'),
'M6': ('World expansion', 'Desert, volcano, island, forest and optional events without breaking v1.'),
'M7': ('Release', 'Error handling, compatibility, platform packages and evidence for shipping.'),
}

# These are actual decision points, not permission prompts for generating course files.
LAB_TITLES = {
'Separate absorbed moisture and surface wetness', 'Find connected support groups',
'Drop and float rigid groups as units', 'Budget the production scheduler',
'Distribute exact calendar multipliers', 'Restore grass after a fire',
'Snapshot a running world without changing it', 'Commit a complete save generation',
'Compose sky sun moon and night', 'Make material states readable without texture noise',
'Draw weather without changing its physics', 'Erode island shores with an explicit water policy',
}


def lesson(row, previous, following):
    n, title, stage = row['number'], row['title'], row['stage']
    if str(n) in DEEP:
        return render_deep_lesson(row, previous, following, DEEP[str(n)], shell=shell, name=name, stages=STAGES, lab_titles=LAB_TITLES)
    prior = 'No programming prerequisite; establish terminal familiarity with your tutor.' if previous is None else f'Builds on <a href="{name(previous)}">{E(previous["title"])}</a>. If you cannot explain that result yet, return to it before adding this feature.'
    review = COURSE[max(0, n-4)] if n > 3 else None
    review_html = '' if review is None else f'<p class="review">Spaced retrieval: before reading, recall one decision from <a href="{name(review)}">lesson {review["number"]:02d}</a>. Try it from memory, then check the earlier page.</p>'
    lab = '' if title not in LAB_TITLES else '<aside class="notice"><strong>Design lab.</strong> The spec leaves a choice or conflicting promise here. Write a concrete proposal, test its consequences, and get the milestone decision agreed before treating it as settled behavior.</aside>'
    milestone = ''
    if previous is not None and previous['stage'] != stage:
        milestone = f'<aside class="notice"><strong>Milestone checkpoint: {stage}.</strong> Review the current game-design milestone and approve its implementation plan before beginning production changes. These prepared lessons do not approve that plan.</aside>'
    snippet = ''
    if title in SNIPPETS:
        label, code = SNIPPETS[title]
        snippet = f'<details class="example"><summary>Worked example — inspect after your first attempt</summary><p>{E(label)}. This is a teaching example, not the finished game.</p><pre><code>{E(code)}</code></pre></details>'
    if title == 'Draw one simulation cell':
        snippet += '<p>Inside the window loop, after clearing and before yielding:</p><pre><code>draw_rectangle(48.0, 20.0, 4.0, 4.0, Color::from_rgba(226, 182, 121, 255));</code></pre>'
    if title == 'Run movement on a 20 TPS clock':
        snippet += '<p>Use the complete integration in the next lesson only after you can trace the clock. First practise with a function that increases one row. This small 1x demo uses floating-point elapsed seconds, not the production integer calendar scheduler.</p>'
    refs = { 'Start': 'rust-foundations.html', 'M0': 'grid-and-state.html', 'M1': 'simulation-contracts.html', 'M2': 'simulation-contracts.html', 'M3': 'time-and-ecology.html', 'M4': 'save-and-release.html', 'M5': 'art-and-ui.html', 'M6': 'simulation-contracts.html', 'M7': 'save-and-release.html' }
    links = source_link(row['source'])
    if row['source'] not in ('design', 'art'):
        links += ' · ' + source_link('design')
    nav_prev = '<a href="index.html">← Course map</a>' if previous is None else f'<a rel="prev" href="{name(previous)}">← {previous["number"]:02d} · {E(previous["title"])}</a>'
    nav_next = '<a href="index.html">Return to course map →</a>' if following is None else f'<a rel="next" href="{name(following)}">{following["number"]:02d} · {E(following["title"])} →</a>'
    work = ''.join(f'<li>{E(s)}.</li>' for s in row['work'])
    content = f'''<article class="lesson" data-lesson="{n}">
<p class="eyebrow">{E(stage)} / {E(STAGES[stage][0])} <span>Lesson {n:02d} of {len(COURSE)}</span></p>
<h1>{E(title)}</h1><p class="lede">{E(row['concept'])}</p>
<p class="prerequisite">{prior}</p>{milestone}{lab}
<div class="lesson-layout"><div class="lesson-main">
<section aria-labelledby="start"><h2 id="start">Start from what you know</h2>
<p>Before opening the example, explain the previous working result to your tutor and predict what needs to change here. Say “I don’t know” when needed; the next step can be smaller.</p>{review_html}
<p>Agree on this lesson’s path: identify the state or rule → make one small change → check the result. During a live session, your tutor probes the relevant prerequisites before presenting code.</p></section>
<section aria-labelledby="build"><h2 id="build">Build one working piece</h2><ol class="work">{work}</ol>{snippet}
<p class="tip">Work in <code>game/</code>. Before moving on, format and check the code; for simulation changes run the relevant headless test. Keep errors visible and ask about the first diagnostic you cannot explain.</p></section>
<section class="recall" aria-labelledby="recall"><h2 id="recall">Close the example. Retrieve it.</h2>
<p id="question-{n}">{E(row['question'])}</p>
<label for="answer-{n}">Your explanation or prediction</label><textarea id="answer-{n}" rows="3" data-recall placeholder="Write your reasoning before revealing feedback."></textarea>
<button type="button" data-check aria-expanded="false" aria-controls="feedback-{n}">Compare with feedback</button>
<p class="attempt-message" aria-live="polite"></p>
<div class="feedback" id="feedback-{n}" hidden><h3>Compare your reasoning</h3><p>{E(row['feedback'])}</p><p>This is a self-check, not automatic grading. If your explanation differs, make a tiny experiment or ask your tutor before continuing.</p></div>
<noscript><p>Write your answer first, then ask your tutor for feedback. Interactive controls need JavaScript; all build instructions remain readable without it.</p></noscript></section>
<section aria-labelledby="proof"><h2 id="proof">Show it works</h2><p>{E(row['proof'])}</p>
<p>Save the tiny fixture, command output or observation that proves this result. Then change one input from your fixture, predict the result, and verify it without copying the example. This transfer check is where your tutor can decide whether to continue.</p>
<label class="completion"><input type="checkbox" data-complete> I attempted the work and recorded its result.</label><p class="small">This checkbox tracks attempts locally in this browser. It does not certify understanding or write a learning record. Do not put private information in browser notes.</p></section>
</div><aside class="reading"><h2>Keep within reach</h2><p>Primary reading: {links}.</p>
<p>Read only the section needed for today’s task. The custom simulation rules are game-design choices, not claims about real physics.</p>
<a href="../reference/{refs[stage]}">Open the relevant reference sheet ↗</a><hr><p><strong>Ask your tutor:</strong><br>“I expected ___, but observed ___. Which assumption should I check?”</p>
<p class="small">Prepared draft. Adapt the pace using demonstrated knowledge; splitting a page across several sittings is expected.</p></aside></div>
<nav class="lesson-nav" aria-label="Lesson navigation">{nav_prev}{nav_next}</nav></article>'''
    return shell(title, content)


def build_index():
    groups = defaultdict(list)
    for row in COURSE:
        groups[row['stage']].append(row)
    sections = ''
    for stage, items in groups.items():
        title, description = STAGES[stage]
        entries = ''.join(f'<li data-course-item="{r["number"]}" data-search="{E((r["title"]+" "+r["concept"]).lower())}"><span class="lesson-number">{r["number"]:02d}</span><a href="{name(r)}">{E(r["title"])}</a><span class="attempt-badge" aria-label="Not yet marked attempted"></span></li>' for r in items)
        sections += f'<section class="stage" id="{stage.lower()}"><div class="stage-heading"><p class="eyebrow">{stage}</p><h2>{E(title)}</h2><p>{E(description)}</p></div><ol class="course-list">{entries}</ol></section>'
    return shell('The complete Rust-to-release course', f'''<section class="hero"><p class="eyebrow">Your Rust learning workspace</p><h1>A living world.<br>One small lesson at a time.</h1>
<p class="lede">{len(COURSE)} detailed build lessons take you from a first Rust program to a release-ready ecological sandbox. Learn the mechanism, follow a worked example, do the exercises, inspect the diagrams.</p>
<a class="button" href="{name(COURSE[0])}">Begin with Rust setup →</a><p class="small">Start at zero. Move forward when you can explain and change the result.</p></section>
<section class="course-intro"><h2>How this course works</h2><p>Each lesson begins with a concrete problem from the game. Follow the explanation, predict the example, run it, then attempt the exercises. Worked solutions and troubleshooting are included so you can learn independently and bring specific questions to your tutor. Lessons include explained commands or code, expected results, worked exercises, troubleshooting. Later examples isolate one mechanism before you integrate it with the existing game.</p>
<p>The mission is to learn Rust while creating this calm sandbox. Work through each lesson at your own pace. At each milestone, agree on its implementation plan; unresolved spec decisions are explicit design labs. Learning records and the canonical glossary are written only after you demonstrate understanding.</p>
<p>The earlier <a href="../docs/lessons/00-index.md">five Markdown drafts</a> are preserved as history. This HTML course is the active path and splits their work into smaller steps.</p>
<p><a href="../reference/index.html">Reference shelf</a> · <a href="../docs/course-coverage.md">Milestone coverage and remaining decisions</a> · <a href="../RESOURCES.md">Trusted resources and optional communities</a></p></section>
<section class="course-controls"><label for="filter">Find a lesson</label><input id="filter" type="search" placeholder="Try borrowing, water, save, or release" data-filter><p aria-live="polite" data-progress></p><button type="button" class="quiet" data-reset>Clear local attempt marks and answers</button><p class="small">Local browser storage is optional; if unavailable the course still works. Marks are attempts, not grades. Revisit an earlier exercise on a later day before relying on recall.</p></section>
{sections}<p class="empty-search" hidden>No lessons match. Try a different word.</p>''', 'course-index')

REFERENCES = {
'rust-foundations.html': ('Rust foundations', [
('The edit/run loop', 'Work inside game/. cargo check checks compilation; cargo run builds and launches; cargo fmt formats source; cargo test runs tests. Formatting is not behavioral verification.'),
('Bindings and functions', 'Use let for a binding, let mut for a binding you reassign. Integer cell coordinates and floating-point drawing/time values have different units. A function’s final expression can supply its return value.'),
('Ownership and access', 'By-value arguments move non-Copy values. &T borrows shared access; &mut T borrows exclusive mutable access while used. A borrow may end at its last use, not necessarily the enclosing closing brace.'),
('Modeling state', 'Use structs for named fields, enums for valid alternatives, Option for possible absence and Result for recoverable failure. Match the alternatives instead of assuming every operation succeeds.'),
('Retrieve before reading', 'From memory, write a function that changes one borrowed Cell and another that only reads it. Compile both, then deliberately try a mutation through the shared reference.')], ['cargo','variables','functions','borrowing','enums','errors']),
'grid-and-state.html': ('Grid and storage', [
('Coordinate contract', 'The first prototype counts rows downward. Production world altitude increases upward. Use an explicit conversion. One simulation cell occupies 2 × 2 art pixels and 4 × 4 physical pixels.'),
('Index contract', 'Validate signed coordinates before conversion. Row-major flat index is row * width + column. Invalid coordinates return None; index zero is a valid cell, not a missing-cell sentinel.'),
('Chunk contract', 'A chunk contains 64 × 64 cells. Euclidean quotient identifies the containing chunk and Euclidean remainder identifies a nonnegative local coordinate, including for negative world positions.'),
('Update contract', 'Moving matter must clear or swap its source. Scan order and moved markers prevent repeated movement. Clip updates per cell to the active rectangle, not merely per overlapping chunk.'),
('Retrieve before reading', 'For world x = -1 and a chunk width of 64, calculate the chunk x and local x. Then test x = 0, 63 and 64 and verify a round trip.')], ['integers','vec','option','design']),
'simulation-contracts.html': ('Simulation contracts', [
('Promises before loops', 'For each rule write its inputs, read/write footprint, conservation exceptions, time limit and one tiny counterexample. Write a headless fixture before relying on a beautiful landscape.'),
('Concurrency', 'A fixed RNG seed does not make overlapping writes safe. Prove the entire read/write footprint of concurrent tasks. Use safe disjoint borrows or synchronization; do not introduce unsafe access to bypass a missing proof.'),
('Exposure', 'Sky exposure controls direct weather; air exposure controls contact heat exchange. A roof shelters an interior while a cave wall may still exchange heat with local sheltered air.'),
('Support and events', 'Connected rigid groups need external support. Flood-fill caps and off-region anchoring are explicit approximations. Particles temporarily own thrown matter; re-entry, caps and serialization need defined policies.'),
('Retrieve before reading', 'Build a five-cell counterexample for duplicate movement, false support, or heat creation. Explain which assertion distinguishes a correct rule from the broken one.')], ['threads','slices','testing','design']),
 'time-and-ecology.html': ('Ticks and ecology', [
('Two counters', 'step_tick counts executed simulation updates. world_tick counts calendar progression. At baseline 20 TPS, 24,000 world ticks make a day, 72,000 make a season and 288,000 make a year.'),
('Scheduling', 'Elapsed-time scheduling is independent of rendered-frame count. Pause/reset, elapsed-time clamp, overdue discard, per-frame time budget and residual calendar debt need explicit tests. A synchronous tick cannot be cancelled by an after-tick budget check.'),
('Random slow work', 'At M = 1, choose 16 cells per active chunk per tick. Above that, use P = min(16 * M, 256); the capped fallback approximates additional work. Repeated picks must not permit same-tick state chains.'),
('Evidence', 'Use fixed regression seeds and the specified 32-seed reliability suite. A majority threshold alone does not guarantee reliability. Compare visible behavior at 1x, 16x and 64x and report accepted differences.'),
('Retrieve before reading', 'Distribute 10 calendar ticks across 3 executed updates, then simulate an early budget stop. Show exactly what remains owned by the accumulator.')], ['time','instant','design']),
 'save-and-release.html': ('Saves and release', [
('State inventory', 'A coherent snapshot captures one tick’s relevant cells, counters, metadata, weather deltas, cooldowns, falling groups and the agreed particle policy. Scheduler remainder persistence is a decision to document.'),
('Crash consistency', 'One writer orders disk operations. Per-file replacement is not a transaction spanning many files. A committed manifest/generation policy must let loading select a complete old or new snapshot.'),
('Versions', 'Format version controls decoding. Generator version controls how absent terrain regenerates. Keep old supported fixtures; reject unsupported data without modifying the input world.'),
('Release artifacts', 'cargo build --release builds optimized executables. cargo package creates a source crate archive, not an end-user game installer. Package assets and licences explicitly and test native launches outside the checkout.'),
('Retrieve before reading', 'Interrupt a simulated save after one region file completes but before the manifest commits. Demonstrate why loading still chooses a coherent prior generation.')], ['io','fs','errors','release','design']),
 'art-and-ui.html': ('Art, display and input', [
('Three units', 'Cells are simulation units, art pixels are drawing units and physical pixels are display units. macroquad screen dimensions use DPI-adjusted drawing coordinates; do not assume they equal physical framebuffer dimensions.'),
('Rendering rules', 'Use the art-pixel buffer and nearest filtering for 2× presentation. Preserve hard edges, coherent flat colours/ramps and the ban on light reflections and bloom. Palette and cloud/UI details still need their specified approvals.'),
('Input ownership', 'Convert mouse coordinates relative to the actual world rectangle. UI consumes a click before painting. A capped border is a panel, never frozen world geometry.'),
('Visual evidence', 'Reference assets must be linked and capture conditions recorded. Day/night, underground, shelter and required reactions need motion/readability review in addition to still comparisons.'),
('Retrieve before reading', 'Draw the same world cell in two differently sized DPI-aware windows. Verify its physical footprint and matching mouse pick without stretching the world.')], ['texture','input','art']),
}


def build_reference(filename, title, sections, keys):
    body = ''.join(f'<section><h2>{E(h)}</h2><p>{E(p)}</p></section>' for h,p in sections)
    sources = ' · '.join(source_link(k) for k in keys)
    return shell(title, f'<article class="reference"><p class="eyebrow">Reference shelf / not an assessment record</p><h1>{E(title)}</h1><p class="lede">A compact aid to use after attempting retrieval. This sheet does not certify that these concepts are learned.</p>{body}<section><h2>Sources</h2><p>{sources}</p></section><p><a href="index.html">← Reference shelf</a> · <a href="../lessons/index.html">Course map →</a></p></article>')


def main():
    for folder in ('lessons','reference','assets'):
        (ROOT/folder).mkdir(exist_ok=True)
    assert [r['number'] for r in COURSE] == list(range(1, len(COURSE)+1))
    for i,row in enumerate(COURSE):
        assert row['source'] in SOURCES
        (ROOT/'lessons'/name(row)).write_text(lesson(row, COURSE[i-1] if i else None, COURSE[i+1] if i+1<len(COURSE) else None))
    (ROOT/'lessons/index.html').write_text(build_index())
    for filename,(title,sections,keys) in REFERENCES.items():
        (ROOT/'reference'/filename).write_text(build_reference(filename,title,sections,keys))
    entries = ''.join(f'<li><a href="{filename}">{E(title)}</a></li>' for filename,(title,_,_) in REFERENCES.items())
    (ROOT/'reference/index.html').write_text(shell('Reference shelf', f'<article class="reference"><p class="eyebrow">Keep the essentials close</p><h1>Reference shelf</h1><p class="lede">Try recall first. Then use a sheet to check a command, unit or contract.</p><ul class="shelf">{entries}</ul><p>Canonical glossary entries and learning records are added only after demonstrated understanding; these are prepared reference aids.</p><a href="../lessons/index.html">← Course map</a></article>'))
    # Export complete beginner examples for verification, without creating the game implementation.
    examples = {row['title']: DEEP[str(row['number'])]['worked']['code'] for row in COURSE if str(row['number']) in DEEP and DEEP[str(row['number'])]['worked'].get('language','').lower() == 'rust'}
    (ROOT/'assets/examples.json').write_text(json.dumps(examples,indent=2,ensure_ascii=False)+'\n')
    print(f'Built {len(COURSE)} lessons ({len(DEEP)} expanded), course map and {len(REFERENCES)} reference sheets.')

if __name__ == '__main__':
    main()
