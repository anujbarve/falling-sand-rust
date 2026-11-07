# 01 — Set up Rust and run your first program

**Result:** a program you wrote prints a message. **Prerequisites:** none; allow extra time for installation.

## Start with a probe

What does a terminal command do?

1. Sends an instruction to a program on your computer.
2. Adds a line to the currently open source file.
3. Changes the programming language of a source file.
0. I don't know.

Wait for the answer. Ask the learner to open their terminal and identify their current folder. If they already know this, probe the difference between source code and a running program before choosing the teaching depth.

## Plan the lesson

Establish files and folders, then source versus executable, then the edit/build/run cycle. Present this order and confirm it before starting. The practical question is: how do the instructions in a text file become a running game?

## Work

1. Check what is already installed:

   ```sh
   rustc --version
   cargo --version
   ```

2. If either command is missing, follow the official [Rust installation instructions](https://doc.rust-lang.org/book/ch01-01-installation.html) for the learner's OS. On macOS, install command-line developer tools with `xcode-select --install` if needed for linking. On Windows, follow the installer’s MSVC prerequisites; on Linux, use the distribution's compiler/linker prerequisites. Reopen the terminal after installation and repeat the version checks. Do not reinstall a working toolchain.

3. From the `rusty` repository root, create the teaching project, provided `game/` does not already exist:

   ```sh
   cargo new game --vcs none
   cd game
   cargo run
   ```

   If the folder already exists, inspect it instead of overwriting it. All later Cargo commands run inside `game/`.

4. Open `src/main.rs`. Change only the text in `println!`:

   ```rust
   fn main() {
       println!("Our ecological sandbox begins here.");
   }
   ```

5. Save and run `cargo run`. Then run `cargo check` and `cargo fmt`.

## Build the understanding

`main` is the entry function of this small executable. Its body is inside braces. `println!` writes a line to the terminal; its exclamation mark identifies a macro invocation. We will learn macro internals later.

`Cargo.toml` describes the package and its dependencies. `src/main.rs` contains our executable's source. Cargo invokes the compiler as part of building; `cargo run` builds and runs, while `cargo check` checks compilation without producing the final executable. `cargo fmt` formats source; it does not prove behavior correct.

Explain one distinction at a time and ask the learner to predict what their next command will do before running it. Use a deliberately removed closing quote to practise reading the compiler's file/line location, then repair it.

## Exit check

Without copying the example, print two different lines. Explain which file you edited, which command ran it, and whether editing a file alone changes an already running program. Show both successful output and a successful `cargo check`.

Next: [02 — Variables](02-variables.md).

Source: [The Cargo introduction](https://doc.rust-lang.org/book/ch01-03-hello-cargo.html).
