# Run from game/ after adding the named fixtures in earlier lessons.
cargo fmt --check
cargo check
cargo test --release rest_generated_world
cargo test --release save_round_trip
cargo test --release negative_chunk_boundaries
cargo test --release stale_save_ack
cargo run --release

# During the recorded manual run:
# 1. Use the documented fixed seed and generator version.
# 2. Pan across all v1 biomes and edit across a chunk boundary.
# 3. Save; reload; compare complete agreed state hashes.
# 4. Record resident AND queued memory throughout the stress run.
# 5. Preserve hardware, timings and observed failures in the M4 report.
