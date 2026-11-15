use serde_json::{json,Value};
use tauri::{AppHandle,Manager};

fn resources(app:&AppHandle)->Result<std::path::PathBuf,String>{
    let bundled=app.path().resource_dir().map_err(|_|"Could not locate the MCP bridge.")?.join("agent-control");
    if bundled.join("server.py").is_file(){return Ok(bundled);}
    let development=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/agent-control");
    if cfg!(debug_assertions) && development.join("server.py").is_file(){return Ok(development);}
    Err("The MCP bridge is missing. Install a complete PartyLine build.".into())
}
#[tauri::command]
pub fn cloud_client_status(app:AppHandle)->Result<Value,String>{
    let resource=resources(&app)?;
    Ok(json!({"server":resource.join("server.py"),"clients":["codex","claude-code","claude-desktop"],"transport":"stdio","app_required":true,"restart_client_after_setup":true}))
}
#[tauri::command]
pub async fn cloud_client_setup(app:AppHandle,client:String)->Result<Value,String>{
    if !["codex","claude-code","claude-desktop"].contains(&client.as_str()){return Err("Choose Codex, Claude Code, or Claude Desktop.".into());}
    let resource=resources(&app)?;
    // Install a stable copy so moving/updating the app does not break client configuration.
    let bridge=app.path().app_data_dir().map_err(|_|"Could not locate app data.")?.join("agent-control/client");
    tokio::fs::create_dir_all(&bridge).await.map_err(|_|"Could not install the MCP bridge.")?;
    for name in ["server.py","tools.json"]{
        tokio::fs::copy(resource.join(name),bridge.join(name)).await.map_err(|_|"Could not copy the MCP bridge. Reinstall PartyLine.")?;
    }
    let python=if std::path::Path::new("/opt/homebrew/bin/python3").is_file(){"/opt/homebrew/bin/python3"}else{"/usr/bin/python3"};
    let output=tokio::process::Command::new(python).arg(resource.join("setup.py")).arg("--client").arg(&client).arg("--server").arg(bridge.join("server.py")).output().await.map_err(|_|"Could not start MCP setup. Python 3 is required.")?;
    if !output.status.success(){return Err("Client setup failed. Check that its CLI is installed and review any existing PartyLine MCP entry.".into());}
    serde_json::from_slice(&output.stdout).map_err(|_|"MCP setup returned an invalid result.".into())
}
