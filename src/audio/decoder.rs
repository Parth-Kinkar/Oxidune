use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder as CodecDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use std::time::Duration;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::units::Time;

use crate::error::{Error, Result};

use super::spec::StreamSpec;

/// Turns an audio source into interleaved f32 samples, chunk by chunk.
/// f32 holds up to 24-bit audio exactly, so FLAC/ALAC lose nothing here.
pub trait Decoder {
    fn spec(&self) -> StreamSpec;

    /// Next chunk of interleaved samples, or None at end of stream.
    fn next_chunk(&mut self) -> Result<Option<Vec<f32>>>;

    /// Jump to a position in the track.
    fn seek(&mut self, to: Duration) -> Result<()>;
}

pub struct SymphoniaDecoder {
    format: Box<dyn FormatReader>,
    codec: Box<dyn CodecDecoder>,
    track_id: u32,
    spec: StreamSpec,
    /// Decoded during open() to learn the real format; returned first.
    first_chunk: Option<Vec<f32>>,
}

impl SymphoniaDecoder {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe().format(
            &hint,
            stream,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )?;
        let format = probed.format;

        let (track_id, params) = {
            let track = format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .ok_or_else(|| Error::Decode("no audio track found".into()))?;
            (track.id, track.codec_params.clone())
        };

        let codec = symphonia::default::get_codecs().make(&params, &DecoderOptions::default())?;

        let mut decoder = Self {
            format,
            codec,
            track_id,
            spec: StreamSpec {
                sample_rate: 0,
                channels: 0,
            },
            first_chunk: None,
        };

        // The container doesn't always know the channel layout up front,
        // so take the real format from the first decoded buffer.
        let (chunk, spec) = decoder
            .decode_next()?
            .ok_or_else(|| Error::Decode("file contains no audio".into()))?;
        decoder.spec = spec;
        decoder.first_chunk = Some(chunk);

        Ok(decoder)
    }

    fn decode_next(&mut self) -> Result<Option<(Vec<f32>, StreamSpec)>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.codec.decode(&packet) {
                Ok(decoded) => {
                    let audio_spec = *decoded.spec();
                    let spec = StreamSpec {
                        sample_rate: audio_spec.rate,
                        channels: audio_spec.channels.count() as u16,
                    };
                    let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, audio_spec);
                    buf.copy_interleaved_ref(decoded);
                    return Ok(Some((buf.samples().to_vec(), spec)));
                }
                // A corrupt packet: skip it and keep going.
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}

impl Decoder for SymphoniaDecoder {
    fn spec(&self) -> StreamSpec {
        self.spec
    }

    fn next_chunk(&mut self) -> Result<Option<Vec<f32>>> {
        if let Some(chunk) = self.first_chunk.take() {
            return Ok(Some(chunk));
        }
        Ok(self.decode_next()?.map(|(chunk, _)| chunk))
    }

    fn seek(&mut self, to: Duration) -> Result<()> {
        let time = Time::new(to.as_secs(), f64::from(to.subsec_nanos()) / 1e9);
        self.format.seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;
        self.codec.reset();
        self.first_chunk = None;
        Ok(())
    }
}
