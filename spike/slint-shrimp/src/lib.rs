//! THROWAWAY SPIKE for milestone 1 (SPEC §10). Not production code.
//!
//! The synth, analyser and shrimp here are crude stand-ins for measuring
//! performance. The real analysis and shrimp are Mr.Bigg's to write.
//!
//! Env vars for headless runs:
//! - `SPIKE_EXIT_AFTER=<secs>`: print final stats and quit.
//! - `SPIKE_TAB=1`: start on the Library tab.
//! - `SPIKE_AUTOSCROLL=<px/s>`: scroll the library list continuously.
//! - `SPIKE_VOLUME=<0..1>`: output volume (analysis taps before volume).
//! - `SPIKE_FILE=<path>`: play a real audio file instead of the synth.
//!   On Android, the first audio file in the app's external files dir is used.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::fmt::Write as _;
use std::num::NonZero;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use rodio::{ChannelCount, SampleRate, Source};

slint::include_modules!();

const SR: u32 = 48_000;
const FFT_N: usize = 2048;

// ---------------------------------------------------------------- test signal

/// Endless 120 BPM test pattern: kick (bass), swelling chord (mids), off-beat
/// hats (highs). Sections vary so each band visibly rises and falls.
struct Synth {
    n: u64,
    ch: u16,
    cur: f32,
    kick_phase: f64,
    rng: u32,
    prev_noise: f32,
}

impl Synth {
    fn new() -> Self {
        Self {
            n: 0,
            ch: 0,
            cur: 0.0,
            kick_phase: 0.0,
            rng: 0x1234_5678,
            prev_noise: 0.0,
        }
    }

    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn frame(&mut self) -> f32 {
        let t = self.n as f64 / SR as f64;
        let tb = (t % 0.5) as f32; // time since beat
        let section4 = (t / 4.0) as i64;
        let section8 = (t / 8.0) as i64;

        let f = 45.0 + 90.0 * (-tb * 35.0).exp();
        self.kick_phase += 2.0 * std::f64::consts::PI * f as f64 / SR as f64;
        let kick_on = if section4 % 4 == 3 { 0.0 } else { 1.0 };
        let kick = (self.kick_phase.sin() as f32) * (-tb * 9.0).exp() * 0.9 * kick_on;

        let swell = 0.5 + 0.5 * (2.0 * std::f64::consts::PI * t / 6.0).sin() as f32;
        let tau = 2.0 * std::f64::consts::PI * t;
        let pad = ((tau * 261.63).sin() + (tau * 329.63).sin() + (tau * 392.0).sin()) as f32 / 3.0
            * 0.35
            * swell;

        let nz = self.noise();
        let hp = nz - self.prev_noise;
        self.prev_noise = nz;
        let hat_env = if tb >= 0.25 {
            (-(tb - 0.25) * 70.0).exp()
        } else {
            0.0
        };
        let hat_gain = if section8 % 2 == 0 { 0.25 } else { 0.07 };
        let hat = hp * hat_env * hat_gain;

        (kick + pad + hat).tanh() * 0.8
    }
}

impl Iterator for Synth {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.ch == 0 {
            self.cur = self.frame();
        }
        self.ch = (self.ch + 1) % 2;
        if self.ch == 0 {
            self.n += 1;
        }
        Some(self.cur)
    }
}

impl Source for Synth {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(SR).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

// ---------------------------------------------------------------- analysis tap

/// Passes samples through untouched and pushes a mono downmix into a lock-free
/// ring buffer. Never blocks or allocates; drops samples when the buffer is full.
struct Tap<S> {
    inner: S,
    prod: rtrb::Producer<f32>,
    channels: u16,
    idx: u16,
    acc: f32,
}

impl<S: Source> Tap<S> {
    fn new(inner: S, prod: rtrb::Producer<f32>) -> Self {
        let channels = inner.channels().get();
        Self {
            inner,
            prod,
            channels,
            idx: 0,
            acc: 0.0,
        }
    }
}

impl<S: Source> Iterator for Tap<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let s = self.inner.next()?;
        self.acc += s;
        self.idx += 1;
        if self.idx == self.channels {
            let _ = self.prod.push(self.acc / self.channels as f32);
            self.acc = 0.0;
            self.idx = 0;
        }
        Some(s)
    }
}

impl<S: Source> Source for Tap<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }
    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
}

// ---------------------------------------------------------------- analyser

struct Analyzer {
    sr: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    hist: VecDeque<f32>,
    input: Vec<f32>,
    spec: Vec<Complex<f32>>,
    prev_mag: Vec<f32>,
    peak: [f32; 3],
    level: [f32; 3],
    flux_hist: VecDeque<f32>,
    since_hop: f32,
    last_target: [f32; 3],
    /// Frames that arrived with no new audio since the previous frame.
    starved: u32,
}

impl Analyzer {
    fn new(sr: u32) -> Self {
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(FFT_N);
        let window = (0..FFT_N)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / (FFT_N - 1) as f32).cos())
            .collect();
        let spec = fft.make_output_vec();
        Self {
            sr: sr as f32,
            input: fft.make_input_vec(),
            prev_mag: vec![0.0; spec.len()],
            spec,
            fft,
            window,
            hist: VecDeque::with_capacity(FFT_N * 4),
            peak: [-60.0; 3],
            level: [0.0; 3],
            flux_hist: VecDeque::with_capacity(64),
            since_hop: 1.0,
            last_target: [0.0; 3],
            starved: 0,
        }
    }

    /// Returns (bass, mids, highs) in 0..1 and whether to hop this frame.
    fn update(&mut self, cons: &mut rtrb::Consumer<f32>, dt: f32) -> ([f32; 3], bool) {
        let mut fresh = 0;
        while let Ok(x) = cons.pop() {
            self.hist.push_back(x);
            fresh += 1;
        }
        while self.hist.len() > FFT_N {
            self.hist.pop_front();
        }
        self.since_hop += dt;

        // No new audio this frame (audio arrives in bursts): hold the last
        // target instead of treating the gap as silence.
        let mut target = self.last_target;
        let mut hop = false;
        if fresh == 0 {
            self.starved += 1;
        }
        if fresh > 0 && self.hist.len() == FFT_N {
            for (i, (x, w)) in self.hist.iter().zip(&self.window).enumerate() {
                self.input[i] = x * w;
            }
            self.fft.process(&mut self.input, &mut self.spec).ok();
            let hz_per_bin = self.sr / FFT_N as f32;
            let bands = [(20.0, 250.0), (250.0, 4000.0), (4000.0, 16000.0)];
            let mut flux = 0.0;
            let mut energy = [0.0f32; 3];
            for (k, c) in self.spec.iter().enumerate() {
                let f = k as f32 * hz_per_bin;
                let mag = c.norm();
                for (b, (lo, hi)) in bands.iter().enumerate() {
                    if f >= *lo && f < *hi {
                        energy[b] += mag * mag;
                    }
                }
                if f < 500.0 {
                    flux += (mag - self.prev_mag[k]).max(0.0);
                }
                self.prev_mag[k] = mag;
            }
            for b in 0..3 {
                let db = 10.0 * (energy[b] + 1e-9).log10();
                self.peak[b] = db.max(self.peak[b] - 6.0 * dt);
                target[b] = ((db - (self.peak[b] - 24.0)) / 24.0).clamp(0.0, 1.0);
            }
            self.last_target = target;
            let n = self.flux_hist.len().max(1) as f32;
            let mean = self.flux_hist.iter().sum::<f32>() / n;
            let var = self
                .flux_hist
                .iter()
                .map(|v| (v - mean).powi(2))
                .sum::<f32>()
                / n;
            if self.flux_hist.len() > 20 && flux > mean + 1.5 * var.sqrt() && self.since_hop > 0.25
            {
                hop = true;
                self.since_hop = 0.0;
            }
            self.flux_hist.push_back(flux);
            if self.flux_hist.len() > 60 {
                self.flux_hist.pop_front();
            }
        }
        let attack = 1.0 - (-dt / 0.03).exp();
        let release = 1.0 - (-dt / 0.15).exp();
        for b in 0..3 {
            let k = if target[b] > self.level[b] {
                attack
            } else {
                release
            };
            self.level[b] += (target[b] - self.level[b]) * k;
        }
        (self.level, hop)
    }
}

// ---------------------------------------------------------------- placeholder shrimp

type V = (f32, f32);
fn add(a: V, b: V) -> V {
    (a.0 + b.0, a.1 + b.1)
}
fn mul(a: V, s: f32) -> V {
    (a.0 * s, a.1 * s)
}
fn dir(a: f32) -> V {
    (a.cos(), a.sin())
}
fn mid(a: V, b: V) -> V {
    mul(add(a, b), 0.5)
}

/// Crude placeholder shrimp. Returns (body+tail fill, eye fill, antennae+legs strokes).
fn build_shrimp(t: f32, lv: [f32; 3], hop_y: f32) -> (String, String, String) {
    let [bass, mids, highs] = lv;
    let seg = [34.0, 32.0, 30.0, 27.0, 24.0, 20.0, 16.0];
    let rad = [34.0, 32.0, 29.0, 25.0, 20.0, 15.0, 10.0, 6.0];
    let mut pts: Vec<V> = vec![(265.0, 130.0 - hop_y)];
    let mut dirs: Vec<V> = vec![];
    let mut a = PI;
    for (i, len) in seg.iter().enumerate() {
        let curl = 0.10 + bass * 0.22 + mids * 0.12 * (t * 2.2 + i as f32 * 0.7).sin();
        a -= curl;
        dirs.push(dir(a));
        let p = add(*pts.last().unwrap(), mul(dir(a), *len));
        pts.push(p);
    }
    dirs.push(*dirs.last().unwrap());
    let d0 = (-1.0f32, 0.0f32);
    let normal = |d: V| (-d.1, d.0);
    let upper: Vec<V> = (0..pts.len())
        .map(|i| {
            let d = if i == 0 { d0 } else { dirs[i - 1] };
            add(pts[i], mul(normal(d), rad[i]))
        })
        .collect();
    let lower: Vec<V> = (0..pts.len())
        .map(|i| {
            let d = if i == 0 { d0 } else { dirs[i - 1] };
            add(pts[i], mul(normal(d), -rad[i]))
        })
        .collect();

    let mut ring: Vec<V> = upper.clone();
    ring.extend(lower.iter().rev());
    let mut body = String::with_capacity(1024);
    let _ = write!(body, "M {:.1} {:.1} ", ring[0].0, ring[0].1);
    for i in 1..ring.len() - 1 {
        let m = mid(ring[i], ring[i + 1]);
        let _ = write!(
            body,
            "Q {:.1} {:.1} {:.1} {:.1} ",
            ring[i].0, ring[i].1, m.0, m.1
        );
    }
    let last = ring[ring.len() - 1];
    let _ = write!(body, "L {:.1} {:.1} ", last.0, last.1);
    // head cap bulging forward
    let head_ctrl = add(pts[0], mul(d0, -rad[0] * 1.6));
    let head_ctrl = (pts[0].0 + rad[0] * 1.6, head_ctrl.1);
    let _ = write!(
        body,
        "Q {:.1} {:.1} {:.1} {:.1} Z ",
        head_ctrl.0, head_ctrl.1, upper[0].0, upper[0].1
    );
    // tail fan
    let pe = *pts.last().unwrap();
    let de = *dirs.last().unwrap();
    let ae = de.1.atan2(de.0);
    let ne = normal(de);
    for k in -1..=1 {
        let ang = ae + k as f32 * (0.5 + bass * 0.3);
        let tip = add(pe, mul(dir(ang), 22.0 + bass * 10.0));
        let b1 = add(pe, mul(ne, 5.0));
        let b2 = add(pe, mul(ne, -5.0));
        let _ = write!(
            body,
            "M {:.1} {:.1} L {:.1} {:.1} L {:.1} {:.1} Z ",
            b1.0, b1.1, tip.0, tip.1, b2.0, b2.1
        );
    }

    // eye
    let e = add(pts[0], (rad[0] * 0.55, -rad[0] * 0.35));
    let r = 5.0;
    let eye = format!(
        "M {:.1} {:.1} A {r} {r} 0 1 0 {:.1} {:.1} A {r} {r} 0 1 0 {:.1} {:.1} Z",
        e.0 - r,
        e.1,
        e.0 + r,
        e.1,
        e.0 - r,
        e.1
    );

    // antennae + legs
    let mut lines = String::with_capacity(512);
    let s = add(pts[0], (rad[0] * 0.9, -rad[0] * 0.5));
    for k in 0..2 {
        let kf = k as f32;
        let wig = highs * 14.0 * (t * 18.0 + kf * 1.7).sin();
        let c1 = add(s, (40.0, -35.0 - kf * 12.0 + wig));
        let c2 = add(s, (85.0, -10.0 - kf * 25.0 - wig));
        let end = add(s, (125.0 - kf * 10.0, 30.0 - kf * 45.0 + wig * 0.5));
        let _ = write!(
            lines,
            "M {:.1} {:.1} C {:.1} {:.1} {:.1} {:.1} {:.1} {:.1} ",
            s.0, s.1, c1.0, c1.1, c2.0, c2.1, end.0, end.1
        );
    }
    for i in 1..=5 {
        let base = lower[i];
        let d = dirs[i - 1];
        let down = mul(normal(d), -1.0);
        let swing = (t * 8.0 + i as f32).sin() * 0.5 * (0.3 + mids);
        let ang = down.1.atan2(down.0) + swing;
        let tip = add(base, mul(dir(ang), 16.0));
        let _ = write!(
            lines,
            "M {:.1} {:.1} L {:.1} {:.1} ",
            base.0, base.1, tip.0, tip.1
        );
    }
    (body, eye, lines)
}

// ---------------------------------------------------------------- library rows

fn fake_tracks(n: usize) -> Vec<TrackRow> {
    let adj = [
        "Salty", "Coral", "Tidal", "Deep", "Glowing", "Peeled", "Sunken", "Brine", "Krill", "Reef",
    ];
    let noun = [
        "Shrimp", "Current", "Shell", "Lagoon", "Prawn", "Kelp", "Abyss", "Bubble", "Pincer",
        "Wave",
    ];
    let artist = [
        "The Crustaceans",
        "Mantis & Co",
        "Pistol Shrimp",
        "DJ Krill",
        "Tiger Prawns",
        "Sea Monkeys",
    ];
    (0..n)
        .map(|i| {
            let secs = 120 + (i * 7919) % 300;
            TrackRow {
                title: format!(
                    "{} {} #{}",
                    adj[i % adj.len()],
                    noun[(i / 10) % noun.len()],
                    i + 1
                )
                .into(),
                artist: artist[(i / 3) % artist.len()].into(),
                duration: format!("{}:{:02}", secs / 60, secs % 60).into(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------- app

fn env_f32(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.parse().ok()
}

fn audio_ext(p: &std::path::Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("mp3" | "flac" | "ogg" | "m4a" | "wav")
    )
}

fn pick_file() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("SPIKE_FILE") {
        return Some(p.into());
    }
    if cfg!(target_os = "android") {
        let dir =
            std::path::Path::new("/storage/emulated/0/Android/data/shrimp.player.spike/files");
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| audio_ext(p))
            .collect();
        files.sort();
        return files.into_iter().next();
    }
    None
}

/// Opens the default output and plays `source`. Without a device, pulls the
/// source on a thread at roughly real-time pace so the analyser still runs.
fn start_audio<S: Source + Send + 'static>(
    source: S,
) -> (
    String,
    Option<rodio::MixerDeviceSink>,
    Option<rodio::Player>,
) {
    match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(sink) => {
            let cfg = sink.config();
            let note = format!("out {} Hz {} ch", cfg.sample_rate(), cfg.channel_count());
            let p = rodio::Player::connect_new(sink.mixer());
            if let Some(v) = env_f32("SPIKE_VOLUME") {
                p.set_volume(v);
            }
            p.append(source);
            (note, Some(sink), Some(p))
        }
        Err(e) => {
            let mut src = source;
            let per_10ms = (src.sample_rate().get() * src.channels().get() as u32 / 100) as usize;
            std::thread::spawn(move || {
                loop {
                    for _ in 0..per_10ms {
                        src.next();
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            });
            (format!("NO AUDIO DEVICE ({e}); silent pacing"), None, None)
        }
    }
}

pub fn run() {
    let ui = AppWindow::new().expect("create window");
    ui.set_tracks(Rc::new(slint::VecModel::from(fake_tracks(5000))).into());
    if std::env::var("SPIKE_TAB").as_deref() == Ok("1") {
        ui.set_tab(1);
    }

    // Audio: synth -> tap -> rodio. If no device, pace the synth on a thread so
    // the analyser still gets data.
    let (prod, cons) = rtrb::RingBuffer::<f32>::new(SR as usize);
    let file = pick_file();
    let decoded = file.as_ref().and_then(|p| {
        let f = std::fs::File::open(p)
            .map_err(|e| eprintln!("open {p:?}: {e}"))
            .ok()?;
        rodio::Decoder::try_from(f)
            .map_err(|e| eprintln!("decode {p:?}: {e}"))
            .ok()
    });
    let (label, src_rate, (audio_note, sink, player)) = match decoded {
        Some(dec) => {
            let name = file
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let rate = dec.sample_rate().get();
            (name, rate, start_audio(Tap::new(dec, prod)))
        }
        None => (
            "test synth".to_string(),
            SR,
            start_audio(Tap::new(Synth::new(), prod)),
        ),
    };
    let audio_note = format!("{label} @ {src_rate} Hz -> {audio_note}");
    let player = Rc::new(player);

    {
        let player = player.clone();
        let weak = ui.as_weak();
        ui.on_toggle_play(move || {
            let Some(ui) = weak.upgrade() else { return };
            if let Some(p) = player.as_ref() {
                if p.is_paused() {
                    p.play()
                } else {
                    p.pause()
                }
                ui.set_playing(!p.is_paused());
            }
        });
    }

    // Frame counter via the renderer (not all renderers support it).
    let frames = Rc::new(Cell::new(0u32));
    let notifier_ok = {
        let frames = frames.clone();
        ui.window()
            .set_rendering_notifier(move |state, _| {
                if let slint::RenderingState::AfterRendering = state {
                    frames.set(frames.get() + 1);
                }
            })
            .is_ok()
    };

    let analyzer = RefCell::new(Analyzer::new(src_rate));
    let cons = RefCell::new(cons);
    let start = Instant::now();
    let last = Cell::new(Instant::now());
    let hop_env = Cell::new(0.0f32);
    let hops = Cell::new(0i32);
    let ticks = Cell::new(0u32);
    let work_ns = Cell::new(0u128);
    let window_start = Cell::new(Instant::now());
    let exit_after = env_f32("SPIKE_EXIT_AFTER");
    let autoscroll = env_f32("SPIKE_AUTOSCROLL");
    let summary = Rc::new(RefCell::new(Vec::<String>::new()));

    {
        let weak = ui.as_weak();
        let summary = summary.clone();
        ui.on_frame(move || {
            let Some(ui) = weak.upgrade() else { return };
            let now = Instant::now();
            let dt = (now - last.get()).as_secs_f32().min(0.1);
            last.set(now);
            let t = (now - start).as_secs_f32();

            let w0 = Instant::now();
            let (lv, hop) = analyzer.borrow_mut().update(&mut cons.borrow_mut(), dt);
            if hop {
                hop_env.set(1.0);
                hops.set(hops.get() + 1);
            }
            hop_env.set((hop_env.get() - dt * 3.5).max(0.0));
            let p = 1.0 - hop_env.get();
            let hop_y = if hop_env.get() > 0.0 { 4.0 * p * (1.0 - p) * 28.0 } else { 0.0 };
            let (body, eye, lines) = build_shrimp(t, lv, hop_y);
            work_ns.set(work_ns.get() + w0.elapsed().as_nanos());

            ui.set_body_path(body.into());
            ui.set_eye_path(eye.into());
            ui.set_line_path(lines.into());
            ui.set_bass(lv[0]);
            ui.set_mids(lv[1]);
            ui.set_highs(lv[2]);
            ui.set_hops(hops.get());
            if let Some(px) = autoscroll {
                // ListView viewport-y is negative when scrolled; wrap at ~4,900 rows.
                let mut y = ui.get_list_y() - px * dt;
                if y < -(52.0 * 4900.0) {
                    y = 0.0;
                }
                ui.set_list_y(y);
            }
            ticks.set(ticks.get() + 1);

            let span = (now - window_start.get()).as_secs_f32();
            if span >= 1.0 {
                let fps = if notifier_ok { format!("{:.0}", frames.get() as f32 / span) } else { "n/a".into() };
                let line = format!(
                    "t={:.0}s fps {} | frames/s {:.0} (no new audio: {}) | analysis+path {:.3} ms/frame | {} | levels {:.2} {:.2} {:.2} | hops {}",
                    t,
                    fps,
                    ticks.get() as f32 / span,
                    std::mem::take(&mut analyzer.borrow_mut().starved),
                    work_ns.get() as f64 / 1e6 / ticks.get().max(1) as f64,
                    audio_note,
                    lv[0], lv[1], lv[2],
                    hops.get()
                );
                ui.set_stats(line.clone().into());
                println!("{line}");
                summary.borrow_mut().push(line);
                frames.set(0);
                ticks.set(0);
                work_ns.set(0);
                window_start.set(now);
            }
            if let Some(secs) = exit_after {
                if t >= secs {
                    let _ = slint::quit_event_loop();
                }
            }
        });
    }

    ui.run().expect("run event loop");
    drop(player);
    drop(sink);
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).expect("init slint android backend");
    run();
}
