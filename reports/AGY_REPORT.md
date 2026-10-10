# AGY REPORT: task-002
Model used: Claude Opus 4.6 (Thinking)
Status: DONE

## Decisions summary
a. **Bridge:** `flutter_rust_bridge` (FFI) on all native platforms. Axum server kept for dev CLI only, not shipped.
b. **Web:** Dropped from v1. ONNX can't compile to WASM; FRB web support is limited; offline-first promise undeliverable.
c. **EPUB rendering:** Custom document model (parse XHTML → ContentBlock/InlineSpan → Flutter RichText). Scroll mode v1, pagination v2.
d. **Word timings:** ElevenLabs: native char-level alignment (verified). OpenAI TTS: no timing, use estimated fallback (verified). Kokoro: native token durations via `pred_dur` tensor (verified). Piper: phoneme frame durations, need mapping (verified).
e. **Offline TTS:** Kokoro primary (Apache 2.0, better quality, native timestamps, ~164 MB fp16). Piper deferred to v2.
f. **Cloud sync:** LAN-only v1 (mDNS + direct TLS). Cross-network sync is not feasible without a relay server — flagged for v2 with optional user-provided relay.
g. **Secure storage:** `flutter_secure_storage` on all platforms (Keychain/Keystore/CredMgr/libsecret) — verified for Linux+Windows desktop.
**v1/v2 split:** v1 = EPUB+TXT, scroll reader, Kokoro+ElevenLabs+OpenAI TTS, highlights/notes, AI, 3 themes, LAN sync (beta). Cut: MOBI, pagination, Piper, 3D carousel, Calibre, web, ink-fluid animations, cross-network sync.

## What I did (bullets)
- Read AGENTS.md, project overview, existing task-001 report, and codebase structure.
- Researched 7 architectural decisions via web search + 3 parallel research subagents (TTS APIs, FRB/platform lifecycle, EPUB rendering).
- Verified all TTS provider timing capabilities against official docs (ElevenLabs, OpenAI, Kokoro, Piper).
- Verified `flutter_secure_storage` platform support (all 6 platforms including Linux/Windows desktop).
- Verified `flutter_rust_bridge` v2 capabilities: async Rust, Stream support, iOS/Android lifecycle constraints, WASM limitations.
- Wrote `docs/specs/01-architecture.md` (~650 lines) covering all 8 sections requested.

## Files created/changed (paths only)
- inkwave/docs/specs/01-architecture.md (new)
- inkwave/reports/AGY_REPORT.md (overwritten)

## Verification
| Check | Where run | Result |
|---|---|---|
| ElevenLabs alignment API | Web search + subagent | VERIFIED — char-level via `/stream-with-timestamps`, word-level via Forced Alignment API |
| OpenAI TTS timing | Web search + subagent | VERIFIED — no native timing data |
| Kokoro word timing | Web search + subagent | VERIFIED — `pred_dur` tensor in timestamped ONNX model |
| Piper phoneme durations | Web search + subagent | VERIFIED — not exposed in standard ONNX output; needs patched export |
| flutter_secure_storage desktop | Web search + subagent | VERIFIED — Windows (Credential Manager), Linux (libsecret) |
| flutter_rust_bridge iOS lifecycle | Web search + subagent | VERIFIED — process frozen on suspend; needs audio background mode |
| Architecture spec line count | local | ~650 lines, within 700 limit |

## Deviations from the task (and why)
- Section 4 (API contract) lists FRB function signatures instead of HTTP endpoints, because ADR-A chose FFI over HTTP. HTTP notation is shown alongside for clarity.
- `rusqlite` replaces `sqlx` from the project overview — explained in crate layout section (synchronous is simpler for FFI).

## Blockers / questions for Claude
- None. All 7 decisions are researchable and documented. Two open questions flagged for Phase 2 prototype spikes (audio playback package for streaming PCM, audio_service + FRB compatibility on iOS).

## Suggested next step (1-2 lines)
Update `tasks/NEXT_TASK.md` for task-003: Scaffold the Rust crate layout (inkwave-parsers, inkwave-tts, inkwave-ai, inkwave-sync, inkwave-db, inkwave-ffi) with trait stubs and shared types from the architecture spec. No implementations yet.
