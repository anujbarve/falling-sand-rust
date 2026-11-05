#!/bin/bash
set -u
A=/Users/anujbarve/Documents/rusty/.claude/worktrees/ab-main/sand
B=/Users/anujbarve/Documents/rusty/.claude/worktrees/perf-integration/sand
(cd $A && cargo build --release --bin profile) > /private/tmp/claude-501/-Users-anujbarve-Documents-rusty/ff4dc30f-d51a-4e49-a0f4-51e8fe6d8c94/scratchpad/ab-build-a.log 2>&1
(cd $B && cargo build --release --bin profile) > /private/tmp/claude-501/-Users-anujbarve-Documents-rusty/ff4dc30f-d51a-4e49-a0f4-51e8fe6d8c94/scratchpad/ab-build-b.log 2>&1
for i in 1 2 3; do
  (cd $A && ./target/release/profile) > /private/tmp/claude-501/-Users-anujbarve-Documents-rusty/ff4dc30f-d51a-4e49-a0f4-51e8fe6d8c94/scratchpad/ab-a-$i.log 2>&1
  (cd $B && ./target/release/profile) > /private/tmp/claude-501/-Users-anujbarve-Documents-rusty/ff4dc30f-d51a-4e49-a0f4-51e8fe6d8c94/scratchpad/ab-b-$i.log 2>&1
done
echo done > /private/tmp/claude-501/-Users-anujbarve-Documents-rusty/ff4dc30f-d51a-4e49-a0f4-51e8fe6d8c94/scratchpad/ab-done
