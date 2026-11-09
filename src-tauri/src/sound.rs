use crate::app::AppCore;
use crate::timer::Phase;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug)]
pub enum SoundKind {
    Work,
    ShortBreak,
    LongBreak,
}

impl SoundKind {
    pub fn from_phase(phase: Phase) -> Self {
        match phase {
            Phase::Work => SoundKind::Work,
            Phase::ShortBreak => SoundKind::ShortBreak,
            Phase::LongBreak => SoundKind::LongBreak,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            SoundKind::Work => "work",
            SoundKind::ShortBreak => "short-break",
            SoundKind::LongBreak => "long-break",
        }
    }

    /// Note sequence (frequency, duration ms), rendered as a soft bell.
    fn notes(self) -> Vec<(f32, u32)> {
        match self {
            // Focus finished: rising triad, feels like "done".
            SoundKind::Work => vec![(659.25, 260), (783.99, 260), (1046.5, 420)],
            // Break finished: two descending notes, gentle pull back.
            SoundKind::ShortBreak => vec![(880.0, 240), (587.33, 420)],
            SoundKind::LongBreak => vec![(987.77, 260), (739.99, 260), (493.88, 520)],
        }
    }
}

fn find_bin(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Players are tried in this order. `paplay`/`pw-play`/`aplay` only understand plain
/// wav, which is why bundled chimes are synthesised; `ffplay` is the fallback that can
/// also handle user-supplied mp3/ogg.
fn pick_player(sample: &Path) -> Option<(PathBuf, Vec<String>)> {
    for bin in ["pw-play", "paplay", "aplay"] {
        if let Some(p) = find_bin(bin) {
            let args = if bin == "aplay" {
                vec!["-q".into(), sample.display().to_string()]
            } else {
                vec![sample.display().to_string()]
            };
            return Some((p, args));
        }
    }
    if let Some(p) = find_bin("ffplay") {
        return Some((
            p,
            vec![
                "-nodisp".into(),
                "-autoexit".into(),
                "-loglevel".into(),
                "quiet".into(),
                sample.display().to_string(),
            ],
        ));
    }
    None
}

fn render_wav(notes: &[(f32, u32)], volume: f32) -> Vec<u8> {
    const RATE: usize = 44_100;
    let mut samples: Vec<f32> = Vec::new();
    for (freq, dur_ms) in notes {
        let n = RATE * (*dur_ms as usize) / 1000;
        let gap = RATE * 40 / 1000;
        samples.reserve(n + gap);
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let envelope = (-t * 3.2).exp();
            // Fundamental plus a muted partial gives it a bell-like edge without ringing.
            let value = (2.0 * std::f32::consts::PI * *freq * t).sin() * 0.7
                + (2.0 * std::f32::consts::PI * *freq * 2.0 * t).sin() * 0.2;
            samples.push(value * envelope);
        }
        samples.extend(std::iter::repeat(0.0f32).take(gap));
    }
    // Fade the tail so the last sample never clicks.
    let tail = (RATE / 50).min(samples.len());
    let fade_from = samples.len() - tail;
    for (i, s) in samples[fade_from..].iter_mut().enumerate() {
        *s *= 1.0 - i as f32 / tail as f32;
    }

    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    let gain = (volume.clamp(0.0, 1.0) / peak).min(4.0);
    let pcm: Vec<i16> = samples
        .iter()
        .map(|s| (s * gain).clamp(-1.0, 1.0).mul_add(32767.0, 0.0) as i16)
        .collect();

    let data_len = pcm.len() * 2;
    let mut out: Vec<u8> = Vec::with_capacity(44 + data_len);
    let rate: u32 = RATE as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Writes the scaled clip and plays it detached. Returns false when no audio helper
/// exists on the system, which the settings page surfaces as a hint.
pub fn play(ac: &AppCore, kind: SoundKind) {
    let settings = ac.settings_snapshot();
    let alerts = &settings.alerts;
    if !alerts.sound_enabled || alerts.volume == 0 {
        return;
    }

    let custom = match kind {
        SoundKind::Work => alerts.work_sound_path.clone(),
        _ => alerts.break_sound_path.clone(),
    };
    let custom_path = if custom.is_empty() { None } else { Some(PathBuf::from(&custom)) };

    if let Some(path) = custom_path.filter(|p| p.is_file()) {
        // User files keep their own format; ffplay understands the most.
        if let Some(bin) = find_bin("ffplay") {
            let volume = alerts.volume.to_string();
            let file = path.to_string_lossy().into_owned();
            spawn(bin, &["-nodisp", "-autoexit", "-loglevel", "quiet", "-volume", &volume, &file]);
            return;
        }
        if let Some((bin, args)) = pick_player(&path) {
            let _ = Command::new(bin)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
        return;
    }

    let dir = ac.sound_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let sample = dir.join(format!("{}.wav", kind.slug()));
    let bytes = render_wav(&kind.notes(), alerts.volume as f32 / 100.0);
    {
        let Ok(mut file) = fs::File::create(&sample) else {
            return;
        };
        if file.write_all(&bytes).is_err() {
            return;
        }
    }
    if let Some((bin, args)) = pick_player(&sample) {
        let _ = Command::new(bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

fn spawn(bin: PathBuf, args: &[&str]) {
    let _ = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

pub fn available_backend() -> Option<String> {
    ["pw-play", "paplay", "aplay", "ffplay"]
        .iter()
        .find(|b| find_bin(b).is_some())
        .map(|b| (*b).to_string())
}
