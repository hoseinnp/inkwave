# 🌊 InkWave — Project Overview

> A cross-platform ebook reader with AI-powered audio narration, glassmorphism UI, and a distraction-free reading experience.

---

## Vision

InkWave is a premium ebook reader that combines the calm of a paper book with the power of AI narration. Readers can listen to any book in their library with natural AI voices — word-by-word highlighted as audio plays — or read in silence across a beautifully themed interface. No accounts. No lock-in. Private by default.

---

## Platform

| Target | Method |
|--------|---------|
| iOS | Flutter |
| Android | Flutter |
| Web | Flutter Web |
| macOS | Flutter Desktop |
| Windows | Flutter Desktop |
| Linux | Flutter Desktop |

**Stack:** Flutter (UI) + Rust (backend via local HTTP / FFI)

---

## Architecture

```
┌─────────────────────────────────────┐
│         Flutter App (UI)            │
│  Library · Reader · Player · AI     │
└────────────┬────────────────────────┘
             │ HTTP / FFI
┌────────────▼────────────────────────┐
│       Rust Backend (axum)           │
│  Book parser · TTS router           │
│  AI provider · Calibre sync         │
│  Cloud sync (device pairing)        │
└────────────┬───────────┬────────────┘
             │           │
    ┌────────▼──┐   ┌────▼──────────┐
    │ Local DB  │   │ Cloud APIs    │
    │ (SQLite)  │   │ ElevenLabs    │
    │ Local TTS │   │ OpenAI TTS    │
    │ (Kokoro)  │   │ GPT/Gemini    │
    └───────────┘   └───────────────┘
```

- **Flutter** owns all UI rendering and state management
- **Rust (axum)** owns parsing, TTS routing, AI calls, sync, and local DB
- **SQLite** stores library metadata, reading progress, highlights, notes
- Mobile uses FFI; desktop/web uses local HTTP

---

## Book Formats Supported

| Format | Notes |
|--------|-------|
| EPUB | Primary format, full support |
| MOBI / AZW | Amazon Kindle format |
| Plain Text (.txt) | Simple import |

> PDF excluded intentionally — layout-based, not reflowable, incompatible with word-sync TTS.

---

## Library & Book Sources

| Source | Method |
|--------|--------|
| Local device files | File picker import |
| Cloud sync | Device pairing (no accounts) — sync progress & library across devices |
| Project Gutenberg | Browse & download free public domain books |
| Standard Ebooks | Curated, beautifully formatted free ebooks |
| Calibre | Connect to existing Calibre library via Calibre Content Server |

---

## Audio / TTS System

**Mode: Hybrid** — offline-first, cloud when available, user-configurable.

| Mode | Engine | Quality | Cost |
|------|--------|---------|------|
| Offline | Kokoro / Piper (Rust, on-device) | Good | Free |
| Online | ElevenLabs API | Excellent | Per-character |
| Online | OpenAI TTS API | Excellent | Per-character |

- User provides their own API keys (stored locally, never sent to InkWave servers)
- User can lock to offline or online mode in Settings
- TTS streams word timestamps → drives word-by-word highlight sync in reader

---

## Reading Experience

- **Word-by-word highlight** — each word lights up in sync with audio playback
- Smooth page transitions with ink-fluid animations
- Distraction-free fullscreen reading mode
- Font size, font family, line spacing controls
- Day / Night / Sepia mode per-book override

---

## Audio Player

**Full mode** (default):
- Play / Pause
- Speed control (0.5× – 4×)
- Voice selector (per-provider)
- Skip sentence forward / back
- Chapter navigation
- Bookmarks
- Sleep timer

**Mini-player** (collapsed):
- Floating pill — usable while browsing library
- Background playback on mobile (iOS / Android)
- Tap to expand back to full controls

---

## Annotations

| Feature | Details |
|---------|---------|
| Highlights | Select text → choose color → stored in SQLite |
| Notes | Attach text note to any highlight |
| Export | Export highlights + notes as Markdown or plain text |
| AI Summary | Select passage → Summarize / Explain via AI provider |

---

## AI Features

**User brings their own API key — no InkWave server costs, no data sharing.**

| Provider | Type |
|----------|------|
| OpenAI (GPT-4o) | Cloud |
| Google Gemini | Cloud |
| Ollama (local endpoint) | Local / self-hosted |

Rust backend implements a unified `AiProvider` trait — swapping providers requires only a Settings change. Hybrid: Ollama when offline, cloud when online (configurable).

**AI capabilities:**
- Summarize selected passage
- Explain selected passage (ELI5, academic, or custom tone)

---

## Themes & Visual Design

**Multiple switchable themes:**

| Theme | Background | Feel |
|-------|-----------|------|
| Shadow | `#141414` | Dark, minimal, ink-on-dark |
| Black Iris | `#080813` | Deep dark navy, premium night mode |
| Cream | `#F5F0E8` | Paper-like, warm and literary |
| Custom | User-defined | Accent color picker |

**Accent colors** (user-selectable):
- Frozen Blue `#A0BDDB`
- Citrus `#E4FD97`
- Warm Amber (default light)
- Pure White (default dark)

**Design language:**
- Glassmorphism — frosted glass book cards with backdrop blur and depth
- 3D perspective carousel for the library shelf view
- Smooth curves, organic transitions, ink-fluid animations
- Rounded corners, layered depth, no sharp UI elements
- Typography-first reading screen — nothing competes with the text

---

## Screens

```
App
├── Library
│   ├── 3D Carousel View (featured / recent)
│   ├── Grid View
│   ├── List View
│   └── Search + Filter
├── Book Detail
│   ├── Cover, metadata, description
│   ├── Add to library / Download
│   └── Start Reading / Continue
├── Reader
│   ├── Page view (paginated or scroll)
│   ├── Word-by-word highlight overlay
│   ├── Audio player (full / mini)
│   └── Annotation toolbar
├── Discover
│   ├── Project Gutenberg browser
│   ├── Standard Ebooks browser
│   └── Calibre server connect
└── Settings
    ├── Theme & appearance
    ├── TTS provider & API keys
    ├── AI provider & API keys
    ├── Cloud sync (device pairing)
    └── Calibre connection
```

---

## Key Rust Crates (planned)

| Crate | Purpose |
|-------|---------|
| `axum` | Local HTTP server |
| `epub` | EPUB parsing |
| `mobi` | MOBI parsing |
| `sqlx` | SQLite async queries |
| `reqwest` | HTTP calls to TTS/AI APIs |
| `serde` | JSON serialization |
| `tokio` | Async runtime |

---

## Privacy & Data

- ❌ No user accounts
- ❌ No telemetry
- ❌ No data sent to InkWave servers
- ✅ All data stored locally (SQLite)
- ✅ API keys stored in device secure storage only
- ✅ Cloud sync is peer-to-peer device pairing (no central store)

---

## Next Steps

1. Write full design spec (`docs/superpowers/specs/`)
2. Create implementation plan (phases: Rust core → Flutter shell → TTS → AI → Themes)
3. Scaffold project structure
4. Phase 1: Book parsing + local library (EPUB/MOBI/TXT)
5. Phase 2: Reader UI + word-sync TTS
6. Phase 3: Library carousel UI + themes
7. Phase 4: AI annotations
8. Phase 5: Cloud sync + Calibre + public sources

---

*Generated: 2026-10-10 | Project: InkWave | Stack: Flutter + Rust*
