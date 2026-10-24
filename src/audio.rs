//! Tiny square-wave beeper output (Apple II speaker nostalgia).
//!
//! - Native builds: cpal plays the tones as a mono sample stream.
//! - Web builds: the same tones are pushed into a WebAudio oscillator queue
//!   implemented in `web/gl.js` and imported as `env.alien_audio_tone`.

use crate::game::Sfx;

/// A single tone to play.
#[derive(Clone, Copy)]
pub struct Tone {
    pub freq: f32,
    pub secs: f32,
}

/// Map a game sound effect to a short beep sequence.
pub fn tones_for(sfx: Sfx) -> Vec<Tone> {
    let mut v = Vec::new();
    match sfx {
        Sfx::Start => {
            for f in [330.0, 440.0, 554.0, 659.0] {
                v.push(Tone {
                    freq: f,
                    secs: 0.07,
                });
            }
        }
        Sfx::Land { pad } => {
            let notes: &[f32] = match pad {
                0 => &[392.0, 523.0],
                1 => &[523.0, 659.0, 784.0],
                _ => &[659.0, 784.0, 1046.0, 1318.0],
            };
            for f in notes {
                v.push(Tone {
                    freq: *f,
                    secs: 0.06,
                });
            }
        }
        Sfx::Crash => {
            for f in [240.0, 180.0, 130.0, 90.0] {
                v.push(Tone {
                    freq: f,
                    secs: 0.14,
                });
            }
        }
        Sfx::LifeUp => {
            for f in [880.0, 1108.0, 1318.0] {
                v.push(Tone {
                    freq: f,
                    secs: 0.07,
                });
            }
        }
    }
    v
}

// ---- Native (cpal) implementation -------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::Tone;
    use std::collections::VecDeque;
    use std::sync::mpsc::{channel, Receiver, Sender};

    /// Master amplitude for the square wave.
    const GAIN: f32 = 0.16;

    /// A degenerate square-wave voice that plays queued tones.
    struct Synth {
        queue: VecDeque<Vec<Tone>>,
        cur: VecDeque<Tone>,
        freq: f32,
        remaining: f32,
        phase: f32,
    }

    impl Synth {
        fn new() -> Synth {
            Synth {
                queue: VecDeque::new(),
                cur: VecDeque::new(),
                freq: 0.0,
                remaining: 0.0,
                phase: 0.0,
            }
        }

        fn push(&mut self, tones: Vec<Tone>) {
            self.queue.push_back(tones);
        }

        fn next_sample(&mut self, sr: f32) -> f32 {
            if self.freq <= 0.0 || self.remaining <= 0.0 {
                if let Some(t) = self.cur.pop_front() {
                    self.freq = t.freq;
                    self.remaining = t.secs;
                    self.phase = 0.0;
                } else if let Some(batch) = self.queue.pop_front() {
                    self.cur.extend(batch);
                    if let Some(t) = self.cur.pop_front() {
                        self.freq = t.freq;
                        self.remaining = t.secs;
                        self.phase = 0.0;
                    }
                } else {
                    return 0.0;
                }
            }
            let s = if self.phase < 0.5 { GAIN } else { -GAIN };
            self.phase += self.freq / sr;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            self.remaining -= 1.0 / sr;
            s
        }
    }

    /// Send side of the audio pipe. If no output device was found this is
    /// `None` and the game just runs silently.
    #[derive(Clone)]
    pub struct SfxOut {
        tx: Option<Sender<Vec<Tone>>>,
    }

    impl SfxOut {
        pub fn play(&self, tones: Vec<Tone>) {
            if let Some(tx) = &self.tx {
                let _ = tx.send(tones);
            }
        }
    }

    fn build_stream<T>(
        device: cpal::Device,
        config: cpal::StreamConfig,
        channels: usize,
        mut synth: Synth,
        rx: Receiver<Vec<Tone>>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        use cpal::traits::DeviceTrait;
        let sr = config.sample_rate as f32;
        device.build_output_stream(
            &config,
            move |data: &mut [T], _| {
                while let Ok(batch) = rx.try_recv() {
                    synth.push(batch);
                }
                for frame in data.chunks_mut(channels.max(1)) {
                    let sample: T = T::from_sample(synth.next_sample(sr));
                    for c in frame.iter_mut() {
                        *c = sample;
                    }
                }
            },
            |err| eprintln!("audio stream error: {err}"),
            None,
        )
    }

    /// Try to open the default output device. Returns a silent (`None`) output
    /// when unavailable so the game still runs headless.
    pub fn init() -> SfxOut {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            return SfxOut { tx: None };
        };
        let Ok(supported) = device.default_output_config() else {
            return SfxOut { tx: None };
        };
        let config: cpal::StreamConfig = supported.config();
        let channels = config.channels as usize;
        let (tx, rx) = channel();
        let synth = Synth::new();
        let stream: Option<cpal::Stream> = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                build_stream::<f32>(device, config, channels, synth, rx).ok()
            }
            cpal::SampleFormat::I16 => {
                build_stream::<i16>(device, config, channels, synth, rx).ok()
            }
            cpal::SampleFormat::U16 => {
                build_stream::<u16>(device, config, channels, synth, rx).ok()
            }
            _ => return SfxOut { tx: None },
        };
        if let Some(stream) = &stream {
            use cpal::traits::StreamTrait;
            let _ = stream.play();
        }
        if stream.is_none() {
            return SfxOut { tx: None };
        }
        // Keep the stream alive for the process lifetime (we have no Drop).
        std::mem::forget(stream);
        SfxOut { tx: Some(tx) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::SfxOut;

#[cfg(not(target_arch = "wasm32"))]
pub fn init() -> SfxOut {
    native::init()
}

// ---- Web (WebAudio bridge) implementation ------------------------------------

#[cfg(target_arch = "wasm32")]
mod web {
    use super::Tone;

    // Scheduled by `web/gl.js`, which keeps a WebAudio oscillator queue.
    #[link(wasm_import_module = "env")]
    extern "C" {
        fn alien_audio_tone(freq: f32, secs: f32);
    }

    #[derive(Clone)]
    pub struct SfxOut;

    impl SfxOut {
        pub fn play(&self, tones: Vec<Tone>) {
            for t in tones {
                unsafe {
                    alien_audio_tone(t.freq, t.secs);
                }
            }
        }
    }

    pub fn init() -> SfxOut {
        SfxOut
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::SfxOut;

#[cfg(target_arch = "wasm32")]
pub fn init() -> SfxOut {
    web::init()
}
