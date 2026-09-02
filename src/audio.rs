use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One synthesized sentence (or silence pad).
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub metadata: HashMap<String, String>,
}

impl AudioChunk {
    pub fn new(samples: Vec<f32>, sample_rate: u32) -> Self {
        Self {
            samples,
            sample_rate,
            channels: 1,
            metadata: HashMap::new(),
        }
    }
    pub fn num_samples(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }
    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.num_samples() as f64 / self.sample_rate as f64
        }
    }
}

/// Audio sink trait.
/// Uses async fn in trait (AFIT) - Rust 1.85 supports this. Not dyn-safe, use generics.
pub trait AudioSink: Send + Sync {
    fn warmup(
        &mut self,
        sample_rate: u32,
        channels: u16,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send;
    fn begin_stream(
        &mut self,
        sample_rate: u32,
        channels: u16,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send;
    fn write(
        &mut self,
        chunk: AudioChunk,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send;
    fn end_stream(&mut self) -> impl std::future::Future<Output = Result<(), String>> + Send;
    fn stop(&mut self) -> impl std::future::Future<Output = Result<(), String>> + Send;
    fn close(&mut self) -> impl std::future::Future<Output = Result<(), String>> + Send;
}

/// NullSink: discards samples, records counters.
#[derive(Debug, Default)]
pub struct NullSink {
    pub begin_calls: Vec<(u32, u16)>,
    pub write_count: usize,
    pub samples_received: usize,
    pub end_calls: usize,
    pub stop_calls: usize,
    pub close_calls: usize,
    stream_sample_rate: Option<u32>,
}

impl NullSink {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AudioSink for NullSink {
    async fn warmup(&mut self, _sample_rate: u32, _channels: u16) -> Result<(), String> {
        Ok(())
    }
    async fn begin_stream(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        self.begin_calls.push((sample_rate, channels));
        self.stream_sample_rate = Some(sample_rate);
        Ok(())
    }
    async fn write(&mut self, chunk: AudioChunk) -> Result<(), String> {
        if let Some(sr) = self.stream_sample_rate {
            if chunk.sample_rate != sr {
                return Err(format!(
                    "chunk sample_rate={} does not match stream sample_rate={}",
                    chunk.sample_rate, sr
                ));
            }
        }
        self.write_count += 1;
        self.samples_received += chunk.num_samples();
        Ok(())
    }
    async fn end_stream(&mut self) -> Result<(), String> {
        self.end_calls += 1;
        self.stream_sample_rate = None;
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), String> {
        self.stop_calls += 1;
        self.stream_sample_rate = None;
        Ok(())
    }
    async fn close(&mut self) -> Result<(), String> {
        self.close_calls += 1;
        Ok(())
    }
}

/// WavSink: writes one WAV file per stream.
pub struct WavSink {
    out_dir: PathBuf,
    current_path: Option<PathBuf>,
    current_file: Option<std::fs::File>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    stream_index: usize,
    pub written_files: Vec<PathBuf>,
    pcm_bytes: Vec<u8>,
}

impl WavSink {
    pub fn new(out_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let out_dir = out_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&out_dir)?;
        Ok(Self {
            out_dir,
            current_path: None,
            current_file: None,
            sample_rate: None,
            channels: None,
            stream_index: 0,
            written_files: Vec::new(),
            pcm_bytes: Vec::new(),
        })
    }

    fn write_wav_header(&self, sample_rate: u32, channels: u16, data_len: u32) -> Vec<u8> {
        let mut hdr = Vec::with_capacity(44);
        let byte_rate = sample_rate * channels as u32 * 2;
        let block_align = channels * 2;
        hdr.extend_from_slice(b"RIFF");
        hdr.extend_from_slice(&(36 + data_len).to_le_bytes());
        hdr.extend_from_slice(b"WAVE");
        hdr.extend_from_slice(b"fmt ");
        hdr.extend_from_slice(&16u32.to_le_bytes());
        hdr.extend_from_slice(&1u16.to_le_bytes()); // PCM
        hdr.extend_from_slice(&channels.to_le_bytes());
        hdr.extend_from_slice(&sample_rate.to_le_bytes());
        hdr.extend_from_slice(&byte_rate.to_le_bytes());
        hdr.extend_from_slice(&block_align.to_le_bytes());
        hdr.extend_from_slice(&16u16.to_le_bytes());
        hdr.extend_from_slice(b"data");
        hdr.extend_from_slice(&data_len.to_le_bytes());
        hdr
    }
}

impl AudioSink for WavSink {
    async fn warmup(&mut self, _sample_rate: u32, _channels: u16) -> Result<(), String> {
        Ok(())
    }
    async fn begin_stream(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        // Flush previous if any
        if self.current_path.is_some() {
            let _ = self.end_stream().await;
        }
        self.stream_index += 1;
        let path = self
            .out_dir
            .join(format!("stream_{:04}.wav", self.stream_index));
        self.current_path = Some(path);
        self.sample_rate = Some(sample_rate);
        self.channels = Some(channels);
        self.pcm_bytes.clear();
        Ok(())
    }
    async fn write(&mut self, chunk: AudioChunk) -> Result<(), String> {
        let sr = self
            .sample_rate
            .ok_or("WavSink.write called before begin_stream")?;
        if chunk.sample_rate != sr {
            return Err(format!(
                "chunk sample_rate={} != stream sample_rate={}",
                chunk.sample_rate, sr
            ));
        }
        // Clamp and convert to i16
        for &s in &chunk.samples {
            let clamped = s.clamp(-1.0, 1.0);
            let pcm = (clamped * 32767.0) as i16;
            self.pcm_bytes.extend_from_slice(&pcm.to_le_bytes());
        }
        Ok(())
    }
    async fn end_stream(&mut self) -> Result<(), String> {
        if let Some(path) = self.current_path.take() {
            let sr = self.sample_rate.unwrap_or(24000);
            let ch = self.channels.unwrap_or(1);
            let data_len = self.pcm_bytes.len() as u32;
            let header = self.write_wav_header(sr, ch, data_len);
            let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
            use std::io::Write;
            file.write_all(&header).map_err(|e| e.to_string())?;
            file.write_all(&self.pcm_bytes).map_err(|e| e.to_string())?;
            self.written_files.push(path);
            self.pcm_bytes.clear();
            self.sample_rate = None;
            self.channels = None;
        }
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), String> {
        self.end_stream().await
    }
    async fn close(&mut self) -> Result<(), String> {
        self.stop().await
    }
}

/// CpalSink stub: logs and fails gracefully if no device.
#[derive(Debug, Default)]
pub struct CpalSink {
    stream_sample_rate: Option<u32>,
    stream_channels: Option<u16>,
}

impl CpalSink {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AudioSink for CpalSink {
    async fn warmup(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        tracing::info!("CpalSink warmup sr={} ch={} (stub)", sample_rate, channels);
        // Try to open device would go here; stub succeeds
        self.stream_sample_rate = Some(sample_rate);
        self.stream_channels = Some(channels);
        Ok(())
    }
    async fn begin_stream(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        tracing::info!(
            "CpalSink begin_stream sr={} ch={} (stub)",
            sample_rate,
            channels
        );
        if let (Some(sr), Some(ch)) = (self.stream_sample_rate, self.stream_channels) {
            if sr == sample_rate && ch == channels {
                return Ok(());
            }
        }
        self.stream_sample_rate = Some(sample_rate);
        self.stream_channels = Some(channels);
        Ok(())
    }
    async fn write(&mut self, chunk: AudioChunk) -> Result<(), String> {
        if let Some(sr) = self.stream_sample_rate {
            if chunk.sample_rate != sr {
                return Err(format!(
                    "CpalSink chunk sr {} != stream sr {}",
                    chunk.sample_rate, sr
                ));
            }
        } else {
            return Err("CpalSink write before begin_stream".to_string());
        }
        // Stub: discard
        tracing::trace!("CpalSink write {} samples", chunk.num_samples());
        Ok(())
    }
    async fn end_stream(&mut self) -> Result<(), String> {
        // Keep stream open like audio-backendSink
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), String> {
        tracing::info!("CpalSink stop (stub)");
        Ok(())
    }
    async fn close(&mut self) -> Result<(), String> {
        self.stream_sample_rate = None;
        self.stream_channels = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn null_sink_counts() {
        let mut sink = NullSink::new();
        sink.begin_stream(24000, 1).await.unwrap();
        let chunk = AudioChunk::new(vec![0.0; 2400], 24000);
        sink.write(chunk).await.unwrap();
        assert_eq!(sink.write_count, 1);
        assert_eq!(sink.samples_received, 2400);
        sink.end_stream().await.unwrap();
        assert_eq!(sink.end_calls, 1);
    }

    #[tokio::test]
    async fn null_sink_mismatched_sample_rate_fails() {
        let mut sink = NullSink::new();
        sink.begin_stream(24000, 1).await.unwrap();
        let chunk = AudioChunk::new(vec![0.0; 100], 48000);
        let res = sink.write(chunk).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn wav_sink_writes_file() {
        let dir = std::env::temp_dir().join(format!("lexaloud_wav_test_{}", std::process::id()));
        let mut sink = WavSink::new(&dir).unwrap();
        sink.begin_stream(24000, 1).await.unwrap();
        let chunk = AudioChunk::new(vec![0.5; 2400], 24000);
        sink.write(chunk).await.unwrap();
        sink.end_stream().await.unwrap();
        assert_eq!(sink.written_files.len(), 1);
        assert!(sink.written_files[0].exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cpal_sink_stub() {
        let mut sink = CpalSink::new();
        sink.warmup(24000, 1).await.unwrap();
        sink.begin_stream(24000, 1).await.unwrap();
        let chunk = AudioChunk::new(vec![0.0; 100], 24000);
        sink.write(chunk).await.unwrap();
        sink.stop().await.unwrap();
        sink.close().await.unwrap();
    }
}
