//! Moody procedural music. Sines and a soft guitar, not a bright square-wave loop.
//!
//! If the machine has no playback device, `Audio::start` stays silent and the
//! game keeps running.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::{Direction, ValueOr};

pub const ST_TITLE: u8 = 0;
pub const ST_CALM: u8 = 1;
pub const ST_MID: u8 = 2;
pub const ST_CLOSE: u8 = 3;
pub const ST_WARN: u8 = 4;
pub const ST_SHOP: u8 = 5;
pub const ST_STING: u8 = 6;
pub const ST_TRACED: u8 = 7;

pub const SFX_CLICK: u8 = 1;
pub const SFX_TICK: u8 = 2;
pub const SFX_BLIP: u8 = 3;
pub const SFX_WIPE: u8 = 4;
pub const SFX_BOUNCE: u8 = 5;
pub const SFX_BURN: u8 = 6;
pub const SFX_COIN: u8 = 7;
pub const SFX_ALARM: u8 = 8;
pub const SFX_CHORD: u8 = 9;
pub const SFX_DENY: u8 = 10;
pub const SFX_STATIC: u8 = 11;

const SR: f64 = 44_100.0;

struct Ctl {
    state: AtomicU8,
    music: AtomicU8,
    sfx: AtomicU8,
    adaptive: AtomicBool,
    run: AtomicBool,
    live: AtomicBool,
    queue: Mutex<Vec<u8>>,
}

pub struct Audio {
    ctl: Arc<Ctl>,
}

impl Audio {
    pub fn start() -> Self {
        let ctl = Arc::new(Ctl {
            state: AtomicU8::new(ST_TITLE),
            music: AtomicU8::new(80),
            sfx: AtomicU8::new(60),
            adaptive: AtomicBool::new(true),
            run: AtomicBool::new(true),
            live: AtomicBool::new(false),
            queue: Mutex::new(Vec::new()),
        });
        let bg = Arc::clone(&ctl);
        thread::spawn(move || play_loop(bg));
        Self { ctl }
    }

    pub fn silent() -> Self {
        Self {
            ctl: Arc::new(Ctl {
                state: AtomicU8::new(ST_TITLE),
                music: AtomicU8::new(0),
                sfx: AtomicU8::new(0),
                adaptive: AtomicBool::new(true),
                run: AtomicBool::new(false),
                live: AtomicBool::new(false),
                queue: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn live(&self) -> bool {
        self.ctl.live.load(Ordering::Relaxed)
    }

    pub fn set_state(&self, state: u8) {
        self.ctl.state.store(state, Ordering::Relaxed);
    }

    pub fn set_levels(&self, music: u8, sfx: u8) {
        self.ctl.music.store(music, Ordering::Relaxed);
        self.ctl.sfx.store(sfx, Ordering::Relaxed);
    }

    pub fn set_adaptive(&self, on: bool) {
        self.ctl.adaptive.store(on, Ordering::Relaxed);
    }

    pub fn sfx(&self, kind: u8) {
        if self.ctl.sfx.load(Ordering::Relaxed) == 0 || kind == 0 {
            return;
        }
        if let Ok(mut q) = self.ctl.queue.lock() {
            if q.len() < 16 {
                q.push(kind);
            }
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.ctl.run.store(false, Ordering::Relaxed);
    }
}

fn play_loop(ctl: Arc<Ctl>) {
    let Ok(pcm) = PCM::new("default", Direction::Playback, false) else {
        return;
    };
    if setup(&pcm).is_err() {
        return;
    }
    let Ok(io) = pcm.io_i16() else {
        return;
    };
    ctl.live.store(true, Ordering::Relaxed);
    let mut synth = Synth::new(1);
    let mut buf = [0i16; 1024];
    while ctl.run.load(Ordering::Relaxed) {
        synth.follow(&ctl);
        if let Ok(mut q) = ctl.queue.lock() {
            for kind in q.drain(..) {
                synth.trigger(kind);
            }
        }
        for sample in &mut buf {
            *sample = synth.next_i16();
        }
        if io.writei(&buf).is_err() {
            break;
        }
    }
    ctl.live.store(false, Ordering::Relaxed);
}

fn setup(pcm: &PCM) -> alsa::Result<()> {
    let hwp = HwParams::any(pcm)?;
    hwp.set_channels(1)?;
    hwp.set_rate(44_100, ValueOr::Nearest)?;
    hwp.set_format(Format::s16())?;
    hwp.set_access(Access::RWInterleaved)?;
    let _ = hwp.set_buffer_size_near(4096);
    pcm.hw_params(&hwp)?;
    let swp = pcm.sw_params_current()?;
    if let Ok(size) = pcm.hw_params_current()?.get_buffer_size() {
        swp.set_start_threshold(size)?;
    }
    pcm.sw_params(&swp)?;
    Ok(())
}

pub struct Synth {
    t: u64,
    state: u8,
    music: f32,
    sfx_vol: f32,
    adaptive: bool,
    pad: f32,
    bass: f32,
    arp: f32,
    guitar: f32,
    hat: f32,
    lp: f32,
    noise: u32,
    sting_age: u32,
    trace_age: u32,
    voices: [(u8, u32); 6],
}

impl Synth {
    pub fn new(seed: u32) -> Self {
        Self {
            t: 0,
            state: ST_TITLE,
            music: 0.8,
            sfx_vol: 0.6,
            adaptive: true,
            pad: 0.0,
            bass: 0.0,
            arp: 0.0,
            guitar: 0.0,
            hat: 0.0,
            lp: 0.0,
            noise: seed | 1,
            sting_age: u32::MAX,
            trace_age: u32::MAX,
            voices: [(0, 0); 6],
        }
    }

    fn follow(&mut self, ctl: &Ctl) {
        let mut state = ctl.state.load(Ordering::Relaxed);
        let adaptive = ctl.adaptive.load(Ordering::Relaxed);
        if !adaptive && matches!(state, ST_MID | ST_CLOSE | ST_WARN) {
            state = ST_CALM;
        }
        self.set_state(state);
        self.music = f32::from(ctl.music.load(Ordering::Relaxed)) / 100.0;
        self.sfx_vol = f32::from(ctl.sfx.load(Ordering::Relaxed)) / 100.0;
        self.adaptive = adaptive;
    }

    pub fn set_state(&mut self, state: u8) {
        if state == self.state {
            return;
        }
        if state == ST_STING {
            self.sting_age = 0;
        }
        if state == ST_TRACED {
            self.trace_age = 0;
        }
        self.state = state;
    }

    pub fn set_levels(&mut self, music: u8, sfx: u8) {
        self.music = f32::from(music) / 100.0;
        self.sfx_vol = f32::from(sfx) / 100.0;
    }

    pub fn trigger(&mut self, kind: u8) {
        if let Some(slot) = self.voices.iter_mut().find(|v| v.0 == 0) {
            *slot = (kind, 0);
        }
    }

    pub fn next_i16(&mut self) -> i16 {
        let y = self.next_f32().clamp(-1.0, 1.0);
        (y * 26_000.0) as i16
    }

    pub fn next_f32(&mut self) -> f32 {
        let t = self.t as f64 / SR;
        self.t += 1;
        let (tp, tb, ta, tg, th, cut) = targets(self.state);
        self.pad += (tp - self.pad) * 0.00008;
        self.bass += (tb - self.bass) * 0.00008;
        self.arp += (ta - self.arp) * 0.0001;
        self.guitar += (tg - self.guitar) * 0.00012;
        self.hat += (th - self.hat) * 0.00015;

        let beat = 60.0 / 74.0;
        let eighth = beat / 2.0;
        let step = (t / eighth) as usize;
        let ph = ((t % eighth) / eighth) as f32;
        let env = (1.0 - ph).powf(1.7);

        let trem = 0.85 + 0.15 * (t * 0.35).sin() as f32;
        let pad = (sine(110.0, t) + sine(130.81, t) + sine(164.81, t) + 0.35 * sine(110.4, t))
            * 0.25
            * trem;
        let bass_gate = if step % 8 == 0 || step % 8 == 4 {
            env
        } else if step % 8 == 2 {
            env * 0.35
        } else {
            0.0
        };
        let bass = sine(55.0, t) * bass_gate;
        let arp_hz = [220.0, 261.63, 329.63, 246.94, 196.0, 233.08, 293.66, 174.61];
        let arp = tri(arp_hz[step % 8], t) * env * 0.7;
        let tense = matches!(self.state, ST_CLOSE | ST_WARN);
        let g_hz = if tense {
            [185.0, 196.0, 174.61, 155.56]
        } else {
            [220.0, 196.0, 174.61, 146.83]
        };
        let g = g_hz[step % 4];
        let guitar = (sine(g, t) + 0.22 * sine(g * 2.01, t)) * env;

        let hat = if ph < 0.05 {
            self.hash_noise() * (1.0 - ph / 0.05)
        } else {
            0.0
        };

        let mut music = pad * self.pad * 0.55
            + bass * self.bass * 0.7
            + arp * self.arp * 0.18
            + guitar * self.guitar * 0.28
            + hat * self.hat * 0.12;

        if self.sting_age < u32::MAX {
            music += sting(self.sting_age);
            self.sting_age = self.sting_age.saturating_add(1);
            if self.sting_age > (SR as u32) * 2 {
                self.sting_age = u32::MAX;
            }
        }
        if self.state == ST_TRACED {
            music = traced_bed(self.trace_age, &mut self.noise);
            self.trace_age = self.trace_age.saturating_add(1);
        }

        self.lp += (music - self.lp) * cut;
        let sfx = self.sfx_mix();
        (self.lp * self.music + sfx * self.sfx_vol).tanh()
    }

    fn sfx_mix(&mut self) -> f32 {
        let mut y = 0.0;
        for i in 0..self.voices.len() {
            if self.voices[i].0 == 0 {
                continue;
            }
            let age = self.voices[i].1 as f32 / SR as f32;
            self.voices[i].1 = self.voices[i].1.saturating_add(1);
            let noise = self.hash_noise();
            let kind = self.voices[i].0;
            let (sample, done) = sfx_voice(kind, age, noise);
            y += sample;
            if done {
                self.voices[i] = (0, 0);
            }
        }
        y
    }

    fn hash_noise(&mut self) -> f32 {
        let mut x = self.noise;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        (x as i32 as f32) / (i32::MAX as f32)
    }
}

fn targets(state: u8) -> (f32, f32, f32, f32, f32, f32) {
    match state {
        ST_TITLE => (1.0, 0.15, 0.0, 0.0, 0.0, 0.018),
        ST_CALM => (0.9, 0.55, 0.0, 0.0, 0.0, 0.022),
        ST_MID => (0.75, 0.6, 0.45, 0.0, 0.35, 0.04),
        ST_CLOSE => (0.45, 0.35, 0.15, 1.0, 0.0, 0.05),
        ST_WARN => (0.25, 0.1, 0.0, 1.0, 0.0, 0.035),
        ST_SHOP => (0.7, 0.45, 0.2, 0.0, 0.05, 0.02),
        ST_STING => (0.4, 0.3, 0.0, 0.8, 0.0, 0.06),
        ST_TRACED => (0.0, 0.0, 0.0, 0.0, 0.0, 0.2),
        _ => (0.5, 0.2, 0.0, 0.0, 0.0, 0.02),
    }
}

fn sine(freq: f64, t: f64) -> f32 {
    (t * freq * std::f64::consts::TAU).sin() as f32
}

fn tri(freq: f64, t: f64) -> f32 {
    let p = (t * freq).rem_euclid(1.0);
    (4.0 * (p - 0.5).abs() - 1.0) as f32
}

fn sting(age: u32) -> f32 {
    let t = f64::from(age) / SR;
    let notes = [196.0, 233.08, 261.63, 311.13, 392.0];
    let i = ((t / 0.11) as usize).min(notes.len() - 1);
    let local = (t % 0.11) as f32;
    let env = (1.0 - (local / 0.11)).max(0.0).powi(2);
    let body = sine(notes[i], t) + 0.3 * sine(notes[i] * 1.5, t);
    body * env * 0.35 * (1.0 - (t as f32 / 1.6)).max(0.0)
}

fn traced_bed(age: u32, noise: &mut u32) -> f32 {
    let t = f64::from(age) / SR;
    let mut x = *noise;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *noise = x;
    let n = (x as i32 as f32) / (i32::MAX as f32);
    let burst = if t < 0.35 {
        n * (1.0 - t as f32 / 0.35) * 0.55
    } else {
        0.0
    };
    let g = sine(92.5, t) * 0.16 + sine(98.0, t) * 0.12;
    let fade = ((t as f32 - 0.2) / 1.2).clamp(0.0, 1.0);
    burst + g * fade
}

fn sfx_voice(kind: u8, age: f32, noise: f32) -> (f32, bool) {
    let env = |dur: f32| -> f32 { (1.0 - age / dur).max(0.0) };
    let done = |dur: f32| age >= dur;
    match kind {
        SFX_CLICK => (noise * env(0.03) * 0.4, done(0.03)),
        SFX_TICK => {
            let y = (age * 740.0 * std::f32::consts::TAU).sin() * env(0.045);
            (y * 0.35, done(0.045))
        }
        SFX_BLIP => {
            let f = 480.0 + age * 1800.0;
            let y = (age * f * std::f32::consts::TAU).sin() * env(0.09);
            (y * 0.3, done(0.09))
        }
        SFX_WIPE => (noise * env(0.16) * 0.35, done(0.16)),
        SFX_BOUNCE => {
            let p = if age < 0.06 { age } else { age - 0.07 };
            let y = if (0.0..0.06).contains(&age) || (0.07..0.13).contains(&age) {
                (p * 520.0 * std::f32::consts::TAU).sin() * (1.0 - p / 0.06).max(0.0)
            } else {
                0.0
            };
            (y * 0.3, done(0.14))
        }
        SFX_BURN => {
            let y = (age * 90.0 * std::f32::consts::TAU).sin() * 0.4 + noise * 0.25;
            (y * env(0.18), done(0.18))
        }
        SFX_COIN => {
            let f = if age < 0.07 { 988.0 } else { 1318.0 };
            let y = (age * f * std::f32::consts::TAU).sin() * env(0.16);
            (y * 0.28, done(0.16))
        }
        SFX_ALARM => {
            let gate = if (age * 12.0) as i32 % 2 == 0 {
                1.0
            } else {
                0.0
            };
            let y = (age * 440.0 * std::f32::consts::TAU).sin() * gate * env(0.28);
            (y * 0.3, done(0.28))
        }
        SFX_CHORD => {
            let y = (age * 196.0 * std::f32::consts::TAU).sin()
                + (age * 247.0 * std::f32::consts::TAU).sin()
                + (age * 294.0 * std::f32::consts::TAU).sin();
            (y * 0.12 * env(0.45), done(0.45))
        }
        SFX_DENY => {
            let y = (age * 110.0 * std::f32::consts::TAU).sin() * env(0.1);
            (y * 0.4, done(0.1))
        }
        SFX_STATIC => (noise * env(0.25) * 0.5, done(0.25)),
        _ => (0.0, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(state: u8, music: u8, n: usize) -> Vec<f32> {
        let mut s = Synth::new(7);
        s.set_state(state);
        s.set_levels(music, 0);
        // let the filter open
        (0..n).map(|_| s.next_f32()).collect()
    }

    #[test]
    fn silence_when_music_is_off() {
        let samples = render(ST_CALM, 0, 4000);
        let peak = samples.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(peak < 0.001, "{peak}");
    }

    #[test]
    fn calm_is_audible_and_warning_differs() {
        let calm = render(ST_CALM, 80, 8000);
        let warn = render(ST_WARN, 80, 8000);
        let energy = |v: &[f32]| v.iter().map(|s| s.abs()).sum::<f32>();
        assert!(energy(&calm) > 10.0, "{}", energy(&calm));
        let diff: f32 = calm
            .iter()
            .zip(warn.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 5.0, "{diff}");
    }

    #[test]
    fn traced_is_not_the_calm_loop() {
        let calm = render(ST_CALM, 80, 6000);
        let torn = render(ST_TRACED, 80, 6000);
        let diff: f32 = calm
            .iter()
            .zip(torn.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 5.0, "{diff}");
    }

    #[test]
    fn start_without_a_device_does_not_panic() {
        let audio = Audio::start();
        audio.set_state(ST_CALM);
        audio.sfx(SFX_BLIP);
        thread::sleep(std::time::Duration::from_millis(80));
        // No device in this environment: live stays false and the game continues.
        let _ = audio.live();
    }
}
