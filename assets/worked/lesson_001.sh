rustc --version
cargo --version
rustup --version
xcode-select -p
# If the Apple developer tools are absent:
xcode-select --install
# If Rust is absent, run the official installer:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Reopen Terminal, or activate this shell:
source "$HOME/.cargo/env"
rustc --version
cargo --version
rustup show active-toolchain
