# Shrimp Player

A shrimp-themed local music player in Rust, for desktop and Android. **The design lives in `docs/SPEC.md`.**
- Read it before starting work.
- Don't re-ask questions it already settles.
- Change it through a PR when a decision changes.

## Working rules

- **This is a learning project.** The queue, the audio analysis and the shrimp drawing are Mr.Bigg's to write, and Claude coaches (SPEC §9.2):
  - Claude scaffolds each piece with failing tests.
  - Hints come in three levels: a nudge, a bigger hint, then "show me".
  - Claude never rewrites Mr.Bigg's code unless asked.
  - "take it" hands a piece back to Claude.
- **Git:**
  - `main` always builds.
  - Work happens on branches and merges through PRs on `github.com/WozzySalmon/shrimp-player`.
  - Commits use the repo-local noreply identity.
- **CI only builds.** Run `cargo fmt --check`, `cargo clippy` and `cargo test` locally before merging, and never add test or clippy jobs to CI.
- **Never commit audio files or the signing keystore.** Generate test audio in code. The keystore lives at `~/.config/shrimp-player/shrimp.jks`.

## Environment on think

- Rust is in `~/.cargo/bin` but not on PATH, so run `export PATH=$HOME/.cargo/bin:$PATH` first. The toolchain is stable, with the Android targets installed.
- The Android SDK is in `~/Android/sdk`: NDK 29, build-tools 36, platforms 35/36. `adb` is in `~/Android/sdk/platform-tools`, and Java is 21.
- think has **no audio device and no display**.
  - Test desktop builds on lap-boi (`ssh root@100.107.46.45`).
  - Test Android on mrs-s26-ultra through wireless adb.
