use serde::Serialize;
const SERVICE: &str = "com.heychad.partyline.cloud";
#[derive(Serialize)]
pub struct KeyStatus {
    pub assemblyai: bool,
    pub openai: bool,
}
#[cfg(target_os = "macos")]
pub fn read(provider: &str) -> Result<String, String> {
    security_framework::passwords::get_generic_password(SERVICE, provider)
        .map_err(|_| format!("Enter your {} API key in Connections.", provider))
        .and_then(|v| String::from_utf8(v).map_err(|_| "Invalid saved API key.".into()))
}
#[cfg(not(target_os = "macos"))]
pub fn read(_provider: &str) -> Result<String, String> {
    Err("Cloud key storage currently requires macOS.".into())
}
#[tauri::command]
pub fn cloud_key_status() -> KeyStatus {
    KeyStatus {
        assemblyai: read("assemblyai").is_ok(),
        openai: read("openai").is_ok(),
    }
}
#[tauri::command]
pub fn cloud_save_keys(
    assemblyai: Option<String>,
    openai: Option<String>,
) -> Result<KeyStatus, String> {
    for (provider, key) in [("assemblyai", assemblyai), ("openai", openai)] {
        if let Some(key) = key {
            let key = key.trim();
            if key.is_empty() {
                continue;
            }
            if key.len() > 512 || key.chars().any(char::is_whitespace) {
                return Err("An API key contains whitespace or is too long.".into());
            }
            #[cfg(target_os = "macos")]
            security_framework::passwords::set_generic_password(SERVICE, provider, key.as_bytes())
                .map_err(|_| "Could not save the API key in Keychain.".to_string())?;
            #[cfg(not(target_os = "macos"))]
            return Err("Cloud key storage currently requires macOS.".into());
        }
    }
    Ok(cloud_key_status())
}
#[tauri::command]
pub fn cloud_remove_key(provider: String) -> Result<KeyStatus, String> {
    if !["assemblyai", "openai"].contains(&provider.as_str()) {
        return Err("Unknown provider.".into());
    }
    #[cfg(target_os = "macos")]
    security_framework::passwords::delete_generic_password(SERVICE, &provider)
        .map_err(|_| "Could not remove the API key from Keychain.".to_string())?;
    Ok(cloud_key_status())
}
#[tauri::command]
pub async fn cloud_test_keys() -> Result<String, String> {
    let a = read("assemblyai")?;
    let o = read("openai")?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| "Could not create connection.".to_string())?;
    let r = client
        .get("https://streaming.assemblyai.com/v3/token")
        .query(&[("expires_in_seconds", "60")])
        .header("Authorization", a)
        .send()
        .await
        .map_err(|_| "Could not reach AssemblyAI. Check your connection.".to_string())?;
    if !r.status().is_success() {
        return Err(format!(
            "AssemblyAI rejected the connection (HTTP {}). Check your API key and account credits.",
            r.status().as_u16()
        ));
    }
    let r = client
        .get("https://api.openai.com/v1/models/gpt-6.1-terra")
        .bearer_auth(o)
        .send()
        .await
        .map_err(|_| "Could not reach OpenAI. Check your connection.".to_string())?;
    if !r.status().is_success() {
        return Err(format!(
            "OpenAI could not access GPT-6.1 Terra (HTTP {}). Check your API key and model access.",
            r.status().as_u16()
        ));
    }
    Ok("AssemblyAI and Terra are connected.".into())
}
