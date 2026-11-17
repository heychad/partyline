use super::{
    archive::{Frame, Track, RATE},
    library::model::CaptureHealth,
    Shared,
};
use crate::audio::{
    capture::AudioCaptureBackend,
    devices::{default_input_device, AudioDevice, DeviceType as DeviceKind},
    recording_state::{DeviceType, RecordingState},
    stream::AudioStream,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::Emitter;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

// This worker owns audio devices and disk writes on a separate runtime. It never
// waits for the UI, meeting JSON, notes, Keychain, or a transcription socket.
async fn record(
    folder: PathBuf,
    microphone: Option<String>,
    senders: [mpsc::Sender<Frame>; 2],
    stop: CancellationToken,
    paused: Arc<AtomicBool>,
    health: watch::Sender<CaptureHealth>,
) -> Result<(), String> {
    let state = RecordingState::new();
    let (tx, mut rx) = mpsc::unbounded_channel();
    state.set_audio_sender(tx);
    let mic = match microphone.filter(|s| !s.is_empty()) {
        Some(n) => AudioDevice::new(n, DeviceKind::Input),
        None => default_input_device().map_err(|_| "No microphone is available.")?,
    };
    let mic_stream =
        AudioStream::create(Arc::new(mic), state.clone(), DeviceType::Microphone, None)
            .await
            .map_err(|e| format!("Could not start your microphone: {e}"))?;
    let sys_stream = match AudioStream::create_with_backend(
        Arc::new(AudioDevice::new("Mac audio".into(), DeviceKind::Output)),
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
            return Err(format!(
                "Could not capture Mac audio. Check PartyLine's Audio Capture permission. {e}"
            ));
        }
    };
    let mut streams = [Some(mic_stream), Some(sys_stream)];
    let result=async {
        let mut tracks=[Track::create(&folder.join("microphone.wav"))?,Track::create(&folder.join("mac-audio.wav"))?];
        state.start_recording().map_err(|e|e.to_string())?;
        let start=Instant::now();let mut last=[Instant::now();2];let mut received=[false;2];let mut checkpoint=Instant::now();let mut durable=0.0;
        let mut tick=tokio::time::interval(Duration::from_millis(100));tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut failure=None;
        loop {
            tokio::select! {
                _=stop.cancelled()=>break,
                chunk=rx.recv()=>match chunk {
                    Some(chunk)=>{
                        let i=if chunk.device_type==DeviceType::Microphone{0}else{1};received[i]=true;last[i]=Instant::now();
                        if chunk.sample_rate!=RATE {failure=Some("Unexpected audio sample rate. Check the input device.".into());break;}
                        // Arrival timestamps share a monotonic clock. Pad a missing callback
                        // interval; preserve every received sample even after long stalls.
                        let beginning=(chunk.timestamp-chunk.data.len() as f64/RATE as f64).max(0.0);
                        let write=(|| {
                            if beginning-tracks[i].duration()>0.25 {tracks[i].silence_until(beginning,|f|senders[i].try_send(f).is_ok())?;}
                            if paused.load(Ordering::SeqCst) {let end=tracks[i].duration()+chunk.data.len() as f64/RATE as f64;tracks[i].silence_until(end,|f|senders[i].try_send(f).is_ok())}
                            else {tracks[i].push(&chunk.data,|f|senders[i].try_send(f).is_ok())}
                        })();
                        if let Err(e)=write {failure=Some(e);break;}
                    },None=>{failure=Some("The audio input channel closed. Captured audio is saved.".into());break;}
                },
                _=tick.tick()=>{
                    if !state.is_recording() || state.has_fatal_error() {failure=Some("An audio device stopped responding. Captured audio is saved; check your devices.".into());break;}
                    for i in 0..2 {
                        if last[i].elapsed()>Duration::from_millis(500) {
                            if let Err(e)=tracks[i].silence_until(start.elapsed().as_secs_f64()-0.25,|f|senders[i].try_send(f).is_ok()){failure=Some(e);break;}
                        }
                    }
                    if checkpoint.elapsed()>=Duration::from_secs(2) {
                        for t in &mut tracks {if let Err(e)=t.checkpoint(){failure=Some(e);break;}}
                        durable=tracks.iter().map(Track::duration).fold(f64::INFINITY,f64::min);checkpoint=Instant::now();
                    }
                    health.send_replace(CaptureHealth { recording:true,durable_seconds:durable,seconds:[tracks[0].duration(),tracks[1].duration()],live_skipped_seconds:[tracks[0].skipped_samples as f64/RATE as f64,tracks[1].skipped_samples as f64/RATE as f64],levels:[tracks[0].level,tracks[1].level],received,silent:[tracks[0].peak<0.00001,tracks[1].peak<0.00001],updated_at:chrono::Utc::now().to_rfc3339() });
                    if failure.is_some(){break;}
                }
            }
        }
        state.stop_recording();for stream in &mut streams {if let Some(s)=stream.take(){let _=s.stop();}}
        // Drain already received callbacks before closing the files. Paused data
        // is silence, including the tail, so privacy semantics remain unchanged.
        while let Ok(chunk)=rx.try_recv(){let i=if chunk.device_type==DeviceType::Microphone{0}else{1};if chunk.sample_rate==RATE {
            let samples=if paused.load(Ordering::SeqCst){vec![0.0;chunk.data.len()]}else{chunk.data};
            if let Err(e)=tracks[i].push(&samples,|f|senders[i].try_send(f).is_ok()){failure=Some(e);break;}
        }}
        let end=tracks.iter().map(Track::duration).fold(start.elapsed().as_secs_f64(),f64::max);
        for i in 0..2 {if let Err(e)=tracks[i].silence_until(end,|f|senders[i].try_send(f).is_ok()){failure=Some(e);}}
        let mut report=health.borrow().clone(); report.recording=false; report.seconds=[tracks[0].duration(),tracks[1].duration()];report.live_skipped_seconds=[tracks[0].skipped_samples as f64/RATE as f64,tracks[1].skipped_samples as f64/RATE as f64];
        for (i,t) in tracks.into_iter().enumerate(){if let Err(e)=t.finish(|f|senders[i].try_send(f).is_ok()){failure=Some(e);}}
        if failure.is_none(){report.durable_seconds=end;}report.updated_at=chrono::Utc::now().to_rfc3339();health.send_replace(report);
        failure.map_or(Ok(()),Err)
    }.await;
    state.stop_recording();
    for stream in &mut streams {
        if let Some(s) = stream.take() {
            let _ = s.stop();
        }
    }
    result
}

pub async fn run(
    shared: Arc<Shared>,
    microphone: Option<String>,
    senders: [mpsc::Sender<Frame>; 2],
) -> Result<(), String> {
    let (tx, mut rx) = watch::channel(CaptureHealth::default());
    let folder = shared.folder.clone();
    let stop = shared.stop.clone();
    let paused = shared.paused.clone();
    let worker = tokio::task::spawn_blocking(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Could not start the audio recorder.".to_string())?
            .block_on(record(folder, microphone, senders, stop, paused, tx))
    });
    let mut published = Instant::now() - Duration::from_secs(3);
    let mut started = false;
    while rx.changed().await.is_ok() {
        let h = rx.borrow_and_update().clone();
        {
            let mut m = shared.meeting.lock().await;
            m.duration = h.seconds[0].max(h.seconds[1]);
            m.capture = h.clone();
            if !started && h.recording {
                if m.status == "connecting" {
                    m.status = "recording".into();
                }
                started = true;
            }
            if h.live_skipped_seconds.iter().any(|s| *s > 0.0) {
                m.recovery_needed = true;
                let warning="Live transcription has gaps. Local recording continues; use Recover transcript after stopping.".to_string();
                if !m.errors.contains(&warning) {
                    m.errors.push(warning);
                }
            }
            if m.duration > 10.0 {
                for (i, label) in ["Microphone", "Mac audio"].iter().enumerate() {
                    if !h.received[i] || h.silent[i] {
                        let warning = format!(
                            "{label} has no audible signal. Check the device and audio meter."
                        );
                        if !m.errors.contains(&warning) {
                            m.errors.push(warning);
                        }
                    }
                }
            }
        }
        let _=shared.app.emit("cloud-levels",serde_json::json!({"id":shared.id,"me":h.levels[0],"remote":h.levels[1],"duration":h.seconds[0].max(h.seconds[1]),"capture":h}));
        if published.elapsed() >= Duration::from_secs(2) || !h.recording {
            shared.publish(true).await;
            published = Instant::now();
        }
    }
    let result = worker.await.map_err(|_| {
        "The local audio worker stopped unexpectedly. Saved files can be recovered.".to_string()
    })?;
    shared.meeting.lock().await.status = "finishing".into();
    shared.publish(true).await;
    crate::tray::update_tray_menu(&shared.app);
    result
}
