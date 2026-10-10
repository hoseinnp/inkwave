# TASK 001: Scaffold the monorepo (Phase 0)

## Goal
Create the project skeleton so later phases have a stable base. No features yet.

## Steps
1. Create this layout:
   /app        Flutter project (org: dev.inkwave, name: inkwave)
   /backend    Cargo workspace
     /crates/inkwave-core     lib: shared types, error type
     /crates/inkwave-server   bin: axum server
   /docs, /tasks, /reports
2. inkwave-server: axum app on 127.0.0.1:0 (random port; print chosen port on stdout as `INKWAVE_PORT=<n>`), one route `GET /health` returning {"status":"ok","version":"<cargo pkg version>"}.
3. Add crates: axum, tokio (full), serde, serde_json, tracing, tracing-subscriber, thiserror. Nothing else.
4. Add one integration test in inkwave-server that starts the app and asserts /health returns 200 + correct JSON.
5. Flutter: enable platforms linux, windows, macos, android, ios, web. Keep the default counter app but change the app title to "InkWave". Do not add packages.
6. Root `.gitignore` (Flutter + Rust).
7. Add `.github/workflows/ci.yml`: Rust (fmt --check, clippy -D warnings, test) + Flutter (analyze, test), on push.

## Verification (run all, report PASS/FAIL)
- cd backend && cargo fmt --check
- cd backend && cargo clippy --all-targets -- -D warnings
- cd backend && cargo test
- cd app && flutter analyze
- cd app && flutter test
- Run the server and curl http://127.0.0.1:<port>/health

## Out of scope
Parsing, TTS, AI, DB, themes, UI design.
