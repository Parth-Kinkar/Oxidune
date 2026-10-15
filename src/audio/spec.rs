/// The format of an audio stream: what the decoder produces
/// and what the output must be opened with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

use std::fmt;

impl fmt::Display for StreamSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:.1} kHz, {} ch",
            self.sample_rate as f64 / 1000.0,
            self.channels
        )
    }
}
