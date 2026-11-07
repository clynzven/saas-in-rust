# Lesson 1: The Plan and a Production-Shaped Skeleton

Lesson 1: what we're building, why this stack, and a skeleton that's already shaped for production.

This folder is the complete Slotbook project as it stands at the end of lesson 1
(Git tag `lesson-01` of this repository). It compiles and its tests pass.

## Run it

```bash
cargo test
cargo run
```

## What changed in this lesson

```
 .env.example    | 4 +
 .gitignore      | 2 +
 Cargo.lock      | 688 +++++++++++++++++++++++++++++++++
 Cargo.toml      | 27 ++
 src/config.rs   | 43 +++
 src/lib.rs      | 18 +
 src/main.rs     | 24 ++
 tests/health.rs | 17 +
 8 files changed, 823 insertions(+)
```

See it in full with `git show lesson-01 -- code/slotbook-complete` from the repository root.
