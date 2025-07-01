# 🤖 Codex Agent Protocol: obsctl

Welcome, Codex or autonomous contributor.

A robust, production-grade command-line tool that recursively uploads files from a local directory to an S3-compatible object storage (e.g., Cloud.ru OBS), while ensuring data integrity by skipping files that are still being written.

## Agent Workflow Rules

1. **All changes must be tied to a task.**
    - Tasks are stored in `tasks/<crate>/tasks.md`

2. **Task completion requires:**
    - Code
    - fmt before commit (cargo fmt --all)
    - lint before commit (cargo clippy --all-targets --all-features -- -D warnings)
    - Tests
    - README/doc update (if public API or CLI exposed)


4. **Do not write to crates outside the current task scope.**
    - Use interfaces exposed by other crates.
    - If you need changes, create a dependency task.

5. **Tests must be deterministic and run under `just test`.**


## Output Guidelines

- All structs, enums, traits must be documented.
- Use `tracing::instrument` for runtime behavior visibility.


Happy contributing
