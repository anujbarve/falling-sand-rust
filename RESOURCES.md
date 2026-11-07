# Rust sandbox resources

## Knowledge

- [The Rust Book: installation](https://doc.rust-lang.org/book/ch01-01-installation.html)
  Toolchain and native linker prerequisites. Start here when either rustc or Cargo is missing.
- [The Rust Book: Cargo](https://doc.rust-lang.org/book/ch01-03-hello-cargo.html)
  Package layout and the edit/build/check/run workflow. Use for the first executable.
- [The Rust Book: basic concepts](https://doc.rust-lang.org/book/ch03-00-common-programming-concepts.html)
  Bindings, numeric types, functions and control flow. Individual lesson pages link the exact chapter needed.
- [The Rust Book: ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
  Moves and references. Use when drawing, updating and retaining the same world state.
- [The Rust Book: structs](https://doc.rust-lang.org/book/ch05-00-structs.html) and [enums](https://doc.rust-lang.org/book/ch06-00-enums.html)
  Named state and valid alternatives. Use for cells, materials and commands.
- [The Rust Book: collections](https://doc.rust-lang.org/book/ch08-00-common-collections.html)
  Vectors and maps. Use for cell buffers, chunk storage and predictable access patterns.
- [The Rust Book: Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
  Recoverable failure and propagation. Use before reading/writing saves or configuration.
- [The Rust Book: traits](https://doc.rust-lang.org/book/ch10-02-traits.html)
  Small behavior interfaces. Use for the deterministic choice source and tests, not as a reason to overabstract materials.
- [The Rust Book: testing](https://doc.rust-lang.org/book/ch11-01-writing-tests.html)
  Assertions and test organization. Use for tiny headless fixtures before visual integration.
- [Standard library: Euclidean integer division](https://doc.rust-lang.org/std/primitive.i32.html#method.div_euclid)
  Correct chunk mapping at negative coordinates; pair with rem_euclid.
- [Standard library: disjoint mutable slices](https://doc.rust-lang.org/std/primitive.slice.html#method.split_at_mut) and [scoped threads](https://doc.rust-lang.org/std/thread/fn.scope.html)
  Safe access foundations for concurrency. They do not themselves prove cross-chunk writes are disjoint.
- [The Rust Book: channels](https://doc.rust-lang.org/book/ch16-02-message-passing.html)
  Ownership transfer between workers and the main loop. Use for bounded streaming and writer queues.
- [Standard library: Instant](https://doc.rust-lang.org/std/time/struct.Instant.html)
  Elapsed-time measurement for scheduler budgets and benchmarks.
- [macroquad 0.4.16 documentation](https://docs.rs/macroquad/0.4.16/macroquad/index.html)
  Pinned rendering/input/frame-loop APIs matching the game design. The introductory low-DPI window is deliberately smaller in scope than the final renderer.
- [macroquad window implementation](https://docs.rs/macroquad/0.4.16/src/macroquad/window.rs.html)
  Verify DPI-adjusted drawing dimensions before converting a physical-pixel budget into world extent.
- [macroquad texture API](https://docs.rs/macroquad/0.4.16/macroquad/texture/struct.Texture2D.html) and [audio API](https://docs.rs/macroquad/0.4.16/macroquad/audio/index.html)
  Art-buffer upload/filtering and clip playback/volume. These sources do not establish arbitrary synthesized audio-stream support.
- [Standard library: File::sync_all](https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all) and [filesystem rename](https://doc.rust-lang.org/std/fs/fn.rename.html)
  File durability and replacement primitives. Whole-world save transactions need an additional manifest/generation policy and platform testing.
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html) and [cargo build](https://doc.rust-lang.org/cargo/commands/cargo-build.html)
  Optimized executable builds and target selection. Desktop asset bundling is separate from [cargo package](https://doc.rust-lang.org/cargo/commands/cargo-package.html), which produces a source crate archive.
- [Game design](docs/superpowers/specs/2026-11-06-game-design.md) and [art style](docs/superpowers/specs/2026-11-06-art-style.md)
  Project-owned rules and acceptance targets. They describe intended behavior, not external scientific facts or already proven performance.

## Wisdom (optional communities)

- [Rust users forum](https://users.rust-lang.org/)
  Ask focused Rust questions with a minimal reproducible example. Optional when a borrow or API question survives local experiments.
- [macroquad project discussions](https://github.com/not-fl3/macroquad/discussions)
  Engine-specific questions and examples. Verify important API answers against the pinned source. No messages are sent on the learner's behalf.

## Gaps and re-verification

- Approved reference-image assets, complete palette/UI/cloud decisions and comparison tolerances must still be attached/approved; course pages cannot establish visual sign-off by themselves.
- Actual floor hardware and graphics compatibility remain unmeasured here.
- Signing, notarization, installers and storefront requirements depend on the eventual distribution channel. Recheck official platform documentation during the native packaging lesson before choosing a release procedure.
- Third-party noise, compression, encoding and audio-asset choices are not silently prescribed. Research and pin any selected dependency in the corresponding milestone plan.
- Physics choices are intentionally game-specific approximations. Use fixtures and visibility deadlines rather than presenting them as physical laws.
