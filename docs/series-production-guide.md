# Record the Rust sandbox series

The HTML lesson is both a teaching chapter and preparation for an episode. It includes a hook, teaching order, visible demonstration and closing takeaway under **Turn the lesson into an episode**. Those beats are not a replacement for understanding and testing the code first.

## Prepare an episode

1. Start from the last known working project state. Keep a copy/commit of that state before changing code for the recording; do not commit secrets, recordings or machine-specific paths accidentally.
2. Work through the new chapter and its three exercises yourself. Read the mechanism and explain it aloud without the page. If a line still feels magical, ask about it before recording.
3. Run the focused example and note actual output. Version numbers and local paths will vary; the lesson explains the expected behavior. Make one deliberate mistake and learn how to diagnose it so the video can show a real repair.
4. Choose one visual result for the opening. For setup it can be the first successful executable; for simulation it might be a grain landing, wet wood resisting ignition or a world surviving a crash-injected save.
5. Use the diagram to show the relationship the code expresses. Do not merely read every label; trace one concrete value through it.

## A repeatable recording shape

- Show the result or failure that motivates the episode.
- State what the viewer already needs and what one thing they will be able to do afterwards.
- Derive the next step from the problem. Explain inputs, state changes and outputs before adding more machinery.
- Type a small change, predict it and run it. Keep compiler errors visible long enough to identify the useful message.
- Pause for one exercise. Reveal the solution only after an attempt, then run its verification check.
- Finish with the working feature, a small variation and the precise next problem it unlocks.

Episode length is not fixed by lesson count. Split a dense lesson across recordings when that keeps the explanation coherent. Combine only already-familiar retrieval or recap, not several new ownership/threading concepts at once.

## What viewers should be able to reproduce

Include the Rust/toolchain and dependency versions, source checkpoint, commands, input fixture/seed and expected behavior. State whether footage shows a teaching model, the actual game, or a proposed visual treatment. A local helper compiling does not prove the entire game milestone passes.

Keep behavior evidence separate from hardware claims. Headless fixture tests can run in CI. GPU compatibility, measured frame times, sound, art approval and native packaging need the relevant real environment and observed results.

## Working with the HTML pages

Open `lessons/index.html` directly; no package installation or development server is required. Every page shares local CSS/JS, includes primary-source links, and preserves its lesson URL. Exercises reveal solutions after a typed attempt; an honest “I don't know” is accepted. Browser-local answers and attempt marks are conveniences, not automatic grades.

The optional interactive models never run terminal commands. Use your actual terminal for installation, compilation and game output. Downloadable examples are focused experiments; read the source-placement note before replacing the game entry point.
