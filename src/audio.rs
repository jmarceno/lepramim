use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex as StdMutex};
use std::thread::{self, JoinHandle};

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
        hdr.extend_from_slice(&1u16.to_le_bytes());
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

enum AudioCmd {
    Warmup {
        sample_rate: u32,
        channels: u16,
        reply: mpsc::Sender<Result<(), String>>,
    },
    BeginStream {
        sample_rate: u32,
        channels: u16,
        reply: mpsc::Sender<Result<(), String>>,
    },
    Write {
        samples: Vec<f32>,
        sample_rate: u32,
        reply: mpsc::Sender<Result<(), String>>,
    },
    Stop {
        reply: mpsc::Sender<Result<(), String>>,
    },
    BufferStatus {
        reply: mpsc::Sender<Result<(usize, u32), String>>,
    },
    Shutdown,
}

struct PlaybackState {
    pending: Vec<f32>,
    read_pos: usize,
    stream_sample_rate: u32,
    device_sample_rate: u32,
}

/// Linear-resample mono PCM from `src_rate` to `dst_rate`.
pub(crate) fn resample_mono(samples: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == 0 || dst_rate == 0 || src_rate == dst_rate {
        return samples.to_vec();
    }
    let ratio = dst_rate as f64 / src_rate as f64;
    let out_len = (samples.len() as f64 * ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = samples.get(idx).copied().unwrap_or(0.0);
        let b = samples.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// Fill an interleaved device buffer from a mono source: one sample per frame,
/// duplicated across channels. Treating the buffer as a flat mono stream makes
/// stereo (and 24 kHz→48 kHz) play back about 2–4× too fast.
pub(crate) fn write_mono_frames<T: Copy>(
    data: &mut [T],
    channels: usize,
    mut sample: impl FnMut() -> T,
) {
    if channels == 0 {
        return;
    }
    for frame in data.chunks_mut(channels) {
        let s = sample();
        for ch in frame.iter_mut() {
            *ch = s;
        }
    }
}

impl PlaybackState {
    fn append_resampled(&mut self, samples: &[f32]) {
        let resampled = resample_mono(samples, self.stream_sample_rate, self.device_sample_rate);
        self.pending.extend_from_slice(&resampled);
    }

    fn pop_sample(&mut self) -> f32 {
        if self.read_pos < self.pending.len() {
            let s = self.pending[self.read_pos];
            self.read_pos += 1;
            if self.read_pos > 8192 && self.read_pos * 2 > self.pending.len().max(1) {
                let pos = self.read_pos;
                self.pending.drain(0..pos);
                self.read_pos = 0;
            }
            s
        } else {
            0.0
        }
    }

    fn remaining_samples(&self) -> usize {
        self.pending.len().saturating_sub(self.read_pos)
    }
}

fn choose_output_config(
    device: &cpal::Device,
    preferred_rate: u32,
) -> Result<(cpal::StreamConfig, cpal::SampleFormat, u16, u32), String> {
    use cpal::traits::DeviceTrait;
    let supported = device
        .default_output_config()
        .map_err(|e| format!("default_output_config failed: {e}"))?;
    let channels = supported.channels();
    let format = supported.sample_format();
    let mut rate = supported.sample_rate().0;
    if let Ok(cfgs) = device.supported_output_configs() {
        for range in cfgs {
            if range.channels() != channels || range.sample_format() != format {
                continue;
            }
            if range.min_sample_rate().0 <= preferred_rate
                && range.max_sample_rate().0 >= preferred_rate
            {
                rate = preferred_rate;
                break;
            }
        }
    }
    let config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(rate),
        buffer_size: cpal::BufferSize::Default,
    };
    Ok((config, format, channels, rate))
}

fn run_audio_thread(rx: mpsc::Receiver<AudioCmd>) {
    use cpal::SampleFormat;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let mut stream: Option<cpal::Stream> = None;
    let playback = Arc::new(StdMutex::new(PlaybackState {
        pending: Vec::new(),
        read_pos: 0,
        stream_sample_rate: 24_000,
        device_sample_rate: 48_000,
    }));
    let last_error: Arc<StdMutex<Option<String>>> = Arc::new(StdMutex::new(None));

    let open_stream = |sample_rate: u32,
                       playback: Arc<StdMutex<PlaybackState>>,
                       last_error: Arc<StdMutex<Option<String>>>|
     -> Result<cpal::Stream, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default audio output device".to_string())?;
        let (config, sample_format, channels, device_rate) =
            choose_output_config(&device, sample_rate)?;
        {
            let mut pb = playback.lock().map_err(|e| e.to_string())?;
            pb.device_sample_rate = device_rate;
            pb.stream_sample_rate = sample_rate;
            pb.pending.clear();
            pb.read_pos = 0;
        }
        *last_error.lock().map_err(|e| e.to_string())? = None;

        let channels = channels.max(1) as usize;
        let make_err_handler = || {
            let err_cb = last_error.clone();
            move |e| {
                if let Ok(mut err) = err_cb.lock() {
                    *err = Some(format!("CPAL stream error: {e}"));
                }
            }
        };

        let stream = match sample_format {
            SampleFormat::F32 => {
                let pb_cb = playback.clone();
                device
                    .build_output_stream(
                        &config,
                        move |data: &mut [f32], _| {
                            if let Ok(mut pb) = pb_cb.lock() {
                                write_mono_frames(data, channels, || pb.pop_sample());
                            } else {
                                data.fill(0.0);
                            }
                        },
                        make_err_handler(),
                        None,
                    )
                    .map_err(|e| format!("build_output_stream f32 failed: {e}"))?
            }
            SampleFormat::I16 => {
                let pb_cb = playback.clone();
                device
                    .build_output_stream(
                        &config,
                        move |data: &mut [i16], _| {
                            if let Ok(mut pb) = pb_cb.lock() {
                                write_mono_frames(data, channels, || {
                                    (pb.pop_sample().clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                                });
                            } else {
                                data.fill(0);
                            }
                        },
                        make_err_handler(),
                        None,
                    )
                    .map_err(|e| format!("build_output_stream i16 failed: {e}"))?
            }
            fmt => return Err(format!("unsupported CPAL sample format: {fmt:?}")),
        };
        stream
            .play()
            .map_err(|e| format!("stream.play failed: {e}"))?;
        tracing::info!(
            "CPAL output opened (tts_sr={sample_rate} device_sr={device_rate} ch={channels})"
        );
        Ok(stream)
    };

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AudioCmd::Warmup {
                sample_rate,
                channels: _,
                reply,
            } => {
                let res = (|| {
                    if stream.is_none() {
                        stream = Some(open_stream(
                            sample_rate,
                            playback.clone(),
                            last_error.clone(),
                        )?);
                    }
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            AudioCmd::BeginStream {
                sample_rate,
                channels: _,
                reply,
            } => {
                let res = (|| {
                    let need_reopen = stream.is_none() || {
                        let pb = playback.lock().map_err(|e| e.to_string())?;
                        pb.stream_sample_rate != sample_rate
                    };
                    if need_reopen {
                        stream = Some(open_stream(
                            sample_rate,
                            playback.clone(),
                            last_error.clone(),
                        )?);
                    } else if let Ok(mut pb) = playback.lock() {
                        pb.pending.clear();
                        pb.read_pos = 0;
                    }
                    Ok(())
                })();
                let _ = reply.send(res);
            }
            AudioCmd::Write {
                samples,
                sample_rate,
                reply,
            } => {
                let res = if let Ok(err) = last_error.lock() {
                    if let Some(ref e) = *err {
                        Err(e.clone())
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                };
                let res = res.and_then(|()| {
                    let mut pb = playback.lock().map_err(|e| e.to_string())?;
                    if pb.stream_sample_rate != sample_rate {
                        return Err(format!(
                            "chunk sr {sample_rate} != stream sr {}",
                            pb.stream_sample_rate
                        ));
                    }
                    pb.append_resampled(&samples);
                    Ok(())
                });
                let _ = reply.send(res);
            }
            AudioCmd::Stop { reply } => {
                // Keep the device stream open. Dropping it on every job forces a
                // PipeWire renegotiation and adds a large delay before speech.
                if let Ok(mut pb) = playback.lock() {
                    pb.pending.clear();
                    pb.read_pos = 0;
                }
                let _ = reply.send(Ok(()));
            }
            AudioCmd::BufferStatus { reply } => {
                let res = playback
                    .lock()
                    .map(|pb| (pb.remaining_samples(), pb.device_sample_rate.max(1)))
                    .map_err(|e| e.to_string());
                let _ = reply.send(res);
            }
            AudioCmd::Shutdown => break,
        }
    }
    drop(stream);
}

fn send_cmd(
    tx: &mpsc::Sender<AudioCmd>,
    build: impl FnOnce(mpsc::Sender<Result<(), String>>) -> AudioCmd,
) -> Result<(), String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(build(reply_tx))
        .map_err(|e| format!("audio thread gone: {e}"))?;
    reply_rx
        .recv()
        .map_err(|e| format!("audio thread reply failed: {e}"))?
}

fn send_cmd_status(tx: &mpsc::Sender<AudioCmd>) -> Result<(usize, u32), String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(AudioCmd::BufferStatus { reply: reply_tx })
        .map_err(|e| format!("audio thread gone: {e}"))?;
    reply_rx
        .recv()
        .map_err(|e| format!("audio thread reply failed: {e}"))?
}

/// CPAL-backed audio output on a dedicated thread (PipeWire/Pulse/ALSA).
pub struct CpalSink {
    tx: mpsc::Sender<AudioCmd>,
    _thread: JoinHandle<()>,
    stream_sample_rate: Option<u32>,
    stream_channels: Option<u16>,
}

impl CpalSink {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("lexaloud-audio".into())
            .spawn(move || run_audio_thread(rx))
            .expect("spawn audio thread");
        Self {
            tx,
            _thread: thread,
            stream_sample_rate: None,
            stream_channels: None,
        }
    }
}

impl Default for CpalSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CpalSink {
    fn drop(&mut self) {
        let _ = self.tx.send(AudioCmd::Shutdown);
    }
}

impl AudioSink for CpalSink {
    async fn warmup(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        tracing::info!("CpalSink warmup sr={} ch={}", sample_rate, channels);
        send_cmd(&self.tx, |reply| AudioCmd::Warmup {
            sample_rate,
            channels,
            reply,
        })?;
        self.stream_sample_rate = Some(sample_rate);
        self.stream_channels = Some(channels);
        Ok(())
    }

    async fn begin_stream(&mut self, sample_rate: u32, channels: u16) -> Result<(), String> {
        send_cmd(&self.tx, |reply| AudioCmd::BeginStream {
            sample_rate,
            channels,
            reply,
        })?;
        self.stream_sample_rate = Some(sample_rate);
        self.stream_channels = Some(channels);
        Ok(())
    }

    async fn write(&mut self, chunk: AudioChunk) -> Result<(), String> {
        let sr = self
            .stream_sample_rate
            .ok_or_else(|| "CpalSink write before begin_stream".to_string())?;
        if chunk.sample_rate != sr {
            return Err(format!(
                "CpalSink chunk sr {} != stream sr {}",
                chunk.sample_rate, sr
            ));
        }
        send_cmd(&self.tx, |reply| AudioCmd::Write {
            samples: chunk.samples,
            sample_rate: sr,
            reply,
        })
    }

    async fn end_stream(&mut self) -> Result<(), String> {
        // Writes only enqueue into the CPAL callback buffer. Stay in this call
        // until those samples have actually been played, so player state (and
        // the tray icon) do not go idle while speech is still audible.
        let (remaining, rate) = send_cmd_status(&self.tx)?;
        if remaining == 0 {
            return Ok(());
        }
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs_f64(remaining as f64 / f64::from(rate) + 3.0);
        let mut last = remaining;
        let mut stagnant = 0u32;
        while std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let (left, _) = send_cmd_status(&self.tx)?;
            if left == 0 {
                return Ok(());
            }
            if left >= last {
                stagnant += 1;
                if stagnant >= 40 {
                    break;
                }
            } else {
                stagnant = 0;
            }
            last = left;
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), String> {
        send_cmd(&self.tx, |reply| AudioCmd::Stop { reply })?;
        self.stream_sample_rate = None;
        self.stream_channels = None;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), String> {
        self.stop().await
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

    #[test]
    fn remaining_samples_tracks_unread_buffer() {
        let mut pb = PlaybackState {
            pending: vec![1.0, 2.0, 3.0],
            read_pos: 0,
            stream_sample_rate: 24_000,
            device_sample_rate: 24_000,
        };
        assert_eq!(pb.remaining_samples(), 3);
        let _ = pb.pop_sample();
        assert_eq!(pb.remaining_samples(), 2);
    }

    #[test]
    fn resample_mono_identity_when_rates_match() {
        let src = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_mono(&src, 24_000, 24_000), src);
    }

    #[test]
    fn resample_mono_doubles_length_24k_to_48k() {
        let src = vec![0.0, 1.0];
        let out = resample_mono(&src, 24_000, 48_000);
        assert_eq!(out.len(), 4);
        assert!((out[0] - 0.0).abs() < 1e-5);
        assert!((out[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn write_mono_frames_duplicates_sample_per_stereo_frame() {
        let mut buf = [0.0f32; 8];
        let src = [1.0, 2.0, 3.0, 4.0];
        let mut i = 0usize;
        write_mono_frames(&mut buf, 2, || {
            let s = src[i];
            i += 1;
            s
        });
        assert_eq!(buf, [1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0]);
        assert_eq!(i, 4);
    }

    #[test]
    fn write_mono_frames_mono_is_one_to_one() {
        let mut buf = [0.0f32; 3];
        let src = [0.5, -0.5, 0.25];
        let mut i = 0usize;
        write_mono_frames(&mut buf, 1, || {
            let s = src[i];
            i += 1;
            s
        });
        assert_eq!(buf, src);
    }

    #[tokio::test]
    async fn cpal_sink_opens_or_skips_without_device() {
        let mut sink = CpalSink::new();
        match sink.warmup(24000, 1).await {
            Ok(()) => {
                sink.begin_stream(24000, 1).await.unwrap();
                let chunk = AudioChunk::new(vec![0.0; 100], 24000);
                sink.write(chunk).await.unwrap();
                sink.stop().await.unwrap();
                sink.close().await.unwrap();
            }
            Err(e) => {
                assert!(e.contains("device") || e.contains("CPAL"));
            }
        }
    }
}
