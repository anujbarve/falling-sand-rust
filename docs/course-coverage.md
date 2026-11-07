# Course coverage and operating notes

Active entry point: [HTML course](../lessons/index.html). Source content: [course.json](../assets/course.json). Rebuild with `python3 assets/build_course.py`; validate with `python3 assets/check_course.py`.

This is the prepared curriculum through release. The expanded HTML chapters teach the mechanism with substantive explanation, exact commands or focused worked code, expected results, at least three exercises with concealed worked solutions, troubleshooting, and a YouTube episode outline. Verified diagrams and optional interactive models are attached where they explain the mechanism. It is not a completed game, a full solution repository, a claim of learner mastery or approval of future milestone implementations.

Later pages remain drafts: live tutoring must probe prerequisites and adjust pace. Some labs require a missing design decision before production code can be finalized. That is part of the work, not an invented settled answer. No learning records or canonical glossary entries have been fabricated.

## Coverage by milestone

| Stage | Observable result | Coverage |
|---|---|---|
| First steps | One cell falls onto a floor | Setup, Cargo, variables, types, functions, control flow, macroquad window, cell coordinates, struct, move, borrowing, 1x elapsed-time clock |
| M0 | Paintable chunked sand/water; measured target | Enums, Vec, checked indexing and Option, swaps, iteration/update markers, piles/flow, input, modules, headless tests, maps, chunks, per-cell active clipping, camera, temporary cache policy, sequential phase traversal, graphics/hardware gate |
| M1 | Water/heat reactions within deadlines | Material properties, actual cell layout, paired heat accounting, exposure/heightmap, freeze/melt/boil, steam/basalt, disjoint borrows, threading proof, RNG trait, deterministic tests |
| M2 | House sheltered, burning and falling | Fuel/flags/flames, wetness semantics, weather columns, surface precipitation, wind/gas boundaries, bounded support, rigid groups/flotation/breakup, particles/explosions, lightning, tools/cooldowns, integrated performance |
| M3 | Weathering and renewal over a measured year | Integer calendar, production scheduler/budgets, multiplier remainders, deterministic random picks, capped slow work, moisture/seepage/evaporation, decay/rust/erosion, recovery source, trees/leaves, season/temperature integration, 32-seed and long-run gates |
| M4 | Seeded persistent explored world | Coordinate conventions, deterministic hills, strata/biomes, lakes/caves/rest guarantee, bounded streaming, channels, Result, explicit binary encoding, coherent snapshots, crash-safe manifest generations, generator compatibility, exact reload/memory/rest gates |
| M5 | Approved 60-second vertical slice | Physical pixels/DPI/art buffer, sky/celestial composition, material-state readability, weather visuals, paged bars/tool hints, bounded local undo, clip sound/mute/mix, settings/view caps, captured visual/hardware acceptance |
| M6 | Expanded biomes and events | Desert/wind, deterministic volcano/eruption, island shore erosion, dense forests, optional event tools, regressing v1 benchmarks |
| M7 | Release-ready native artifacts | Recoverable errors, old-save fixtures, CI/platform matrix, optimized native packages/assets/licences, controls/recovery documentation, usability/accessibility decisions, fresh-install checks, changelog/checklist |

## Rules for the incremental build

- Work cumulatively in one learner-created `game/` package; early isolated terminal experiments may replace `main.rs` temporarily. Save the working graphical version before a terminal experiment and restore it for integration. Cargo commands run from `game/` unless a page says otherwise.
- The course does not implement the game now. Before each production milestone, review and approve its implementation plan as required by the spec.
- The first camera/grid examples use downward screen rows and low DPI. Production altitude increases upward; that conversion is explicit. Before world altitude arrives, keep the screen-row convention inside prototype state and do not mistake it for final world coordinates.
- M0 writes the four-phase traversal sequentially before benchmarking. M1 verifies footprint bounds and turns on parallel dispatch only when needed; the phase partition alone does not prove memory safety.
- M0 changed chunks stay resident until the persistent writer is built in M4. This is a documented temporary limitation, not the final unbounded memory policy.
- M1 ambient/weather time inputs and M2 cooldowns may use small fixed tick/calendar fixtures until M3 implements the production clock. M3 year/lake checks use hand-built repeatable terrain and lake fixtures; they do not require M4 generation to exist.
- M0 material colours are simple approved anchors. M5 owns final art-buffer, DPI and readability approval, but early tests must not rely on forbidden glow/noise to communicate effects.
- Keep the smallest meaningful proof per lesson. Repeat earlier retrieval on later days; do not keep expanding tests after the relevant acceptance conditions pass.

## Design decisions that cannot be replaced by lesson text

The spec's section 18 remains open: hardware model, no-zoom acceptance, time scale, starting tools, undo/controls, platform priority, series pace, physical scale, expanded scope and sound policy. Several implementation issues also require explicit proposals: nonporous wetness, support caps, bounded particle reinsertion, coherent particle saves, scheduler residual debt, whole-save crash recovery, isolated grass recovery and visual cloud/UI/reference-image choices.

A future implementation should resolve these against the current spec and record changed behavior in both companion documents. The course labels the relevant exercises as design labs. Performance is not certified until measured on the claimed hardware.

## How to use the offline course

Open `lessons/index.html` directly in a browser. The pages need no external fonts, framework or network service; outgoing primary-source links require internet. Browser storage on file URLs is implementation-dependent. If you want one consistent local origin, run `python3 -m http.server 8000 --bind 127.0.0.1` from the repository root and visit `http://127.0.0.1:8000/lessons/`; stop it with Ctrl+C. Do not bind the teaching server publicly.

Recall answers and attempt marks are optional browser-local state. They are not automatically graded, synced into the repository, or evidence of understanding. Feedback is revealed only after an attempt, including an honest “I don't know.” Learning records should be written only after evidence in live tutoring.

Preparing a release checklist is authorized course work. Publishing, pushing tags or sending messages to communities is not performed by course generation.

## Expanded lesson source

Long-form content is authored in `assets/deep-01-35.json`, `assets/deep-36-70.json`, `assets/deep-71-85.json` and `assets/deep-86-100.json`. `assets/deep_render.py` renders it through the shared course shell. Worked examples are downloadable from `assets/worked/`; these are teaching experiments, not an automatically implemented game. Diagram PNGs are the visually checked display artifacts; SVG source and captions live beside them. `assets/lab-widgets.js` contains the small local interactive models.

The earlier outline-only version was rejected as insufficient. Do not replace expanded material with generic task lists. Maintain the commands, explanations, full practice/feedback and episode preparation when editing. See [series production guide](series-production-guide.md).
