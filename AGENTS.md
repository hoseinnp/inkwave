# AGENTS.md: Rules for agy (Antigravity)

You are the executor for InkWave. Claude is the planner/reviewer. Follow this protocol every session.

## Loop
1. Read `docs/inkwave-project-overview.md` (once per session) and `tasks/NEXT_TASK.md`.
2. Execute ONLY what NEXT_TASK.md says. No extra features, no refactors outside scope.
3. Run the verification commands listed in the task.
4. Write `reports/AGY_REPORT.md` (overwrite it) using the template below.
5. `git add -A && git commit -m "task-<id>: <short summary>" && git push`
6. Stop. Do not start the next task.

## Report rules (token-saving, Claude reads this)
- Max ~60 lines. No full-file dumps, no long logs.
- Show only: failing output (last 20 lines), key decisions, diff summary.
- Be honest: if something failed or was skipped, say so. Never claim "passes" without running it.

## Report template
# AGY REPORT: task-<id>
Model used: <name>
Status: DONE | PARTIAL | BLOCKED
## What I did (bullets)
## Files created/changed (paths only)
## Verification (command -> PASS/FAIL, last lines if FAIL)
## Deviations from the task (and why)
## Blockers / questions for Claude
## Suggested next step (1-2 lines)

## Code conventions
- Rust: stable toolchain, `cargo fmt` + `cargo clippy -- -D warnings` must pass. Workspace under `/backend`.
- Flutter: stable channel, `flutter analyze` must pass. App under `/app`.
- No telemetry, no accounts, API keys only in device secure storage (see overview, Privacy section).
- Do not add dependencies not listed in the task without noting it in the report.
