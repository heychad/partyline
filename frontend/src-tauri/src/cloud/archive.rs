//! The local recorder has no network, UI, or meeting-state dependencies.
//! Samples are written on arrival. Only the disposable live-transcription copy
//! can be dropped when its bounded queue is full.
use std::{
    fs::File,
    io::{self, BufWriter},
    path::Path,
};

pub const RATE: u32 = 48_000;
pub const FRAME: usize = 4_800;

#[derive(Debug)]
pub struct Frame {
    pub pcm: Vec<u8>,
    pub start: f64,
}

pub fn pcm(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|v| {
            let v = if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            ((v * i16::MAX as f32).round() as i16).to_le_bytes()
        })
        .collect()
}

pub struct Track {
    writer: hound::WavWriter<BufWriter<File>>,
    sync: File,
    pub samples: u64,
    pending: Vec<u8>,
    pub skipped_samples: u64,
    pub peak: f32,
    pub level: f32,
}
impl Track {
    pub fn create(path: &Path) -> Result<Self, String> {
        let file =
            File::create(path).map_err(|_| "Could not create local audio. Check disk space.")?;
        let sync = file
            .try_clone()
            .map_err(|_| "Could not open local audio.")?;
        let writer = hound::WavWriter::new(
            BufWriter::with_capacity(256 * 1024, file),
            hound::WavSpec {
                channels: 1,
                sample_rate: RATE,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|_| "Could not start the local audio file.")?;
        Ok(Self {
            writer,
            sync,
            samples: 0,
            pending: Vec::with_capacity(FRAME * 2),
            skipped_samples: 0,
            peak: 0.0,
            level: 0.0,
        })
    }
    pub fn duration(&self) -> f64 {
        self.samples as f64 / RATE as f64
    }
    pub fn push(
        &mut self,
        samples: &[f32],
        mut live: impl FnMut(Frame) -> bool,
    ) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }
        self.level = (samples
            .iter()
            .map(|x| if x.is_finite() { x * x } else { 0.0 })
            .sum::<f32>()
            / samples.len() as f32)
            .sqrt();
        self.peak = self.peak.max(self.level);
        for bytes in pcm(samples).chunks_exact(2) {
            self.writer
                .write_sample(i16::from_le_bytes([bytes[0], bytes[1]]))
                .map_err(|_| "Could not save audio. Check available disk space.")?;
            self.samples += 1;
            self.pending.extend_from_slice(bytes);
            if self.pending.len() == FRAME * 2 {
                self.send(&mut live);
            }
        }
        Ok(())
    }
    fn send(&mut self, live: &mut impl FnMut(Frame) -> bool) {
        let count = self.pending.len() / 2;
        let frame = Frame {
            pcm: std::mem::replace(&mut self.pending, Vec::with_capacity(FRAME * 2)),
            start: (self.samples - count as u64) as f64 / RATE as f64,
        };
        if !live(frame) {
            self.skipped_samples += count as u64;
        }
    }
    // Fill genuine callback gaps, including a missing device, on the same clock.
    // Scheduling delays never cap the amount of audio we can drain afterward.
    pub fn silence_until(
        &mut self,
        seconds: f64,
        mut live: impl FnMut(Frame) -> bool,
    ) -> Result<(), String> {
        let target = (seconds.max(0.0) * RATE as f64).round() as u64;
        let zeros = [0.0; FRAME];
        while self.samples < target {
            let n = (target - self.samples).min(FRAME as u64) as usize;
            self.push(&zeros[..n], &mut live)?;
        }
        Ok(())
    }
    pub fn checkpoint(&mut self) -> Result<(), String> {
        self.writer
            .flush()
            .map_err(|_| "Could not checkpoint local audio. Check disk space.")?;
        self.sync
            .sync_data()
            .map_err(|_| "Could not save the audio checkpoint to disk.".to_string())
    }
    pub fn finish(mut self, mut live: impl FnMut(Frame) -> bool) -> Result<(), String> {
        if !self.pending.is_empty() {
            self.send(&mut live);
        }
        self.writer
            .finalize()
            .map_err(|_| "Could not finish the audio header. Saved audio may need recovery.")?;
        self.sync
            .sync_all()
            .map_err(|_| "Could not finish saving local audio.".to_string())
    }
}

/// Repair only PartyLine's plain PCM WAV header after an unclean exit. Preserve
/// the original first. This recovers flushed bytes beyond the last checkpoint.
pub fn recover_wav(path: &Path) -> Result<f64, String> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| "Audio file not found.")?;
    let mut header = [0u8; 44];
    file.read_exact(&mut header)
        .map_err(|_| "Incomplete audio header.")?;
    if &header[0..4] != b"RIFF"
        || &header[8..16] != b"WAVEfmt "
        || &header[36..40] != b"data"
        || u16::from_le_bytes([header[20], header[21]]) != 1
        || u16::from_le_bytes([header[22], header[23]]) != 1
        || u32::from_le_bytes(header[24..28].try_into().unwrap()) != RATE
        || u16::from_le_bytes([header[34], header[35]]) != 16
    {
        return Err("Unsupported audio header; the original file was preserved.".into());
    }
    let size = file
        .metadata()
        .map_err(|_| "Could not inspect audio.")?
        .len();
    let data = size.saturating_sub(44) / 2 * 2;
    if data > u32::MAX as u64 - 36 {
        return Err("Audio requires a large-file recovery tool.".into());
    }
    let old = u32::from_le_bytes(header[40..44].try_into().unwrap()) as u64;
    if old != data {
        let backup = path.with_extension("wav.before-recovery");
        if !backup.exists() {
            std::fs::copy(path, backup)
                .map_err(|_| "Could not preserve the original audio header.")?;
        }
        file.seek(SeekFrom::Start(4))
            .and_then(|_| file.write_all(&((data + 36) as u32).to_le_bytes()))
            .and_then(|_| file.seek(SeekFrom::Start(40)))
            .and_then(|_| file.write_all(&(data as u32).to_le_bytes()))
            .and_then(|_| file.sync_all())
            .map_err(|_: io::Error| "Could not recover the audio header.")?;
    }
    Ok(data as f64 / (RATE as f64 * 2.0))
}
