# Teaching notes

- The learner explicitly wants Matt Pocock's `teach` skill, not only the local teaching skill. Loaded source: `/Users/anujbarve/.codex/plugins/cache/mattpocock/mattpocock-skills/1.3.2/skills/productivity/teach/SKILL.md`.
- The mission comes directly from the conversation: Rust from the beginning, short modules, and incremental construction of the ecological game through release.
- Current Rust level: self-described beginner. Terminal and other programming knowledge are unassessed. No quiz answers or completed exercises have established mastery.
- `lessons/index.html` is the active prepared course. The first five Markdown drafts in `docs/lessons/` are preserved, not overwritten.
- Full-course preparation was explicitly requested. Later lesson drafts are not evidence of the learner's zone of proximal development; adapt/split them at the time of teaching.
- In live sessions, probe relevant knowledge, present a small plan, then teach/check one concept at a time. Use concrete game examples and motivate why each Rust concept is needed.
- Do not create a learning record or canonical glossary entry merely because a lesson was authored or opened. Add those only after demonstrated understanding or disclosed prior knowledge, using Matt's formats.
- Browser attempt marks and written recall answers are local conveniences, not grades or learning records. File-URL storage can differ by browser; temporary local serving offers a consistent origin if desired.
- Generated pages use shared CSS and JS. Content lives in `assets/course.json`; rebuild using `python3 assets/build_course.py`. Run `python3 assets/check_course.py` afterwards.
- The production game is not created by this course-generation task. Small worked examples are teaching material, and game milestone implementation plans still need their specified review.
- Early window examples deliberately use low DPI and no resizing. Production physical-pixel/DPI handling is taught in M5.
- Earlier M1–M3 labs use handcrafted deterministic terrain, climate inputs and lake fixtures, not the M4 generator before it exists. Calendar inputs may be fixed fixtures until M3 installs the production clock.
- Optional communities are resources, not an assignment to contact anyone. The user has not stated a community preference.

- The learner rejected the first thin course as inadequate for a YouTube series. Lessons must teach in depth: exact commands, mechanism explanations, expected results, verified diagrams, complete worked examples, at least three real exercises/solutions, troubleshooting and episode beats. Short outcomes do not mean shallow pages. No new browser/test dependencies are authorized; use installed tools only.
