# InkWave Architecture Specification

> task-002 · 2026-10-10 · Foundational architecture, no application code.

---

## 1. Architectural Decisions

### ADR-A: Flutter ↔ Rust Bridge

**Decision:** Use `flutter_rust_bridge` (FRB v2) via FFI on all native platforms. Drop the local axum HTTP server from the app architecture.

**Alternatives considered:**

| Option | Pros | Cons |
|--------|------|------|
| Local HTTP (axum) | Language-agnostic, easy debugging via curl | iOS suspends background processes; HTTP overhead per call; port conflicts; cannot work on web |
| `flutter_rust_bridge` / FFI | Zero-copy, async Rust from Dart, codegen, single binary | Build complexity, codegen step, no web WASM support yet |
| Hybrid (HTTP on desktop, FFI on mobile) | Optimised per platform | Two integration paths to maintain, double the bugs |

**Why FFI wins:**
- iOS aggressively suspends background processes. A local HTTP server will be killed within ~30 s of backgrounding unless the app holds an active audio session. FFI calls survive because they run in-process — Ref: [Apple Background Execution](https://developer.apple.com/documentation/uikit/app_and_environment/scenes/preparing_your_ui_to_run_in_the_background).
- Android foreground services keep the process alive, so HTTP *could* work, but FFI avoids the socket overhead entirely.
- FRB v2 supports async Rust, `Stream` return types (for SSE-like streaming), and zero-copy buffer transfer — Ref: [flutter_rust_bridge docs](https://cjycode.com/flutter_rust_bridge/).
- A single integration path reduces maintenance burden.

**Per-platform recommendation:**

| Platform | Bridge | Audio lifecycle |
|----------|--------|-----------------|
| iOS | FRB/FFI | `audio_service` + Background Modes entitlement |
| Android | FRB/FFI | `audio_service` + Foreground Service |
| macOS/Windows/Linux | FRB/FFI | No lifecycle restrictions |
| Web | See ADR-B | N/A |

**Consequences:**
- `inkwave-server` (axum) crate becomes `inkwave-ffi`, exposing the same API surface as Dart-callable functions instead of HTTP endpoints.
- The existing axum health-check scaffold from task-001 is kept for development/testing CLI only, not shipped in the app.
- Streaming TTS audio+timings uses FRB's `Stream<Vec<u8>>` / `Stream<WordTiming>` rather than SSE.

---

### ADR-B: Web Target

**Decision:** Drop web from v1. Revisit in v2 with a WASM subset.

**Alternatives considered:**

| Option | Feasibility |
|--------|-------------|
| Full Flutter Web + local Rust server | Impossible — browser cannot connect to localhost Rust server |
| WASM compilation of Rust core | Partial — `ort` (ONNX Runtime) does not compile to WASM; `reqwest` needs wasm32 feature flags; SQLite needs sql.js or OPFS; file I/O absent |
| Reduced web build (cloud TTS only, no offline) | Possible but still requires EPUB parser in WASM, custom storage layer, and loses the core offline-first promise |

**Why drop web for v1:**
- The value proposition is offline-first with local TTS. Web cannot deliver this.
- EPUB parsing could work in WASM, but ONNX inference (Kokoro/Piper) cannot — `ort` crate targets native only.
- `flutter_rust_bridge` does not support Flutter Web via WASM as of v2 (unverified for latest; last checked: FRB issue tracker shows open request).
- Building a parallel JS/WASM integration for web is a v2 effort.

**Consequences:**
- `flutter create` still enables the web platform (already done), but no web-specific code is written.
- Architecture must not *prevent* future web support: keep Rust core logic free of native-only syscalls where possible, isolate ONNX behind a trait.

---

### ADR-C: EPUB Rendering + Word Highlight

**Decision:** Custom document model — parse EPUB XHTML into a structured `BookDocument` (blocks/inlines), render with Flutter `RichText` / `SelectableText.rich` widgets. Scrolling mode for v1, pagination for v2.

**Alternatives considered:**

| Option | Pros | Cons |
|--------|------|------|
| WebView + epub.js | Fast to build, handles CSS | No WebView on Linux (webview_flutter unsupported); heavy memory; JS bridge for highlighting is fragile; poor native feel |
| Custom doc model + Flutter text layout | Full control over word-level highlighting; consistent across all desktop + mobile; native performance | Must parse XHTML subset; must handle images, links, basic CSS; pagination is hard |

**Why custom rendering:**
- **Linux/Windows desktop support is a hard requirement.** `webview_flutter` does not support Linux. `flutter_inappwebview` has partial Linux support via WebKitGTK but is fragile — Ref: [pub.dev flutter_inappwebview](https://pub.dev/packages/flutter_inappwebview).
- Word-level highlight requires knowing the pixel position of every word on screen. With WebView this requires round-trip JS calls; with native `TextSpan`, each word is a distinct span with a `GestureRecognizer` and can be styled directly.
- Glassmorphism and theme integration are trivial with native rendering, painful with WebView CSS isolation.

**Rendering pipeline:**

```
EPUB file
  → Rust: unzip, parse OPF manifest, parse XHTML chapters
  → Rust returns: Vec<Chapter> where Chapter = Vec<ContentBlock>
  → ContentBlock = Paragraph(Vec<InlineSpan>) | Heading(...) | Image(url) | ...
  → InlineSpan = { text, bold, italic, href, word_index }
  → Flutter: maps ContentBlock → Widget (RichText, Image, etc.)
  → Each word gets a unique word_index for TTS sync
```

**Pagination vs scroll:**
- **v1: Scroll mode.** `ListView.builder` with one widget per `ContentBlock`. Simple, avoids the hard problem of cross-block page boundaries.
- **v2: Pagination.** Use `TextPainter.computeLineMetrics()` to calculate how many blocks fit in a viewport, then paginate. This is a substantial effort — defer.
- Progress tracking: store scroll offset as percentage + chapter index + first-visible word_index.

**Consequences:**
- Rust EPUB parser must output a structured document model, not raw XHTML.
- `flutter_widget_from_html` is *not* used — too generic, not designed for word-level control.
- Complex CSS (tables, floats, custom fonts embedded in EPUB) will be best-effort in v1.

---

### ADR-D: Word Timestamps per TTS Provider

**Decision:** Define a unified `WordTiming` model. Each provider has a different native capability; use fallback strategies where native timing is unavailable.

#### Provider capabilities (verified via web search, Oct 2026):

| Provider | Native word timing? | Mechanism | Source |
|----------|-------------------|-----------|--------|
| ElevenLabs | **Yes** | `text-to-speech/{voice_id}/stream-with-timestamps` returns character-level alignment; WebSocket API returns word-level alignment | [elevenlabs.io/docs](https://elevenlabs.io/docs/api-reference) |
| OpenAI TTS | **No** | Standard `/v1/audio/speech` returns audio only, no timing data | Verified — community workaround is Whisper post-processing |
| Kokoro (ONNX) | **Yes** | Model can output token-level durations; Rust implementations (e.g. `kokoros` crate) support `--timestamps` for per-word timing | [crates.io/crates/kokoros](https://crates.io/crates/kokoros) |
| Piper (ONNX) | **Partial** | VITS duration predictor outputs phoneme frame durations; must map phonemes→words via espeak-ng phonemization + cumulative timing | [github.com/rhasspy/piper](https://github.com/rhasspy/piper) — now at OHF-Voice/piper1-gpl |

#### Unified model:

```rust
/// Timing for a single word in TTS output.
pub struct WordTiming {
    pub word_index: u32,      // matches InlineSpan.word_index from EPUB model
    pub text: String,         // the word as spoken
    pub start_ms: u64,        // offset from audio chunk start
    pub end_ms: u64,          // offset from audio chunk start
    pub confidence: f32,      // 1.0 = native, 0.5 = estimated
}

/// A chunk of audio with associated timings.
pub struct AudioChunk {
    pub pcm_data: Vec<u8>,    // raw PCM or opus-encoded bytes
    pub format: AudioFormat,  // PCM_S16LE_44100 | OPUS_48000 | MP3
    pub timings: Vec<WordTiming>,
    pub chapter_index: u32,
    pub sentence_index: u32,
}
```

#### Fallback strategies per provider:

| Provider | Strategy | Latency impact |
|----------|----------|----------------|
| ElevenLabs | Use native character-level alignment → aggregate to word boundaries | None |
| OpenAI TTS | **Fallback A:** Post-process with Whisper (transcribe generated audio with `verbose_json` → word timestamps). **Fallback B:** Estimate timing by dividing audio duration proportionally by word character count. | A: +1-2s per sentence. B: None but inaccurate on emphasis/pauses |
| Kokoro | Use native token duration output | None |
| Piper | Map espeak-ng phonemes to words, sum frame durations per word, convert frames→ms via sample_rate/hop_length | Negligible (CPU math) |

**v1 default:** Fallback B (estimated timing) for OpenAI TTS. Whisper fallback is v2 (requires shipping whisper.cpp or calling OpenAI STT API).

**Consequences:**
- `confidence` field lets the UI show degraded highlight (e.g., sentence-level highlight if all confidences < 0.5).
- OpenAI TTS is usable in v1 but with imprecise word sync — document this to the user.
- Whisper-based alignment is architecturally supported but not implemented in v1.

---

### ADR-E: Offline TTS Engine

**Decision:** Kokoro as the primary offline engine via `ort` (ONNX Runtime) in Rust. Piper as a secondary/fallback option.

**Comparison:**

| Factor | Kokoro (82M) | Piper (VITS) |
|--------|-------------|--------------|
| Quality | Better — StyleTTS2 architecture, more natural prosody | Good — VITS, intelligible but more robotic |
| Model size (fp16) | ~164 MB | ~50–75 MB (varies by voice) |
| Model size (int8) | ~88 MB | ~25–40 MB |
| Word timing | Native token durations | Phoneme frame durations (need mapping) |
| Rust ecosystem | `kokoros` crate exists on crates.io | `piper-tts-rs` crate exists; may need extension |
| License | Apache 2.0 | MIT (models) / GPL-3.0 (OHF-Voice piper1) |
| CPU speed (relative) | ~1.5x realtime on modern mobile CPU (unverified) | ~3-5x realtime (faster, simpler model) |

**Why Kokoro primary:**
- Better audio quality is critical for a "premium" reading experience.
- Native word timestamps simplify the pipeline.
- Apache 2.0 license is simpler for distribution.

**Model download/storage:**
- Models are NOT bundled in the app binary — would add 100+ MB to APK/IPA.
- On first use of offline TTS, prompt user to download the default voice (~164 MB fp16, or ~88 MB int8).
- Store in app documents directory: `{app_docs}/models/kokoro/{voice_id}.onnx` + `voices.bin`.
- SQLite table `tts_models` tracks downloaded models, sizes, versions.
- Support: download progress, pause/resume, delete unused models.

**Binary size impact:**
- `ort` crate with static ONNX Runtime adds ~15–25 MB to the native binary (platform-dependent).
- This is acceptable for a desktop app; for mobile, use dynamic linking to the ONNX Runtime shared lib (~8 MB).
- Feature-gate offline TTS behind `cfg(feature = "offline-tts")` so web/test builds skip it.

**Consequences:**
- First-run experience requires a model download step.
- App size without models: ~30–50 MB (Flutter + Rust + ONNX Runtime). With one voice: ~120–220 MB total.
- Piper support is architecturally possible (same `ort` pipeline) but deferred to v2 unless Kokoro proves insufficient.

---

### ADR-F: Cloud Sync (Device Pairing, No Server)

**Decision:** LAN-only sync for v1 using mDNS discovery + direct TCP/TLS. Acknowledge that cross-network sync without a relay server is not feasible; provide an "optional user-provided relay" field for v2.

**Protocol (v1 — LAN only):**

```
1. Device A: Settings → "Pair Device" → generates a 6-digit pairing code
2. Device A: advertises via mDNS: _inkwave._tcp, TXT record = {device_id, code_hash}
3. Device B: Settings → "Pair Device" → discovers A via mDNS, user enters code
4. Mutual TLS handshake using code-derived key (HKDF from code + device_ids)
5. After pairing: exchange X25519 public keys, store in SQLite
6. Sync trigger: on app open, on book progress change (debounced 5s)
7. Sync payload: CRDT-based last-write-wins per field:
   - reading_progress (chapter, scroll_position, timestamp)
   - highlights (id, book_id, cfi, color, text, timestamp)
   - notes (id, highlight_id, text, timestamp)
   - deleted_items (tombstones with TTL)
8. Transport: length-prefixed MessagePack over TLS TCP socket
```

**What is NOT feasible without a server:**

| Feature | Feasibility | Why |
|---------|-------------|-----|
| LAN sync (same Wi-Fi) | ✅ Feasible | mDNS works on all platforms |
| Cross-network sync | ❌ Not feasible | NAT traversal requires STUN/TURN; hole-punching is unreliable; no signaling server to exchange candidates |
| Sync while both devices are not online simultaneously | ❌ Not feasible | No store-and-forward without a relay |
| Syncing book files | ⚠️ Partial | Large file transfer over LAN is fine; cross-network needs relay |

**v2 path — optional relay:**
- User provides their own relay endpoint (e.g., self-hosted, Tailscale, or a simple WebSocket relay).
- App stores relay URL in settings.
- Relay is a dumb pipe — end-to-end encrypted with X25519, relay sees ciphertext only.
- No InkWave-operated relay (privacy promise).

**Consequences:**
- v1 sync only works when both devices are on the same LAN and the app is open on both.
- This is a significant UX limitation — must be clearly communicated in the UI.
- The pairing code UX is modeled after Signal's safety number verification.
- mDNS: use `mdns-sd` crate in Rust, expose via FRB.

---

### ADR-G: Secure Storage of API Keys

**Decision:** Use `flutter_secure_storage` on the Flutter side for all API key storage.

**Per-platform backends (verified Oct 2026):**

| Platform | Backend | Notes |
|----------|---------|-------|
| iOS | Keychain | Secure Enclave on supported devices |
| Android | EncryptedSharedPreferences (API 23+) backed by Android Keystore | Hardware-backed on most devices |
| macOS | Keychain | Same as iOS |
| Windows | Windows Credential Manager + AES-GCM local file | Adequate for API keys |
| Linux | `libsecret` → GNOME Keyring / KWallet | Requires `libsecret-1-dev` at build time |

Ref: [pub.dev flutter_secure_storage](https://pub.dev/packages/flutter_secure_storage)

**Design:**
- API keys are stored/retrieved on the Dart side only.
- When making TTS/AI API calls, Dart reads the key from secure storage and passes it to Rust via FRB function argument.
- Keys are NEVER written to SQLite, logs, or crash reports.
- Keys are NEVER sent to any InkWave-operated endpoint (there are none).

**Key types stored:**

```
elevenlabs_api_key
openai_api_key
gemini_api_key
ollama_endpoint     // not secret, but stored here for consistency
calibre_server_url  // not secret
```

**Consequences:**
- Linux builds require `libsecret-1-dev` as a build dependency — document in README.
- No Rust-side secure storage needed; keys pass through FFI as `String` arguments to individual API calls (not persisted in Rust memory beyond the call).

---

## 2. Component Diagram & Data Flow

### System components

```
┌─────────────────────────────────────────────────────────┐
│                    Flutter App (Dart)                     │
│  ┌──────────┐ ┌──────────┐ ┌─────────┐ ┌─────────────┐ │
│  │ Library  │ │ Reader   │ │ Player  │ │ Settings    │ │
│  │ Screen   │ │ Screen   │ │ Widget  │ │ Screen      │ │
│  └────┬─────┘ └────┬─────┘ └────┬────┘ └──────┬──────┘ │
│       │             │            │              │        │
│  ┌────▼─────────────▼────────────▼──────────────▼──────┐ │
│  │              Riverpod Providers                      │ │
│  │  libraryProvider · readerProvider · playerProvider    │ │
│  │  settingsProvider · syncProvider                      │ │
│  └──────────────────────┬───────────────────────────────┘ │
│                         │ FRB (FFI)                       │
└─────────────────────────┼────────────────────────────────┘
                          │
┌─────────────────────────▼────────────────────────────────┐
│                   Rust Core (inkwave-core)                │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐  │
│  │ parsers  │ │   tts    │ │    ai    │ │    sync    │  │
│  │ epub/mobi│ │ router + │ │ provider │ │  mDNS +    │  │
│  │ /txt     │ │ engines  │ │ trait    │ │  CRDT      │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └─────┬──────┘  │
│       │             │            │              │         │
│  ┌────▼─────────────▼────────────▼──────────────▼──────┐  │
│  │                     db (SQLite via rusqlite)         │  │
│  └─────────────────────────────────────────────────────┘  │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              ort (ONNX Runtime) — offline TTS        │  │
│  └─────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
         │                    │
    ┌────▼────┐         ┌─────▼──────┐
    │ SQLite  │         │ Cloud APIs │
    │  .db    │         │ ElevenLabs │
    │ file    │         │ OpenAI     │
    └─────────┘         │ Gemini     │
                        │ Ollama     │
                        └────────────┘
```

### Data flow: Open Book → Parse → Display

```
User taps book in Library
  → libraryProvider.openBook(bookId)
  → FRB call: parse_book(file_path) → Rust
    → epub::parse(path):
      1. Unzip EPUB to temp dir
      2. Parse META-INF/container.xml → find OPF
      3. Parse OPF → metadata (title, author, cover_image)
      4. Parse spine → ordered list of chapter XHTML files
      5. For each chapter XHTML:
         a. Parse HTML (quick-xml or html5ever)
         b. Walk DOM → Vec<ContentBlock>
         c. Assign monotonic word_index to every text word
      6. Store metadata in SQLite (books table)
      7. Return BookDocument { metadata, chapters: Vec<Chapter> }
  → Dart receives BookDocument
  → readerProvider builds widget tree:
    - ListView.builder(itemCount: currentChapter.blocks.length)
    - Each ContentBlock → RichText widget with per-word TextSpans
  → Screen renders
```

### Data flow: Press Play → TTS → Audio + Timings → Highlight

```
User taps Play on current chapter/sentence
  → playerProvider.play(bookId, chapterIndex, startWordIndex)
  → FRB streaming call: tts_stream(text, provider, voice, apiKey)
      → Returns Stream<AudioChunk> to Dart

  Rust side (tts module):
    1. Split chapter text into sentences (sentence boundary detection)
    2. For each sentence:
       a. Call TTS engine:
          - Kokoro: ort inference → PCM + native word timings
          - ElevenLabs: HTTP POST to /text-to-speech/{voice}/stream-with-timestamps
                        → parse streaming JSON for alignment + audio chunks
          - OpenAI: HTTP POST to /v1/audio/speech → raw audio
                    → Fallback B: estimate timings from audio duration
          - Piper: ort inference → PCM + phoneme durations → word timing calc
       b. Yield AudioChunk { pcm_data, timings, chapter_index, sentence_index }

  Dart side:
    1. playerProvider listens to Stream<AudioChunk>
    2. Audio bytes → platform audio player (just_audio or audioplayers)
       - Enqueue PCM chunks as they arrive
       - Track playback position via audio player's position stream
    3. On each position tick (~16ms):
       - Find current WordTiming where start_ms <= position < end_ms
       - Update highlightedWordIndex in readerProvider
    4. readerProvider rebuilds only the affected TextSpan:
       - Current word: highlighted style (accent color background)
       - Previous words: dimmed style
       - Future words: normal style
    5. Auto-scroll: if highlighted word is below viewport,
       scroll ListView to bring it into view
```

---

## 3. Rust Crate Layout

```
backend/
├── Cargo.toml              # workspace root
└── crates/
    ├── inkwave-core/       # shared types, errors, config
    │   └── src/
    │       ├── lib.rs
    │       ├── error.rs    # InkwaveError enum (thiserror)
    │       ├── models.rs   # BookDocument, Chapter, ContentBlock, InlineSpan,
    │       │               # WordTiming, AudioChunk, AudioFormat
    │       ├── config.rs   # AppConfig, TtsProvider enum, AiProvider enum
    │       └── db.rs       # DB schema constants, migration SQL strings
    │
    ├── inkwave-parsers/    # EPUB, MOBI, TXT parsing
    │   └── src/
    │       ├── lib.rs
    │       ├── epub.rs     # EPUB → BookDocument
    │       ├── mobi.rs     # MOBI → BookDocument (v2, stub for now)
    │       └── txt.rs      # Plain text → BookDocument
    │
    ├── inkwave-tts/        # TTS routing, engines, timing
    │   └── src/
    │       ├── lib.rs
    │       ├── router.rs   # TtsRouter: selects engine based on config
    │       ├── kokoro.rs   # Kokoro ONNX inference + timing extraction
    │       ├── piper.rs    # Piper ONNX inference + phoneme→word timing
    │       ├── elevenlabs.rs # ElevenLabs API client + alignment parsing
    │       ├── openai.rs   # OpenAI TTS API client + estimated timing
    │       └── timing.rs   # WordTiming utilities, estimation fallback
    │
    ├── inkwave-ai/         # AI provider abstraction
    │   └── src/
    │       ├── lib.rs
    │       ├── provider.rs # AiProvider trait: summarize(), explain()
    │       ├── openai.rs   # OpenAI GPT implementation
    │       ├── gemini.rs   # Google Gemini implementation
    │       └── ollama.rs   # Ollama local implementation
    │
    ├── inkwave-sync/       # Device pairing + sync protocol
    │   └── src/
    │       ├── lib.rs
    │       ├── mdns.rs     # mDNS advertisement/discovery
    │       ├── pairing.rs  # Code generation, HKDF key derivation
    │       ├── transport.rs # TLS TCP connection, MessagePack codec
    │       └── crdt.rs     # LWW register, sync merge logic
    │
    ├── inkwave-db/         # SQLite database layer
    │   └── src/
    │       ├── lib.rs
    │       ├── migrations.rs # Embedded migration SQL, version tracking
    │       ├── books.rs    # CRUD for books table
    │       ├── progress.rs # CRUD for reading_progress
    │       ├── highlights.rs # CRUD for highlights + notes
    │       ├── settings.rs # CRUD for settings KV store
    │       └── tts_cache.rs # TTS audio cache management
    │
    ├── inkwave-ffi/        # flutter_rust_bridge glue
    │   └── src/
    │       ├── lib.rs      # FRB annotated public API functions
    │       └── api.rs      # Top-level functions Dart calls
    │
    └── inkwave-server/     # (kept from task-001, dev/CLI tool only)
        └── src/
            ├── lib.rs
            └── main.rs     # axum server for curl-based testing
```

**Dependency graph (crate → crate):**

```
inkwave-ffi → inkwave-core, inkwave-parsers, inkwave-tts,
              inkwave-ai, inkwave-sync, inkwave-db

inkwave-server → inkwave-core, inkwave-parsers, inkwave-tts,
                 inkwave-ai, inkwave-db

inkwave-parsers → inkwave-core
inkwave-tts     → inkwave-core
inkwave-ai      → inkwave-core
inkwave-sync    → inkwave-core, inkwave-db
inkwave-db      → inkwave-core
```

**Key external crates:**

| Crate | Purpose | Used by |
|-------|---------|---------|
| `rusqlite` | SQLite (bundled) | inkwave-db |
| `ort` | ONNX Runtime | inkwave-tts |
| `reqwest` | HTTP client (cloud APIs) | inkwave-tts, inkwave-ai |
| `serde` / `serde_json` | Serialization | all |
| `quick-xml` or `roxmltree` | XML/XHTML parsing | inkwave-parsers |
| `zip` | EPUB unzip | inkwave-parsers |
| `tokio` | Async runtime | inkwave-ffi, inkwave-server |
| `tracing` | Logging | all |
| `thiserror` | Error types | inkwave-core |
| `flutter_rust_bridge` | FFI codegen | inkwave-ffi |
| `mdns-sd` | mDNS | inkwave-sync |
| `rustls` | TLS | inkwave-sync |
| `rmp-serde` | MessagePack | inkwave-sync |
| `hkdf` / `x25519-dalek` | Key derivation | inkwave-sync |

**Note:** `sqlx` (listed in project overview) replaced by `rusqlite`. Reason: `rusqlite` is synchronous, simpler for FFI (FRB handles the threading), avoids compile-time query checking complexity. `sqlx` async model adds nothing when all DB calls are already dispatched to a Rust thread pool by FRB.

---

## 4. API Contract (FRB Function Signatures)

Since ADR-A chose FFI over HTTP, the "API" is Rust functions callable from Dart via FRB. Listed here in HTTP-like notation for clarity, but these are FFI calls, not endpoints.

The existing `inkwave-server` (axum) exposes the same surface as HTTP for dev tooling — listed in parentheses.

### Phase 1: Books & Library

```
# Parse and import a book
POST /books/import
  Request:  { file_path: String }
  Response: { book_id: i64, metadata: BookMetadata }
  FRB:      fn import_book(file_path: String) -> Result<ImportResult>

# List all books
GET /books
  Response: { books: Vec<BookSummary> }
  FRB:      fn list_books() -> Result<Vec<BookSummary>>

# Get full book content (one chapter)
GET /books/{id}/chapters/{index}
  Response: { chapter: Chapter }
  FRB:      fn get_chapter(book_id: i64, chapter_index: u32) -> Result<Chapter>

# Get book metadata
GET /books/{id}
  Response: { metadata: BookMetadata, toc: Vec<TocEntry> }
  FRB:      fn get_book(book_id: i64) -> Result<BookDetail>

# Delete a book
DELETE /books/{id}
  FRB:      fn delete_book(book_id: i64) -> Result<()>

# Get/update reading progress
GET  /books/{id}/progress
PUT  /books/{id}/progress  { chapter_index, scroll_pct, word_index }
  FRB:      fn get_progress(book_id: i64) -> Result<ReadingProgress>
  FRB:      fn save_progress(book_id: i64, progress: ReadingProgress) -> Result<()>
```

### Phase 2: TTS & Audio

```
# Start TTS streaming for a text range
POST /tts/stream
  Request:  { text: String, provider: TtsProvider, voice_id: String,
              api_key: Option<String>, start_word_index: u32 }
  Response: Stream<AudioChunk>  (SSE for HTTP / Stream for FRB)
  FRB:      fn tts_stream(req: TtsRequest) -> Result<Stream<AudioChunk>>

# List available voices for a provider
GET /tts/voices?provider={p}&api_key={k}
  Response: { voices: Vec<VoiceInfo> }
  FRB:      fn list_voices(provider: TtsProvider, api_key: Option<String>)
              -> Result<Vec<VoiceInfo>>

# Download offline TTS model
POST /tts/models/download
  Request:  { model_id: String }
  Response: Stream<DownloadProgress>
  FRB:      fn download_model(model_id: String) -> Result<Stream<DownloadProgress>>

# List available/downloaded models
GET /tts/models
  Response: { models: Vec<ModelInfo> }
  FRB:      fn list_models() -> Result<Vec<ModelInfo>>

# Delete a downloaded model
DELETE /tts/models/{id}
  FRB:      fn delete_model(model_id: String) -> Result<()>
```

### Phase 2: Annotations

```
# Create highlight
POST /books/{id}/highlights
  Request:  { chapter_index: u32, start_word: u32, end_word: u32,
              color: String, text_content: String }
  Response: { highlight_id: i64 }
  FRB:      fn create_highlight(h: NewHighlight) -> Result<i64>

# List highlights for a book
GET /books/{id}/highlights
  Response: { highlights: Vec<Highlight> }
  FRB:      fn list_highlights(book_id: i64) -> Result<Vec<Highlight>>

# Delete highlight
DELETE /highlights/{id}
  FRB:      fn delete_highlight(id: i64) -> Result<()>

# Add/update note on highlight
PUT /highlights/{id}/note
  Request:  { text: String }
  FRB:      fn set_note(highlight_id: i64, text: String) -> Result<()>

# Export highlights + notes
GET /books/{id}/highlights/export?format=md
  Response: { markdown: String }
  FRB:      fn export_highlights(book_id: i64, format: ExportFormat) -> Result<String>
```

### Phase 3: AI & Discover

```
# AI action on text
POST /ai/action
  Request:  { text: String, action: AiAction, provider: AiProvider,
              api_key: Option<String>, tone: Option<String> }
  Response: { result: String }
  FRB:      fn ai_action(req: AiRequest) -> Result<String>

# Search public book sources
GET /discover/search?source={gutenberg|standardebooks}&q={query}
  Response: { results: Vec<DiscoverResult> }
  FRB:      fn discover_search(source: BookSource, query: String)
              -> Result<Vec<DiscoverResult>>

# Download a discovered book
POST /discover/download
  Request:  { source: BookSource, source_id: String }
  Response: { book_id: i64 }
  FRB:      fn discover_download(source: BookSource, source_id: String) -> Result<i64>
```

### Streaming mechanism (FRB)

TTS audio streaming uses FRB's `StreamSink`:
- Rust function returns `impl Stream<Item = AudioChunk>`.
- FRB codegen wraps this as a Dart `Stream<AudioChunk>`.
- Flutter listens with `await for (final chunk in stream)`.
- Each `AudioChunk` contains ~1 sentence of audio + its `Vec<WordTiming>`.
- Backpressure: Rust yields one sentence at a time; next sentence starts generating while current plays.

---

## 5. SQLite Schema

```sql
-- Migration 001: initial schema
-- Managed by inkwave-db/migrations.rs

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    author TEXT,
    language TEXT,
    publisher TEXT,
    isbn TEXT,
    description TEXT,
    cover_image_path TEXT,          -- relative path to extracted cover
    file_path TEXT NOT NULL,        -- original import path
    file_hash TEXT NOT NULL,        -- SHA-256 of source file (dedup)
    format TEXT NOT NULL CHECK(format IN ('epub', 'mobi', 'txt')),
    chapter_count INTEGER NOT NULL DEFAULT 0,
    word_count INTEGER NOT NULL DEFAULT 0,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    imported_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_opened_at TEXT
);
CREATE UNIQUE INDEX idx_books_file_hash ON books(file_hash);

CREATE TABLE IF NOT EXISTS reading_progress (
    book_id INTEGER PRIMARY KEY REFERENCES books(id) ON DELETE CASCADE,
    chapter_index INTEGER NOT NULL DEFAULT 0,
    scroll_pct REAL NOT NULL DEFAULT 0.0,  -- 0.0 to 1.0
    word_index INTEGER NOT NULL DEFAULT 0, -- first visible word
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS highlights (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    chapter_index INTEGER NOT NULL,
    start_word INTEGER NOT NULL,
    end_word INTEGER NOT NULL,
    color TEXT NOT NULL DEFAULT '#FFD700',
    text_content TEXT NOT NULL,        -- the highlighted text
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    sync_id TEXT,                       -- UUID for cross-device sync
    is_deleted INTEGER NOT NULL DEFAULT 0  -- soft delete for sync
);
CREATE INDEX idx_highlights_book ON highlights(book_id, chapter_index);

CREATE TABLE IF NOT EXISTS notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    highlight_id INTEGER NOT NULL REFERENCES highlights(id) ON DELETE CASCADE,
    text TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    sync_id TEXT,
    is_deleted INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
-- Settings keys: theme, accent_color, font_family, font_size,
--   line_spacing, tts_provider, tts_voice, ai_provider,
--   offline_mode, calibre_url, sync_device_id, sync_paired_devices

CREATE TABLE IF NOT EXISTS tts_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    chapter_index INTEGER NOT NULL,
    sentence_index INTEGER NOT NULL,
    provider TEXT NOT NULL,
    voice_id TEXT NOT NULL,
    audio_path TEXT NOT NULL,          -- relative path to cached audio file
    timings_json TEXT NOT NULL,        -- JSON array of WordTiming
    duration_ms INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(book_id, chapter_index, sentence_index, provider, voice_id)
);
CREATE INDEX idx_tts_cache_book ON tts_cache(book_id, chapter_index);

CREATE TABLE IF NOT EXISTS tts_models (
    id TEXT PRIMARY KEY,               -- e.g. "kokoro-v1-fp16-en"
    engine TEXT NOT NULL,              -- "kokoro" | "piper"
    display_name TEXT NOT NULL,
    language TEXT NOT NULL,
    file_path TEXT,                    -- null if not downloaded
    file_size_bytes INTEGER NOT NULL,
    download_url TEXT NOT NULL,
    downloaded_at TEXT,
    version TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_peers (
    device_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    public_key_hex TEXT NOT NULL,
    paired_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_sync_at TEXT
);

CREATE TABLE IF NOT EXISTS sync_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_device_id TEXT NOT NULL REFERENCES sync_peers(device_id),
    direction TEXT NOT NULL CHECK(direction IN ('push', 'pull')),
    records_synced INTEGER NOT NULL,
    synced_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Migrations strategy

- Migrations are embedded as `const &str` arrays in `inkwave-db/src/migrations.rs`.
- On app startup, `inkwave_db::migrate()` reads `schema_version`, applies any un-applied migrations in order.
- Each migration is a SQL string with a version number.
- No ORM. Raw SQL via `rusqlite`.
- Pattern:

```rust
const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../sql/001_initial.sql")),
    // (2, include_str!("../sql/002_add_bookmarks.sql")),
];

pub fn migrate(conn: &Connection) -> Result<()> {
    let current = current_version(conn)?;
    for &(version, sql) in MIGRATIONS {
        if version > current {
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [version],
            )?;
        }
    }
    Ok(())
}
```

---

## 6. Flutter Architecture

### State management: Riverpod

**Decision:** Riverpod (v3.x with code generation).

**Comparison:**

| Factor | Riverpod | Bloc |
|--------|----------|------|
| Boilerplate | Low (with codegen) | High (event + state classes per feature) |
| BuildContext dependency | None — providers accessible anywhere | Needs context or BlocProvider |
| Stream support | Built-in via `StreamProvider` | Built-in via `emit.forEach` |
| Testability | Excellent — `ProviderContainer` override | Excellent — `blocTest` |
| Learning curve | Moderate | Moderate-high |
| Suitability for this project | Better — small team, rapid iteration, FRB streams map naturally to StreamProvider | Overkill for a solo/small-team project |

**Why Riverpod:**
- FRB returns Dart `Stream<AudioChunk>` — this maps directly to Riverpod's `StreamProvider` with zero ceremony.
- No BuildContext needed means providers can be used in background audio service callbacks.
- Code generation (`@riverpod` annotation) eliminates manual provider boilerplate.
- Solo developer project — Bloc's forced structure is overhead without a team to enforce consistency upon.

### Folder structure

```
app/lib/
├── main.dart
├── app.dart                    # MaterialApp, router, theme setup
├── core/
│   ├── bridge/                 # FRB generated bindings + thin wrappers
│   │   ├── native.dart         # Platform-specific bridge init
│   │   └── api.dart            # Generated by flutter_rust_bridge
│   ├── theme/
│   │   ├── app_theme.dart      # ThemeData factory
│   │   ├── tokens.dart         # Design tokens (colors, spacing, radii)
│   │   └── glassmorphism.dart  # Shared glass effect decorations
│   ├── router.dart             # go_router configuration
│   └── constants.dart
├── features/
│   ├── library/
│   │   ├── providers/          # libraryProvider, searchProvider
│   │   ├── screens/            # library_screen.dart
│   │   └── widgets/            # book_card.dart, carousel.dart
│   ├── reader/
│   │   ├── providers/          # readerProvider, highlightProvider
│   │   ├── screens/            # reader_screen.dart
│   │   ├── widgets/            # content_block_widget.dart, word_span.dart
│   │   └── models/             # Dart-side view models if needed
│   ├── player/
│   │   ├── providers/          # playerProvider (audio state, current timing)
│   │   ├── widgets/            # full_player.dart, mini_player.dart
│   │   └── services/           # audio_service integration
│   ├── discover/
│   │   ├── providers/
│   │   ├── screens/
│   │   └── widgets/
│   ├── settings/
│   │   ├── providers/          # settingsProvider, themeProvider
│   │   ├── screens/
│   │   └── widgets/
│   └── annotations/
│       ├── providers/
│       └── widgets/
└── shared/
    └── widgets/                # Common widgets (loading, error, etc.)
```

### Theming tokens

```dart
abstract class InkWaveTokens {
  // Backgrounds
  Color get background;
  Color get surface;
  Color get surfaceGlass;     // glassmorphism overlay color with alpha

  // Text
  Color get textPrimary;
  Color get textSecondary;
  Color get textMuted;

  // Reading
  Color get readerBackground;
  Color get readerText;
  Color get highlightActive;  // current word highlight
  Color get highlightDim;     // already-read words

  // Accents
  Color get accent;

  // Glass
  double get glassBlur;       // backdrop blur sigma
  double get glassOpacity;    // overlay opacity

  // Radii
  double get radiusSmall;     // 8
  double get radiusMedium;    // 16
  double get radiusLarge;     // 24
}

class ShadowTokens extends InkWaveTokens {
  Color get background => const Color(0xFF141414);
  Color get surface => const Color(0xFF1E1E1E);
  Color get surfaceGlass => const Color(0x33FFFFFF);  // white @ 20%
  Color get textPrimary => const Color(0xFFE8E8E8);
  Color get readerBackground => const Color(0xFF141414);
  Color get readerText => const Color(0xFFD0D0D0);
  // ...
}

class BlackIrisTokens extends InkWaveTokens {
  Color get background => const Color(0xFF080813);
  Color get surface => const Color(0xFF10101F);
  // ...
}

class CreamTokens extends InkWaveTokens {
  Color get background => const Color(0xFFF5F0E8);
  Color get surface => const Color(0xFFFFFFFF);
  Color get surfaceGlass => const Color(0x55000000);  // black @ 33%
  Color get textPrimary => const Color(0xFF2C2C2C);
  Color get readerBackground => const Color(0xFFF5F0E8);
  Color get readerText => const Color(0xFF333333);
  // ...
}
```

Theme selection stored in SQLite `settings` table (`key='theme'`), loaded by `themeProvider` at startup. Custom theme stores individual color hex values in settings.

---

## 7. Risks and Open Questions

Ranked by impact × probability.

| # | Risk / Question | Impact | Mitigation / Spike |
|---|----------------|--------|-------------------|
| 1 | **EPUB rendering correctness** — real-world EPUBs have complex CSS, nested tables, SVG, embedded fonts. Custom renderer will miss edge cases. | High | v1: support a "blessed subset" (paragraphs, headings, images, bold/italic, links). Complex elements render as plain text with a warning badge. Spike: parse 20 popular Project Gutenberg EPUBs, catalog unsupported elements. |
| 2 | **ONNX Runtime binary size on mobile** — `ort` static linking adds 15–25 MB per architecture. Universal iOS binary could be 50+ MB for ONNX alone. | High | Use dynamic linking to pre-built ONNX Runtime dylib (~8 MB). Ship dylib inside the app bundle. Measure actual sizes in a spike. |
| 3 | **Kokoro model quality on mobile CPU** — 82M params may be slow on older phones. | Medium | Spike: benchmark Kokoro fp16 on a mid-range Android device (e.g. Pixel 6a). If >3x realtime, default to int8 quantized model. If still slow, recommend cloud TTS on low-end devices. |
| 4 | **FRB codegen stability** — flutter_rust_bridge v2 is actively developed. Breaking changes between minor versions could block builds. | Medium | Pin FRB version. Run codegen in CI. Keep the FRB surface small — thin wrapper in `inkwave-ffi`, business logic in inner crates that don't depend on FRB. |
| 5 | **Word-index alignment between parser and TTS** — parser assigns word indices; TTS receives plain text and must return timings for the *same* word boundaries. Tokenization differences (hyphenation, contractions, punctuation) can cause misalignment. | Medium | Define a canonical word tokenizer in `inkwave-core` used by both parser and TTS. Unit test: parse → extract words → TTS → timings → verify indices match. |
| 6 | **LAN sync UX** — requiring same-WiFi, both apps open simultaneously is a poor user experience. Users may never successfully sync. | Medium | v1: clearly label as "LAN Sync (Beta)". v2: add optional relay. Consider dropping sync from v1 entirely if dev time is constrained. |
| 7 | **MOBI/AZW parsing** — MOBI format is undocumented and Amazon-proprietary. The `mobi` Rust crate is unmaintained. | Low | Defer MOBI to v2. v1 supports EPUB + TXT only. Most users with MOBI files can convert via Calibre. |
| 8 | **iOS App Store review** — "reader app" category may face additional scrutiny. Background audio entitlement must be justified. | Low | Background audio is justified by TTS narration. Prepare App Store review notes explaining the use case. |
| 9 | **Audio playback library** — `just_audio` vs `audioplayers` vs raw platform channels for PCM streaming. | Low | Spike: test `just_audio`'s ability to play raw PCM from a Dart `Stream`. If it requires file URLs, write chunks to a ring buffer file. |

### Open questions needing answers before Phase 2:

1. **Which audio playback package can play streaming PCM from memory?** Spike required.
2. **Can `audio_service` work alongside FRB's background thread?** Need to verify they don't conflict on iOS.
3. **Should TTS cache be audio files on disk or SQLite blobs?** Files are simpler for large audio; blobs are atomic. Recommend: files on disk, paths in SQLite (as currently designed).

---

## 8. Scope Cuts: v1 vs v2

### v1 (MVP — Phases 1-3)

| Feature | Included | Notes |
|---------|----------|-------|
| EPUB parsing + import | ✅ | Full support |
| TXT import | ✅ | Trivial |
| MOBI/AZW parsing | ❌ v2 | Crate unmaintained, format proprietary |
| Library (grid/list) | ✅ | |
| 3D carousel view | ❌ v2 | Complex 3D transforms, not core |
| Reader (scroll mode) | ✅ | |
| Reader (paginated mode) | ❌ v2 | Hard layout problem |
| Word-by-word highlight | ✅ | Core feature |
| TTS: Kokoro offline | ✅ | Primary offline engine |
| TTS: ElevenLabs | ✅ | With native timestamps |
| TTS: OpenAI | ✅ | With estimated timestamps |
| TTS: Piper | ❌ v2 | Kokoro covers offline |
| Audio player (full) | ✅ | Play/pause, speed, chapter nav |
| Mini-player | ✅ | Floating pill |
| Background audio | ✅ | iOS + Android |
| Sleep timer | ✅ | |
| Highlights | ✅ | |
| Notes | ✅ | |
| Export highlights | ✅ | Markdown |
| AI summarize/explain | ✅ | OpenAI + Gemini + Ollama |
| Themes (Shadow, Black Iris, Cream) | ✅ | |
| Custom theme | ❌ v2 | Accent color picker only in v1 |
| Glassmorphism UI | ✅ | Book cards, player chrome |
| Ink-fluid animations | ❌ v2 | Polish, not core |
| Project Gutenberg browse | ✅ | |
| Standard Ebooks browse | ✅ | |
| Calibre sync | ❌ v2 | Requires Calibre Content Server API integration |
| Cloud sync (LAN) | ⚠️ v1-beta | Labeled as beta, LAN only |
| Cloud sync (cross-network relay) | ❌ v2 | Requires relay infrastructure |
| Web platform | ❌ v2 | See ADR-B |
| Whisper forced alignment (OpenAI TTS) | ❌ v2 | Estimated timing sufficient for v1 |
| Bookmarks (separate from highlights) | ❌ v2 | Highlights at a single word serve as bookmarks for v1 |
| Per-book font/theme override | ❌ v2 | Global settings only in v1 |

### Rationale for major cuts:

- **MOBI:** Unmaintained parser, proprietary format. Calibre converts MOBI→EPUB trivially. Not worth the risk.
- **3D carousel:** Pure visual polish. Grid/list view covers all functionality. Carousel is a v2 delight feature.
- **Pagination:** Scroll mode works. Pagination requires solving cross-block page boundary detection with variable font sizes — significant effort for minimal v1 impact.
- **Web:** Architectural incompatibility (see ADR-B). No compromise delivers the core offline-first promise.
- **Calibre:** Requires reverse-engineering/documenting the Content Server API, handling authentication, file transfer. Not core reading experience.
- **Custom theme (full):** Accent color picker is v1. Full background/text/glass color customization is v2.

---

*End of architecture specification.*
