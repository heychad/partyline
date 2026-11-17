//! User-initiated setup check. Discards samples; never saves or uploads audio.
use super::keys;
use crate::audio::{capture::AudioCaptureBackend, devices::{default_input_device,AudioDevice,DeviceType as Kind}, recording_state::{DeviceType,RecordingState}, stream::AudioStream};
use serde::Serialize;
use std::sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering}};
use tauri::{AppHandle,Emitter};
use tokio_util::sync::CancellationToken;

pub(super) static RUNNING:AtomicBool=AtomicBool::new(false);
static STATUS:OnceLock<Mutex<SetupStatus>>=OnceLock::new();
static CANCEL:OnceLock<Mutex<Option<CancellationToken>>>=OnceLock::new();
#[derive(Clone,Default,Serialize)]
pub struct SetupStatus {
    status:String, phase:String, error:Option<String>,
    microphone_received:bool, mac_audio_received:bool,
    microphone_signal:bool, mac_audio_signal:bool,
    connections_unlocked:bool,
}
fn status()-> &'static Mutex<SetupStatus>{STATUS.get_or_init(||Mutex::new(SetupStatus::default()))}
fn publish(app:&AppHandle, update:impl FnOnce(&mut SetupStatus)) {
    let mut current=status().lock().unwrap();update(&mut current);
    let _=app.emit("cloud-setup",current.clone());
}
#[tauri::command]
pub fn cloud_setup_status()->SetupStatus{status().lock().unwrap().clone()}
#[tauri::command]
pub fn cloud_setup_cancel()->SetupStatus{
    if let Some(token)=CANCEL.get_or_init(||Mutex::new(None)).lock().unwrap().as_ref(){token.cancel();}
    cloud_setup_status()
}
#[tauri::command]
pub async fn cloud_setup_start(app:AppHandle,microphone:Option<String>)->Result<SetupStatus,String>{
    if crate::audio::recording_commands::is_recording().await{return Err("Finish the recording before checking setup.".into());}
    {
        let active=super::ACTIVE.lock().unwrap();
        if active.is_some()||RUNNING.swap(true,Ordering::SeqCst){return Err("Finish the recording or current setup check first.".into());}
    }
    let cancel=CancellationToken::new();
    *CANCEL.get_or_init(||Mutex::new(None)).lock().unwrap()=Some(cancel.clone());
    publish(&app,|s|*s=SetupStatus{status:"running".into(),phase:"unlocking".into(),..Default::default()});
    let accepted=cloud_setup_status();
    tauri::async_runtime::spawn(async move {
        let worker_app=app.clone();let worker_cancel=cancel.clone();
        let result=tokio::task::spawn_blocking(move||{
            let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_|"Could not start setup check.".to_string())?;
            runtime.block_on(async move {
                keys::read("assemblyai")?;keys::read("openai")?;
                if worker_cancel.is_cancelled(){return Ok(());}
                publish(&worker_app,|s|{s.connections_unlocked=true;s.phase="checking_audio".into();});
                probe(worker_app,microphone,worker_cancel).await
            })
        }).await.unwrap_or_else(|_|Err("Setup check stopped unexpectedly. Try again.".into()));
        publish(&app,|s|{s.status=if cancel.is_cancelled(){"cancelled"}else if result.is_ok(){"complete"}else{"failed"}.into();s.phase.clear();s.error=result.err();});
        RUNNING.store(false,Ordering::SeqCst);
    });
    Ok(accepted)
}
async fn probe(app:AppHandle,microphone:Option<String>,cancel:CancellationToken)->Result<(),String>{
    let state=RecordingState::new();let(tx,mut rx)=tokio::sync::mpsc::unbounded_channel();state.set_audio_sender(tx);
    let mic=match microphone.filter(|s|!s.is_empty()){Some(name)=>AudioDevice::new(name,Kind::Input),None=>default_input_device().map_err(|_|"No microphone is available.")?};
    let mic_stream=AudioStream::create(Arc::new(mic),state.clone(),DeviceType::Microphone,None).await.map_err(|e|format!("Could not open the microphone. Check its macOS permission. {e}"))?;
    let system=AudioStream::create_with_backend(Arc::new(AudioDevice::new("Mac audio".into(),Kind::Output)),state.clone(),DeviceType::System,None,AudioCaptureBackend::CoreAudio).await;
    let system=match system{Ok(s)=>s,Err(e)=>{let _=mic_stream.stop();return Err(format!("Could not open Mac audio. Check its macOS permission. {e}"));}};
    let result=async {
        state.start_recording().map_err(|e|e.to_string())?;
        let deadline=tokio::time::sleep(std::time::Duration::from_secs(8));tokio::pin!(deadline);
        loop {tokio::select!{
            _=cancel.cancelled()=>break,
            _=&mut deadline=>break,
            chunk=rx.recv()=>{let Some(chunk)=chunk else{return Err("Audio devices stopped during setup.".into());};
                let signal=chunk.data.iter().any(|x|x.abs()>0.0001);
                publish(&app,|s|if chunk.device_type==DeviceType::Microphone{s.microphone_received=true;s.microphone_signal|=signal;}else{s.mac_audio_received=true;s.mac_audio_signal|=signal;});
            }
        }}
        Ok(())
    }.await;
    state.stop_recording();let _=mic_stream.stop();let _=system.stop();result
}
