use super::{get, library};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, path::PathBuf, sync::{Mutex,OnceLock}};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings { pub automatic:bool, pub folder:Option<String> }
impl Default for Settings{fn default()->Self{Self{automatic:true,folder:None}}}
#[derive(Clone,Serialize,Deserialize)]
pub struct Backup { pub meeting_id:String,#[serde(default)] pub title:String,pub status:String,pub progress:u8,pub destination:Option<String>,pub error:Option<String>,pub edit_version:u64 }
static RUNNING:OnceLock<Mutex<HashSet<String>>>=OnceLock::new();
fn running()->&'static Mutex<HashSet<String>>{RUNNING.get_or_init(||Mutex::new(HashSet::new()))}
fn data(app:&AppHandle)->Result<PathBuf,String>{app.path().app_data_dir().map_err(|_|"Could not find backup settings.".into())}
fn settings(app:&AppHandle)->Result<Settings,String>{let path=data(app)?.join("backup-settings.json");match std::fs::read(path){Ok(bytes)=>serde_json::from_slice(&bytes).map_err(|_|"Backup settings could not be read. Choose the folder again.".into()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(Settings::default()),Err(_)=>Err("Could not read backup settings.".into())}}
fn destination(app:&AppHandle,path:&str)->Result<PathBuf,String>{
    let folder=std::fs::canonicalize(path).map_err(|_|"That folder is unavailable. Open Google Drive and choose an available folder.")?;
    let cloud=app.path().home_dir().map_err(|_|"Could not find Google Drive folders.")?.join("Library/CloudStorage");
    let relative=folder.strip_prefix(&cloud).map_err(|_|"Choose a folder inside Google Drive for desktop.")?;
    let account=relative.components().next().ok_or("Choose a folder inside a Google Drive account.")?.as_os_str().to_string_lossy();
    if !account.starts_with("GoogleDrive-") || !folder.is_dir(){return Err("Choose a folder inside Google Drive for desktop.".into());}
    Ok(folder)
}
fn publish(app:&AppHandle,state:&Backup)->Result<(),String>{let folder=data(app)?.join("backup-state");std::fs::create_dir_all(&folder).map_err(|_|"Could not save backup progress.")?;library::atomic_json(&folder.join(format!("{}.json",state.meeting_id)),state)?;let _=app.emit("cloud-backup",state);Ok(())}

#[tauri::command]
pub fn cloud_backup_settings(app:AppHandle)->Result<Settings,String>{settings(&app)}
#[tauri::command]
pub fn cloud_backup_configure(app:AppHandle,automatic:bool,folder:Option<String>)->Result<Settings,String>{
    let folder=folder.filter(|s|!s.is_empty()).map(|p|destination(&app,&p).map(|p|p.to_string_lossy().into_owned())).transpose()?;
    let next=Settings{automatic,folder};library::atomic_json(&data(&app)?.join("backup-settings.json"),&next)?;
    let _=app.emit("cloud-backup-settings",&next);Ok(next)
}
#[tauri::command]
pub async fn cloud_backup_choose_folder(app:AppHandle)->Result<Value,String>{
    let (send,receive)=tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Choose a Google Drive folder for PartyLine backups").pick_folder(move|folder|{let _=send.send(folder);});
    let Some(folder)=receive.await.map_err(|_|"The folder chooser closed unexpectedly.")? else{return Ok(json!({"cancelled":true}));};
    let path=folder.into_path().map_err(|_|"Choose a local Google Drive folder.")?;
    let current=settings(&app)?;
    Ok(json!({"cancelled":false,"settings":cloud_backup_configure(app,current.automatic,Some(path.to_string_lossy().into_owned()))?}))
}
#[tauri::command]
pub fn cloud_backup_status(app:AppHandle)->Result<Value,String>{
    let folder=data(&app)?.join("backup-state");let mut states=Vec::new();
    if folder.is_dir(){for entry in std::fs::read_dir(folder).map_err(|_|"Could not read backup status.")?.flatten(){if let Ok(bytes)=std::fs::read(entry.path()){if let Ok(mut state)=serde_json::from_slice::<Backup>(&bytes){if ["queued","copying"].contains(&state.status.as_str()) && !running().lock().unwrap().contains(&state.meeting_id){state.status="interrupted".into();state.error=Some("PartyLine closed during backup. Retry this recording.".into());}states.push(state);}}}}
    Ok(json!({"items":states,"cloud_upload_verified":false,"sync_note":"Google Drive handles cloud upload after the local copy. Check Drive for upload status."}))
}
#[tauri::command]
pub async fn cloud_backup_start(app:AppHandle,id:String)->Result<Value,String>{
    let config=settings(&app)?;let selected=config.folder.ok_or("Choose a Google Drive folder in Settings first.")?;
    let target=destination(&app,&selected)?;
    let s=get(&app,&id).await?;let m=s.meeting.lock().await.clone();library::idle(&m)?;
    if m.deleted_at.is_some(){return Err("Restore the recording before backing it up.".into());}
    {let mut active=running().lock().unwrap();if !active.insert(id.clone()){return Err("This recording is already being backed up.".into());}}
    let initial=Backup{meeting_id:id.clone(),title:m.title.clone(),status:"queued".into(),progress:0,destination:None,error:None,edit_version:m.edit_version};
    if let Err(error)=publish(&app,&initial){running().lock().unwrap().remove(&id);return Err(error);}
    let first=initial.clone();
    tauri::async_runtime::spawn(async move{
        let mut state=initial;
        let result=async{
            let _guard=s.write_lock.lock().await;let snapshot=s.meeting.lock().await.clone();
            library::idle(&snapshot)?;if snapshot.deleted_at.is_some(){return Err("The recording moved to Trash before backup started.".to_string());}
            state.edit_version=snapshot.edit_version;
            state.title=snapshot.title.clone();
            let directory=target.join(&id);safe_directory(&directory)?;
            let destination=directory.join(format!("revision-{}",snapshot.edit_version));safe_directory(&destination)?;
            state.destination=Some(destination.to_string_lossy().into_owned());state.status="copying".into();state.progress=5;publish(&app,&state)?;
            for (index,(channel,name)) in [("me","microphone.wav"),("remote","mac-audio.wav")].into_iter().enumerate(){
                let from=s.folder.clone();let to=destination.join(name);let copy=snapshot.clone();
                tokio::task::spawn_blocking(move||library::render_audio(&copy,&from,channel,&to)).await.map_err(|_|"Audio backup could not finish.".to_string())??;
                state.progress=40+index as u8*40;publish(&app,&state)?;
            }
            let transcript=library::export_text(&snapshot,"transcript","markdown")?;
            library::atomic_text(&destination.join("transcript.md"),&transcript)?;
            let notes=if snapshot.notes_need_review{"Notes need review after recording edits.".to_owned()}else{snapshot.notes.clone()};
            library::atomic_text(&destination.join("notes.md"),&notes)?;
            library::atomic_json(&destination.join("recording.json"),&json!({"id":snapshot.id,"title":snapshot.title,"created_at":snapshot.created_at,"edit_version":snapshot.edit_version,"start":snapshot.trim_start,"end":snapshot.trim_end.unwrap_or(snapshot.duration),"redactions_applied":!snapshot.redactions.is_empty(),"notes_need_review":snapshot.notes_need_review}))?;
            library::atomic_json(&directory.join("latest.json"),&json!({"revision":snapshot.edit_version,"folder":format!("revision-{}",snapshot.edit_version)}))?;
            Ok::<(),String>(())
        }.await;
        match result{Ok(())=>{state.status="copied".into();state.progress=100;},Err(error)=>{state.status="failed".into();state.error=Some(error);}}
        let _=publish(&app,&state);running().lock().unwrap().remove(&id);
    });
    Ok(json!({"accepted":true,"backup":first}))
}
fn safe_directory(path:&std::path::Path)->Result<(),String>{match std::fs::symlink_metadata(path){Ok(meta) if !meta.is_dir()=>Err("Backup destination contains an unsafe path. Choose another folder.".into()),Ok(_)=>Ok(()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>std::fs::create_dir(path).map_err(|_|"Could not create the backup folder.".into()),Err(_)=>Err("Could not inspect the backup folder.".into())}}
pub async fn automatic(app:AppHandle,id:String){if settings(&app).is_ok_and(|s|s.automatic && s.folder.is_some()){if let Err(error)=cloud_backup_start(app.clone(),id.clone()).await{let title=match get(&app,&id).await{Ok(s)=>s.meeting.lock().await.title.clone(),Err(_)=>String::new()};let _=publish(&app,&Backup{meeting_id:id,title,status:"failed".into(),progress:0,destination:None,error:Some(error),edit_version:0});}}}
