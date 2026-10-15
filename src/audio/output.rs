use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError, bounded};

use crate::error::{Error, Result};

use super::spec::StreamSpec;

/// How long to keep the device open after the last sample is handed over,
/// so the tail of the song isn't cut off.
const TAIL: Duration = Duration::from_millis(300);

/// Something that can play interleaved f32 samples.
pub trait AudioOutput {
    /// Queue samples without blocking. If the buffer is full, the samples
    /// are handed back as `Some(samples)` so the caller can retry later.
    fn try_write(&mut self, samples: Vec<f32>) -> Result<Option<Vec<f32>>>;

    /// Signal that no more samples are coming.
    fn finish(&mut self);

    /// True once everything has been played after `finish()`.
    fn is_finished(&mut self) -> bool;

    fn set_paused(&mut self, paused: bool);

    /// How much audio has actually been played so far.
    fn position(&self) -> Duration;
}

/// Lives inside the audio callback: hands out samples one at a time
/// from the chunks the engine sends. Returns None for silence
/// (paused, buffer underrun, or end of stream).
struct SampleFeeder {
    receiver: Receiver<Vec<f32>>,
    pending: Vec<f32>,
    pos: usize,
    paused: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

impl SampleFeeder {
    fn next_sample(&mut self) -> Option<f32> {
        if self.paused.load(Ordering::Relaxed) {
            return None;
        }
        while self.pos >= self.pending.len() {
            match self.receiver.try_recv() {
                Ok(chunk) => {
                    self.pending = chunk;
                    self.pos = 0;
                }
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    self.finished.store(true, Ordering::Release);
                    return None;
                }
            }
        }
        let sample = self.pending[self.pos];
        self.pos += 1;
        Some(sample)
    }
}

pub struct CpalOutput {
    _stream: cpal::Stream, // kept alive; dropping it stops playback
    sender: Option<Sender<Vec<f32>>>,
    spec: StreamSpec,
    paused: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    samples_played: Arc<AtomicU64>,
    finished_at: Option<Instant>,
}

impl CpalOutput {
    pub fn open(spec: StreamSpec) -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| Error::Audio("no output device found".into()))?;

        let config = cpal::StreamConfig {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let (sender, receiver) = bounded::<Vec<f32>>(16);
        let paused = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let samples_played = Arc::new(AtomicU64::new(0));

        let mut feeder = SampleFeeder {
            receiver,
            pending: Vec::new(),
            pos: 0,
            paused: paused.clone(),
            finished: finished.clone(),
        };
        let played = samples_played.clone();

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut real = 0u64;
                    for out in data.iter_mut() {
                        match feeder.next_sample() {
                            Some(sample) => {
                                *out = sample;
                                real += 1;
                            }
                            None => *out = 0.0,
                        }
                    }
                    played.fetch_add(real, Ordering::Relaxed);
                },
                |err| eprintln!("audio stream error: {err}"),
                None,
            )
            .map_err(|e| Error::Audio(e.to_string()))?;

        stream.play().map_err(|e| Error::Audio(e.to_string()))?;

        Ok(Self {
            _stream: stream,
            sender: Some(sender),
            spec,
            paused,
            finished,
            samples_played,
            finished_at: None,
        })
    }
}

impl AudioOutput for CpalOutput {
    fn try_write(&mut self, samples: Vec<f32>) -> Result<Option<Vec<f32>>> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| Error::Audio("output already finished".into()))?;

        match sender.try_send(samples) {
            Ok(()) => Ok(None),
            Err(TrySendError::Full(samples)) => Ok(Some(samples)),
            Err(TrySendError::Disconnected(_)) => Err(Error::Audio("audio stream closed".into())),
        }
    }

    fn finish(&mut self) {
        self.sender = None; // disconnects the channel once it's empty
    }

    fn is_finished(&mut self) -> bool {
        if !self.finished.load(Ordering::Acquire) {
            return false;
        }
        self.finished_at.get_or_insert_with(Instant::now).elapsed() >= TAIL
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    fn position(&self) -> Duration {
        let samples = self.samples_played.load(Ordering::Relaxed);
        let per_second = u64::from(self.spec.sample_rate) * u64::from(self.spec.channels);
        Duration::from_secs_f64(samples as f64 / per_second as f64)
    }
}
