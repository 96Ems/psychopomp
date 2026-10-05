# Agent Brief

A template for briefing a subagent on Psychopomp work. Fill in the angle
brackets and delete lines that do not apply.

```markdown
WORKTREE: <path> (branch `<branch>`, from `<commit>`). Write only there.

TASK: <the outcome, the scene or recipe it serves, and what done looks like>.

BASELINE (read-only): <dir>, made with `verify baseline --out <dir>` on `<commit>`.
Check your work with `cargo run --release -- verify compare <dir> --expect <scenes you
mean to change>`. Do not build the base commit or render your own baseline.

SHARED FILES: other agents are on <branches>. Keep edits to README.md,
ARCHITECTURE.md, SCENE_PLANS.md, CONTEXT.md, verify.json, and registration sites
(plan_runtime.rs, preflight.rs, render.rs) small and additive. A new scene needs no
Cargo.toml edit. <Reserved IDs, such as Stage primitive kinds, when branches share
a table.>

RULES:
1. Never end your turn while a background command runs; you are not resumed when it
   finishes. Run builds and renders in the foreground with a 30-minute timeout.
2. zsh: write flags as separate words or arrays (`flags=(--theme neutral)`,
   `"${flags[@]}"`), and quote globs (`'*.rs'`).
3. Commit after each milestone (types and tests, renderer, showroom and docs) with
   conventional messages and no AI attribution or model names.
4. Check frames (`plan frame --shutter`, `plan snapshot`, sheets) and a short
   `plan render --range a..b` before one full render.
5. Report lint failures in files you did not touch instead of fixing them.

VERIFY: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features
-- -D warnings`, `cargo test --workspace`, and `verify compare`.

REPORT: commits, what changed and why, verification with numbers, and what is left.
```
