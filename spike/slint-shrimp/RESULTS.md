# Milestone 1: Slint spike results (2026-11-06)

**Verdict: GO for Slint.** All four exit criteria in SPEC §10 were met. This code is throwaway: the real app starts fresh in the workspace (SPEC §3.1).

## Criteria

| # | Criterion | Result |
|---|---|---|
| a | A procedural shrimp animates smoothly, driven by FFT bands, on lap-boi and the phone | **Pass.** lap-boi: 60 fps, with analysis plus path building at 0.075 ms per frame. S26 Ultra (Android 17): 60 fps at about 0.06–0.1 ms per frame. Tested with the synth and with a real MP3 ("friends wine"). Mr.Bigg: "very smooth", "the shrimp was jamming". |
| b | A 5,000-row `ListView` scrolls smoothly on the phone | **Pass.** It kept up with hard flings ("responsive"). lap-boi held 60 fps auto-scrolling at 3,000 px/s. |
| c | An APK built on think installs and runs over wireless adb | **Pass.** `cargo apk build --release` produced an 8.7 MB arm64 APK, signed with the shared keystore and installed over Tailscale (`adb connect 100.104.124.16:<port>`). |
| d | Choose the Activity type | **NativeActivity** (Slint's default, through `backend-android-activity-06`). It works as is. Still unproven: subclassing it from Kotlin inside a Gradle project. That is the first task of milestone 4. |

## Findings that change how the real app is built

1. **Audio reaches the analysis tap in bursts** of about 33 ms. Roughly 60–80% of display frames see no new samples.
   - The analyser must **hold, or better interpolate,** between bursts. Treating a gap as silence made the shrimp jitter.
   - A better design for the real analyser: read the window that ends at the *playback position* (from the audio clock) rather than "whatever has arrived".
2. **Drive the animation from the display, not a timer.**
   - `property <duration> tick: animation-tick(); changed tick => { frame() }` gives one callback per display frame.
   - The callback fired about twice per rendered frame on desktop (harmless). Use real frame-to-frame time (`dt`).
   - A fixed 16 ms timer drifts against the display's refresh and judders.
3. **Android 15+ draws edge to edge.** Pad the root layout by `Window.safe-area-insets`, otherwise the top and bottom controls sit under the system bars and can't be tapped.
4. **Normalising each band against its own recent peak** keeps every bar high (0.6–0.9) on real music. It looks lively but carries little information.
   - Tuning input for the real analysis (§4.6), which is Mr.Bigg's to write: compare bands against each other and use a longer-term loudness reference.
5. **The phone ran at 60 fps** even though its screen can do 120 Hz. That's probably Samsung's adaptive refresh rate deciding what this content needs; we can look into it later. 60 is fine.
6. **rodio 0.23 API:** `DeviceSinkBuilder::open_default_sink()`, `Player::connect_new(sink.mixer())`, `Decoder::try_from(File)`. The decoded file's own rate (44.1 kHz) is resampled to the device rate (48 kHz) automatically. The analysis tap sees the **source** rate, so the analyser must use that rate for its frequency bins.
7. **Logging on Android:** Slint redirects stdout to logcat under the `RustStdoutStderr` tag (`adb logcat -s RustStdoutStderr`).
8. **The placeholder shrimp looks like a finger.** That's expected: it has no shell segments, rostrum or proper tail fan. The real shrimp design is Mr.Bigg's (SPEC §5, milestone 5).

## How to run it

```bash
# desktop (think: headless display + null sink; lap-boi: real hardware)
cargo run --release                    # synth
SPIKE_FILE=song.mp3 cargo run --release
# switches: SPIKE_EXIT_AFTER, SPIKE_TAB=1, SPIKE_AUTOSCROLL=<px/s>, SPIKE_VOLUME=<0..1>

# Android (env as in CLAUDE.md, plus the keystore env vars)
CARGO_APK_RELEASE_KEYSTORE=~/.config/shrimp-player/shrimp.jks \
CARGO_APK_RELEASE_KEYSTORE_PASSWORD=... cargo apk build --release --lib
adb install -r target/release/apk/slint-shrimp-spike.apk
adb push song.mp3 /sdcard/Android/data/shrimp.player.spike/files/
```
