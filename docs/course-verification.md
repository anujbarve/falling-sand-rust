# Expanded course verification

The rewritten course contains 100 authored lessons, 300 exercises with concealed worked solutions, 21 displayed/visually checked diagram PNGs, and five types of optional interactive teaching models. It includes approximately 53,000 words of explanations and exercise material before counting the worked code, episode notes and shared page text.

## What was checked

- `python3 assets/build_course.py` regenerated all 100 HTML lesson pages, the course index and reference sheets.
- `python3 assets/check_course.py` checked lesson coverage, local links/fragments, HTML IDs, shared offline assets, expanded content fields, concealed feedback/solutions and navigation.
- `node --check assets/course.js` and `node --check assets/lab-widgets.js` checked JavaScript syntax using the installed Node runtime.
- All 96 Rust worked examples were checked against the installed toolchain: 95 compile; lesson 12 intentionally fails with the documented moved-value error `E0382`.
- The three macroquad graphical examples (9, 10, 81) were compile-checked with existing cached macroquad 0.4.16 dependencies using Cargo offline. They were not launched or visually tested as game windows.
- The 36 worked examples containing Rust test suites were compiled in test mode and their tests passed. This verifies the focused examples, not a fully implemented game.
- `assets/worked-checks.json` records the current worked-source SHA-256 values and compilation/test outcomes so edits can invalidate stale evidence.
- The diagram maker rendered and inspected the displayed PNGs. Editable SVGs and their metadata remain in `assets/diagrams/`. The course displays those inspected PNGs, not a different unreviewed rendering.

## Limits

No new dependencies were installed. The HTML pages were not run through an installed browser automation suite, and no claim of browser end-to-end testing is made. The optional models, exercise gates and copy controls use local shared JavaScript; the underlying explanations, diagrams, source and commands remain in the HTML.

These checks do not certify the future game's hardware performance, full-game physics, native packages or visual acceptance. Those are the concrete work and gates taught by later lessons.
