use super::{assembly::Frame, Shared};
use crate::audio::{
    capture::AudioCaptureBackend,
    devices::{default_input_device, AudioDevice, DeviceType as DeviceKind},
    recording_state::{DeviceType, RecordingState},
    stream::AudioStream,
};
use std::{
    collections::VecDeque,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tauri::Emitter;
use tokio::sync::mpsc;

pub const RATE: u32 = 48_000;
const FRAME: usize = 4_800;
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
pub async fn run(
    shared: Arc<Shared>,
    microphone: Option<String>,
    senders: [mpsc::Sender<Frame>; 2],
) -> Result<(), String> {
    let state = RecordingState::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.set_audio_sender(tx);
    let mic = match microphone.filter(|x| !x.is_empty()) {
        Some(n) => AudioDevice::new(n, DeviceKind::Input),
        None => default_input_device().map_err(|_| "No microphone is available.".to_string())?,
    };
    let mic_stream =
        AudioStream::create(Arc::new(mic), state.clone(), DeviceType::Microphone, None)
            .await
            .map_err(|e| format!("Could not start your microphone: {e}"))?;
    let sys = AudioDevice::new("Mac audio".into(), DeviceKind::Output);
    let sys_stream = match AudioStream::create_with_backend(
        Arc::new(sys),
        state.clone(),
        DeviceType::System,
        None,
        AudioCaptureBackend::CoreAudio,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            state.stop_recording();
            let _ = mic_stream.stop();
            return Err(format!("Could not capture Mac audio. Enable PartyLine's Audio Capture permission, then retry. {e}"));
        }
    };
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writers = Vec::new();
    for name in ["microphone.wav", "mac-audio.wav"] {
        match hound::WavWriter::create(shared.folder.join(name), spec) {
            Ok(w) => writers.push(w),
            Err(_) => {
                state.stop_recording();
                let _ = mic_stream.stop();
                let _ = sys_stream.stop();
                return Err(
                    "Could not create the local audio files. Check available disk space.".into(),
                );
            }
        }
    }
    state.start_recording().map_err(|e| e.to_string())?;
    let mut queues = [VecDeque::new(), VecDeque::new()];
    let mut received = [false; 2];
    let mut warned = [false; 2];
    let mut tick = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_millis(250),
        Duration::from_millis(100),
    );
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut frames = 0_u64;
    let mut failure = None;
    shared.meeting.lock().await.status = "recording".into();
    shared.publish(true).await;
    crate::tray::update_tray_menu(&shared.app);
    loop {
        tokio::select! {
            _=shared.stop.cancelled()=>break,
            chunk=rx.recv()=>match chunk {
                Some(chunk)=>{
                    let i=if chunk.device_type==DeviceType::Microphone{0}else{1}; received[i]=true;
                    if chunk.sample_rate!=RATE {failure=Some("Unexpected audio sample rate. Recording stopped to avoid distorted audio.".to_string());break;}
                    if !shared.paused.load(Ordering::SeqCst){queues[i].extend(chunk.data);}
                    if queues[i].len()>RATE as usize*3 {failure=Some("Audio processing fell behind. Recording stopped; captured audio is saved.".into());break;}
                },None=>break
            },
            _=tick.tick()=>{
                if !state.is_recording() || state.get_last_error().is_some() {
                    failure=Some("An audio device stopped responding. Captured audio is saved; check your devices and start a new meeting.".into());break;
                }
                let paused=shared.paused.load(Ordering::SeqCst);
                let mut levels=[0.0_f32;2];
                for i in 0..2 {
                    if paused {queues[i].clear();}
                    let samples:Vec<f32>=(0..FRAME).map(|_|queues[i].pop_front().unwrap_or(0.0)).collect();
                    levels[i]=(samples.iter().map(|x|x*x).sum::<f32>()/FRAME as f32).sqrt();
                    let bytes=pcm(&samples);
                    for pair in bytes.chunks_exact(2) {if writers[i].write_sample(i16::from_le_bytes([pair[0],pair[1]])).is_err(){failure=Some("Could not save audio. Check available disk space.".into());break;}}
                    if senders[i].try_send(Frame{pcm:bytes,start:frames as f64/10.0}).is_err() && !warned[i] {
                        warned[i]=true;
                        shared.warning("Live transcription fell behind or disconnected. Audio is saved locally; the transcript may have gaps.".into()).await;
                    }
                }
                frames+=1;
                if frames%5==0 {shared.meeting.lock().await.duration=frames as f64/10.0;let _=shared.app.emit("cloud-levels",serde_json::json!({"id":shared.id,"me":levels[0],"remote":levels[1],"duration":frames as f64/10.0}));}
                if frames%20==0 {for w in &mut writers {if w.flush().is_err(){failure=Some("Could not flush the local recording to disk.".into());}}}
                if frames==100 && !received[0] {shared.warning("No microphone audio is arriving. Check your microphone permission and device.".into()).await;}
                if frames==100 && !received[1] {shared.warning("No Mac audio is arriving. Check PartyLine's Audio Capture permission.".into()).await;}
                if failure.is_some(){break;}
            }
        }
    }
    state.stop_recording();
    let _ = mic_stream.stop();
    let _ = sys_stream.stop();
    // Preserve the last callback's samples, including the short tail at Stop.
    while let Ok(chunk) = rx.try_recv() {
        if !shared.paused.load(Ordering::SeqCst) {
            queues[if chunk.device_type == DeviceType::Microphone {
                0
            } else {
                1
            }]
            .extend(chunk.data);
        }
    }
    while queues.iter().any(|q| !q.is_empty()) {
        for i in 0..2 {
            let samples: Vec<f32> = (0..FRAME)
                .map(|_| queues[i].pop_front().unwrap_or(0.0))
                .collect();
            let bytes = pcm(&samples);
            for pair in bytes.chunks_exact(2) {
                if writers[i]
                    .write_sample(i16::from_le_bytes([pair[0], pair[1]]))
                    .is_err()
                {
                    failure = Some(
                        "Could not save the last audio samples. Check available disk space.".into(),
                    );
                }
            }
            let _ = senders[i].try_send(Frame {
                pcm: bytes,
                start: frames as f64 / 10.0,
            });
        }
        frames += 1;
    }
    for w in writers {
        if w.finalize().is_err() {
            failure = Some(
                "Could not finish the audio file. The captured data may need recovery.".into(),
            );
        }
    }
    {
        let mut m = shared.meeting.lock().await;
        m.duration = frames as f64 / 10.0;
        m.status = "finishing".into();
    }
    shared.publish(true).await;
    drop(senders);
    if let Some(e) = failure {
        Err(e)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_is_little_endian_clipped_and_sanitized() {
        assert_eq!(
            pcm(&[1.0, -1.0, 2.0, f32::NAN, 0.0]),
            vec![255, 127, 1, 128, 255, 127, 0, 0, 0, 0]
        );
    }
}
