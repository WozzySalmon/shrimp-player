# Shrimp Player: Specification

Status: **agreed design, pre-implementation** (2026-11-06).
Decisions marked `[Qn]` trace back to the design interview; change them by editing this file in a PR, not silently in code.

---

## 1. Purpose

A shrimp-themed local music player written in Rust.

- **Two goals, in this order:** a project for learning Rust, and a player Mr.Bigg actually uses every day. `[Q1]`
- **Audience for v1:** just Mr.Bigg. There are no public releases yet, but the repo is public. `[Q31, Q29]`
- **Repo:** `github.com/WozzySalmon/shrimp-player`, dual-licensed MIT OR Apache-2.0. `[Q12]`
- **App name:** "Shrimp Player". Android application ID: `shrimp.player`. `[Q12, Q36]`

### Non-goals for v1

- Streaming services. Jellyfin is planned for post-v1. `[Q3]`
- Opus decoding, which is post-v1 (§10). `[Q9]`
- A system tray, or playback after the desktop window closes. `[Q32]`
- Translations, telemetry and crash reporting. Logs go to a local file only.

---

## 2. Platforms and toolchain

| Platform | Machine | Role |
|---|---|---|
| Windows | big-boi | Mr.Bigg's main dev machine (Zed + rust-analyzer, Rust already installed) `[Q27]` |
| Linux (Wayland/niri) | lap-boi | Desktop testing. Claude runs builds there over SSH (root, key-only, from think's tailnet IP only) `[Q20, Q30]` |
| Android 13+ | mrs-s26-ultra | Phone target, with APKs installed over wireless adb across Tailscale `[Q2, Q20]` |
| Linux (headless) | think | Claude's dev box. It builds everything and tests the core, but has **no audio and no display** |

- Rust 2024 edition, stable toolchain.
- Android settings: `minSdk 33`, `targetSdk 36`, `compileSdk 36`. `[Q35]`
- Android ABIs: `arm64-v8a` (the phone) and `x86_64` (the emulator, if one is ever used).
- **Audio formats in v1:** MP3, FLAC, Ogg Vorbis, AAC/M4A, ALAC and WAV, decoded by Symphonia through rodio. `[Q9]`

---

## 3. Architecture

### 3.1 Workspace layout `[Q7]`

```
shrimp-player/
├── Cargo.toml                 # workspace
├── crates/
│   ├── shrimp-core/           # lib: no UI, no platform code. Fully testable headless.
│   ├── shrimp-ui/             # lib: Slint UI (.slint files + Rust glue), shared by desktop + Android
│   ├── shrimp-desktop/        # bin: desktop entry point, media keys, window lifecycle
│   ├── shrimp-android/        # cdylib: Android entry point (android_main) + JNI bridge
│   └── shrimp-icongen/        # bin: renders the procedural shrimp into icon PNG/ICO files
├── android/                   # Gradle project: Kotlin shim, manifest, signing, packaging
└── docs/
```

**Dependency direction:** `desktop` / `android` → `ui` → `core`.
`core` never depends on Slint, JNI or any OS-specific crate.

### 3.2 Runtime model

```
           ┌──────────────── shrimp-core ────────────────┐
 commands  │  Engine (one per process)                   │  state events
──────────►│   ├─ Library (scan + SQLite index)          ├────────────►  UI (Slint)
 (UI, media│   ├─ Queue (order, shuffle, repeat)         │               Android MediaSession
  keys,    │   ├─ Player trait ── RodioPlayer            │               souvlaki (desktop)
  Kotlin)  │   │        └─ AnalysisTap ─► ring buffer ──►├─ Analyzer ─► shrimp params
           │   └─ SessionState (persisted resume point)  │
           └─────────────────────────────────────────────┘
```

- **One `Engine` per process**, living in a process-global (`OnceLock`).
  - On Android, the Engine outlives the Activity: the foreground service keeps the process alive, and the Slint UI attaches and detaches. `[Q22]`
- **Commands go in through a channel**, and the Engine publishes **state events**: track changed, play/pause, position, queue changed, library scan progress.
  - Every front-end (Slint UI, souvlaki, Android MediaSession) is just a subscriber plus a command sender.
- **Threads:**
  - the UI thread;
  - the audio callback thread, owned by cpal through rodio;
  - one Engine worker thread for commands, the queue and DB writes;
  - a scanner thread that runs only while a scan is in progress.
- **The analysis runs on the UI side**, once per animation frame, using the newest window from the ring buffer.
  - It runs **only while the now-playing UI is visible**: no frames means no analysis. `[Q22]`

---

## 4. Core (`shrimp-core`)

### 4.1 Library scanning `[Q14]`

**Desktop:**
- The user configures one or more music folders in Settings.
- The scanner walks them recursively, keeps files with a supported extension, and compares `(path, mtime, size)` against the DB. Only new or changed files are re-read. `[Q15]`

**Android:**
- The Kotlin shim queries **MediaStore** (`READ_MEDIA_AUDIO`) and passes `(media_id, path, mtime, size)` rows to Rust over JNI.
- From there the flow matches desktop: Rust reads tags itself, straight from the file path. Media permission grants path access on API 33+.

**Common to both:**
- Tags are read with `lofty`: title, artist, album artist, album, track/disc number, duration and embedded cover art.
- If a tag is missing, the file name is used as the title, plus "Unknown Artist" and "Unknown Album".
- **Cover art:**
  - Thumbnails are extracted on demand and cached in the platform cache dir, keyed by track id.
  - When there's no cover, the shrimp-on-coral placeholder is used. `[Q24]`

### 4.2 Library index: SQLite `[Q15]`

`rusqlite` with the `bundled` feature (identical on all platforms). Starting schema, which evolves with versioned migrations:

```sql
CREATE TABLE tracks (
  id            INTEGER PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  media_id      INTEGER,            -- Android MediaStore id, NULL on desktop
  mtime         INTEGER NOT NULL,
  size          INTEGER NOT NULL,
  title         TEXT NOT NULL,
  artist        TEXT NOT NULL,
  album_artist  TEXT,
  album         TEXT NOT NULL,
  track_no      INTEGER,
  disc_no       INTEGER,
  duration_ms   INTEGER,
  has_cover     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX tracks_artist ON tracks(coalesce(album_artist, artist), album, disc_no, track_no);
CREATE INDEX tracks_album  ON tracks(album);

CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);  -- settings + session state
```

- Artists and albums are **derived by query**, not stored as tables, in v1.
- Search (post-v1) adds an FTS5 table over title, artist and album.

### 4.3 Playback `[Q16]`

- `trait Player`: `load(track)`, `play`, `pause`, `seek(Duration)`, `set_volume(f32)`, `position()`, plus an end-of-track notification.
- The v1 implementation is **`RodioPlayer`**, using rodio's Symphonia decoder.
- The trait exists so that a custom symphonia → ring buffer → cpal engine can replace rodio later (for gapless playback or tighter analysis timing) without touching anything else.
- **`AnalysisTap`** is a rodio `Source` wrapper:
  - It passes samples through unchanged, and pushes a mono downmix into a lock-free SPSC ring buffer (`rtrb` or `ringbuf`).
  - It must never block or allocate on the audio thread.
  - If the ring buffer is full, it drops the samples.

### 4.4 Queue `[Q8]`

*Written by Mr.Bigg (see §9.2).*

- An ordered list of track ids plus a current index.
- Operations: play this track now, which replaces the queue with its context (album, artist or all songs) starting at that track; add next; add to end; remove; move; clear.
- **Shuffle** keeps the original order, so turning shuffle off returns to it. The current track stays current when shuffle is toggled.
- **Repeat:** `Off | All | One`.
- Next/previous follow standard player rules: "previous" restarts the track if more than 3 s have played.
- The queue is pure logic with no I/O, so it is unit-tested exhaustively.

### 4.5 Session state `[Q8]`

Saved in the `kv` table:
- the queue and current index;
- the position, saved on pause, on track change and every ~10 s while playing;
- shuffle, repeat and volume.

On launch the app restores this **paused** at the saved position.

### 4.6 Audio analysis `[Q11]`

*Written by Mr.Bigg (see §9.2).*

- **Input:** the newest 2048 mono samples from the ring buffer each frame.
- **Spectrum:** a Hann window and a real FFT (`realfft`).
- **Bands:**

  | Band | Range | Drives |
  |---|---|---|
  | bass | 20–250 Hz | tail |
  | mids | 250 Hz–4 kHz | body sway |
  | highs | 4–16 kHz | antennae |

- Each band's energy is converted to dB and normalised against a slowly adapting peak, so quiet and loud tracks both animate.
- **Smoothing:** fast attack (~30 ms), slower release (~150 ms).
- **Onset (the "hop"):**
  - Spectral flux on the bass and low-mid bins, compared with an adaptive threshold (rolling mean + k·stddev over ~1 s).
  - A ~250 ms cooldown follows each hop.
  - The output is a one-shot `hop` impulse.
- **Output struct:** `ShrimpDrive { bass, mids, highs: f32 (0..1), hop: bool }`.
- **Latency:** the tap sits before the output buffer, so it is roughly 20–50 ms ahead of what you hear. That's acceptable for v1, so there's no compensation.
- **Tests** run headless on generated signals:
  - a sine at 60 Hz lights up bass only, a sine at 1 kHz lights up mids, and so on;
  - a click train triggers hops at its rate;
  - silence gives no hops.

---

## 5. The shrimp `[Q4, Q10, Q24]`

*Drawing and animation are written by Mr.Bigg (see §9.2).*

- **Drawn procedurally, with no art assets.**
  - The body is a chain of about 6 segments, plus a tail fan, a head with eye, 2 long antennae, and leg strokes.
  - Each frame Rust computes segment angles from the `ShrimpDrive` and mood, then builds an SVG-style path string that Slint's `Path` element draws.
- **Inputs to motion:**
  - bass → tail curl amplitude;
  - mids → body sway;
  - highs → antenna wiggle;
  - `hop` → a short vertical jump with a squash-and-stretch.
- **Moods (a state machine):**

  | Mood | When | Behaviour |
  |---|---|---|
  | Dancing | playing | audio-driven as above |
  | Sleeping | paused | curled up, slow breathing, an occasional bubble rising |
  | Swimming | library scan in progress | swims back and forth across its stage |
  | Confused | empty library / error | looks around, antennae droop. This doubles as the "no music found, pick a folder" screen |
  | (Placeholder) | missing cover art | a static shrimp-on-coral image used in place of the album art |

- **Mood transitions blend** over about 300 ms, with no hard cuts.
- **A shrimp preview tool** (`examples/` or a debug screen) has sliders for bass, mids and highs, a hop button and a mood picker, so the shrimp can be tuned without music playing.
- **App icon** `[Q33]`:
  - `shrimp-icongen` renders the shrimp into all the icon sizes needed: Android adaptive icon layers, Windows ICO and Linux PNGs.
  - Until it exists, 🦐 is the placeholder.

---

## 6. UI (`shrimp-ui`, Slint) `[Q13, Q17, Q18]`

### 6.1 Screens

- **Now Playing:**
  - the large shrimp stage;
  - the cover art;
  - title, artist and album;
  - a seek bar with elapsed and remaining time;
  - prev, play/pause and next;
  - shuffle and repeat toggles;
  - volume (a desktop slider; on Android the hardware buttons control media volume).
- **Library:** three tabs.
  - **Artists** → that artist's albums → that album's tracks.
  - **Albums**, shown as a grid of covers.
  - **Songs**, a flat virtualised list.
- **Mini player bar:**
  - shown on the Library screen;
  - small cover, title, play/pause, and a tiny shrimp;
  - tap it to open Now Playing.
  - On the phone it's the familiar bottom bar → full-screen player. On desktop it's a tab or panel switch.
- **Settings:**
  - music folders (desktop only), rescan, and theme (only "Deep Ocean" in v1);
  - an About section, which includes the **"Made with Slint" attribution** that Slint's royalty-free licence requires.

### 6.2 Theme "Deep Ocean" `[Q18]`

| Token | Value (initial) | Use |
|---|---|---|
| `bg-deep` | `#0B1D2E` | window background |
| `bg-mid` | `#12324A` | panels, list rows |
| `bg-shallow` | `#1C4E63` | hover, selected |
| `accent` | `#FF7F6E` | shrimp coral: play button, progress, highlights |
| `accent-soft` | `#FFB4A2` | secondary accent |
| `text` | `#E8F1F5` | primary text |
| `text-dim` | `#8FB3C4` | secondary text |

The palette lives in one Slint `global Theme` so a light "Sandy Shallows" theme can be added later.

### 6.3 Performance

- All long lists use Slint's virtualised `ListView`.
- **Spike target:** 5,000 rows scroll smoothly on the phone.
- Animation runs only while the now-playing stage is visible.

---

## 7. Platform integration

### 7.1 Desktop (`shrimp-desktop`) `[Q32, Q8]`

- **Media keys and the OS "now playing" panel** come from `souvlaki`: MPRIS on Linux, SMTC on Windows.
  - On Windows, SMTC needs the window's HWND, which comes from Slint's winit backend.
- **Closing the window quits the app.** A "keep playing in background" setting is post-v1.
- **Headphone-disconnect pause:** not in v1. `[Q23]`

### 7.2 Android (`shrimp-android` + `android/`) `[Q6, Q14, Q22, Q23]`

**The Rust side** (`shrimp-android`, a cdylib):
- `android_main` starts the Slint UI through the `android-activity` backend.
- Exported JNI functions accept commands and data from Kotlin.
- A registered Kotlin callback object receives Engine state events.

**The Kotlin shim** (`android/`, kept small; target ~200–400 lines):

| Component | Job |
|---|---|
| `MainActivity` | The Activity type `android-activity` needs (GameActivity or NativeActivity, decided during the spike). Requests the `READ_MEDIA_AUDIO` and `POST_NOTIFICATIONS` permissions. |
| `MediaScanner` | Queries MediaStore and hands the rows to Rust. |
| `PlaybackService` | A media3 `MediaSessionService` running in the foreground with `foregroundServiceType="mediaPlayback"`. It provides the notification, lock-screen and Bluetooth controls. |
| `RustPlayer` | A media3 `SimpleBasePlayer` that forwards play, pause, seek and skip to Rust, and mirrors Rust's state events back into the session. |
| Audio focus | Done through `AudioManager` by hand, since `SimpleBasePlayer` doesn't do it: **pause** on loss, **duck** on transient-can-duck, **pause and auto-resume** on transient loss (calls), **pause** on `ACTION_AUDIO_BECOMING_NOISY` (headphone or Bluetooth disconnect). |

**Swiping the app away from recents:**
- If music is playing, it keeps playing and the notification stays.
- If it's paused, the service stops itself. Dismissing the notification while paused also stops it. `[Q22]`

**Visibility:** the UI stops animating and analysing when the Activity isn't visible.

---

## 8. Build, signing and CI

### 8.1 Desktop

- `cargo run -p shrimp-desktop` on Windows and Linux.

### 8.2 Android

- `android/` is a Gradle project (using the wrapper). A Gradle task calls **`cargo-ndk`** to build `libshrimp_android.so` for each ABI into `jniLibs`.
- **Signing** `[Q31]`:
  - A **single keystore** signs both debug and release builds, so an APK from CI and an APK from think install over each other without wiping app data.
  - The keystore lives on think at `~/.config/shrimp-player/shrimp.jks`. It is **never in the repo**: `*.jks` and `keystore.properties` are gitignored.
  - CI gets it from Actions secrets: `ANDROID_KEYSTORE_B64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`.
- **Installing on the phone:** `adb` connects over Tailscale to mrs-s26-ultra, with Wireless debugging paired once.

### 8.3 CI: GitHub Actions, **build only** `[Q21, Q29]`

| Workflow | Trigger | Jobs |
|---|---|---|
| `build.yml` | pull requests to `main`, pushes to `main` | `cargo build` for the Linux and Windows desktop targets |
| `android.yml` | `workflow_dispatch`, tags `v*` | Signed APK, uploaded as an artifact |

**Rules for both workflows:**
- `concurrency: cancel-in-progress` for each branch.
- `paths-ignore` for docs-only changes (`docs/**`, `**/*.md`).
- `Swatinem/rust-cache`.
- **No test or clippy steps in CI.** Tests, `rustfmt` and `clippy` are run locally before merging.

---

## 9. Development workflow

### 9.1 Git `[Q28, Q37]`

- `main` always builds. All work happens on branches (`spike/slint`, `core/queue`, …) and merges through PRs.
- Claude reviews Mr.Bigg's PRs; Mr.Bigg reviews or skims Claude's.
- Commits from think are authored as `WozzySalmon <190420876+WozzySalmon@users.noreply.github.com>`, set in the repo-local git config.

### 9.2 Who writes what `[Q19, Q26]`

| Claude writes | Mr.Bigg writes (coached) |
|---|---|
| Workspace, build scripts, Gradle, CI | Queue and shuffle (§4.4), first |
| Kotlin shim and JNI bridge | Band analysis and onset detection (§4.6), second |
| DB layer, scanner, tag reading | Shrimp drawing and animation (§5), third |
| Player and AnalysisTap, Engine, platform glue | |
| Slint screens (Mr.Bigg is welcome to take any of them) | |

**How coaching works:**
1. **Scaffolds:** Claude provides the types, function signatures, doc comments, `todo!()` bodies and **failing tests**. The task is "make the tests pass". For the shrimp, the target is "make it look right" in the preview tool.
2. **Hints in three levels:** a nudge, then a bigger hint, then "show me". Claude never jumps straight to the answer.
3. **Reviews suggest; they don't rewrite.** Claude doesn't rewrite Mr.Bigg's code unless asked.
4. **Escape hatch:** "take it" means Claude finishes the piece and explains it.

### 9.3 Testing

- `shrimp-core` is fully tested headless on think.
- **Test audio is generated in code** (sines, click trains, noise, silence) and written to temporary WAV files when needed. **No copyrighted audio is ever committed.**
- UI and platform behaviour are tested by hand: Linux on lap-boi, Windows on big-boi, Android on the S26 Ultra.

---

## 10. Milestones `[Q25]`

| # | Milestone | Done when |
|---|---|---|
| 1 | **Slint spike** | Throwaway code; it is **go/no-go for Slint**, with egui/eframe as the fallback. Passes if **all** of these hold: (a) a procedural shrimp `Path` animates smoothly, driven by FFT bands of a generated test tone, on lap-boi **and** the phone; (b) a 5,000-row `ListView` scrolls smoothly on the phone; (c) an APK built on think installs and runs over wireless adb; (d) the Activity type for the Kotlin shim is chosen. |
| 2 | **Headless core** | Scanner, SQLite index, tags, `RodioPlayer`, `AnalysisTap`, Engine command and event loop, and session state all work and are tested on think. The queue scaffold is handed to Mr.Bigg. |
| 3 | **Desktop v1** | All §6 screens work, plus folder settings, souvlaki media keys and resume-on-launch. It runs on Windows and Linux. |
| 4 | **Android v1** | The Kotlin shim, MediaStore scanning, the foreground service and MediaSession, audio focus, and the signed APK in CI. Mr.Bigg uses it daily. |
| 5 | **Shrimp polish** | All moods, blended transitions, the onset hop, generated icons, and theme tuning. |
| 6 | **Post-v1 (backlog)** | Opus via a pure-Rust decoder (`opus-rs` or `ruopus`) plus a Symphonia adapter behind a cargo feature; playlists; search (FTS5); gapless playback (possibly a custom engine behind `Player`); ReplayGain; an equalizer; lyrics; a Jellyfin source; a desktop tray/background mode; desktop headphone-disconnect pause; the "Sandy Shallows" light theme; public releases. |

---

## 11. Known risks

| Risk | Mitigation |
|---|---|
| Slint plus a custom Kotlin service, and which Activity type to use | The spike settles it (milestone 1d) before any real Android work starts |
| Getting the HWND out of Slint for souvlaki/SMTC on Windows | Use Slint's winit backend and its window-handle access, checked early in milestone 3 |
| rodio seek accuracy on VBR MP3s | Accept it for v1; the `Player` trait allows a custom engine later |
| Path-string rebuilds every frame costing too much on the phone | Measured in the spike; fall back to fewer segments or cached shapes |
| No audio or display on think | Claude tests desktop builds on lap-boi over SSH, and the phone over adb |

---

## 12. Decision log

| Q | Decision |
|---|---|
| Q1 | Learning project that Mr.Bigg also uses daily |
| Q2/Q5 | Windows + Linux desktop, plus Android |
| Q3 | Local files; the music source sits behind a trait so Jellyfin can be added later |
| Q4 | A reacting shrimp mascot on top of a shrimp skin |
| Q6 | Background playback in v1, via a small Kotlin shim |
| Q7 | Cargo workspace with a UI-free core |
| Q8 | v1 feature cut as in §4, with media keys included |
| Q9 | Symphonia formats only; Opus is post-v1 via pure Rust |
| Q10 | Shrimp drawn procedurally |
| Q11 | Frequency bands plus onset hop |
| Q12 | GitHub (not Codeberg), MIT/Apache-2.0, "Shrimp Player" |
| Q13 | Slint, after a go/no-go spike |
| Q14 | MediaStore on Android, a folder list on desktop |
| Q15 | SQLite index with incremental rescan |
| Q16 | rodio behind a `Player` trait |
| Q17 | Big now-playing stage + library screen + mini bar |
| Q18 | "Deep Ocean" dark theme with a coral accent |
| Q19/Q26 | Split authorship with scaffolds, levelled hints and reviews |
| Q20/Q27/Q30 | Mr.Bigg codes on big-boi in Zed; lap-boi for Linux testing over SSH; phone over wireless adb |
| Q21/Q29 | Public repo; CI only builds and is minute-frugal |
| Q22 | Playback survives swiping the app away; no animation while hidden |
| Q23 | Standard Android audio-focus behaviour; none on desktop in v1 |
| Q24 | Five moods |
| Q25 | Milestones in the §10 order |
| Q31 | Personal use only; one shared signing keystore |
| Q32 | Closing the desktop window quits |
| Q33 | Icon generated from the shrimp; 🦐 placeholder until then |
| Q34 | `gh` on think, logged in as WozzySalmon |
| Q35 | minSdk 33, target 36 |
| Q36 | Application ID `shrimp.player` |
| Q37 | GitHub noreply address on commits |
