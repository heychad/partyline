use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

pub const SCOPES:&str="openid email https://www.googleapis.com/auth/calendar.calendarlist.readonly https://www.googleapis.com/auth/calendar.events.readonly";
#[derive(Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}
const SERVICE: &str = "com.heychad.partyline.google-calendar";
pub fn secret(name: &str) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let bytes = security_framework::passwords::get_generic_password(SERVICE, name)
            .map_err(|_| "Could not unlock Google Calendar in Keychain. Reconnect the account.")?;
        String::from_utf8(bytes).map_err(|_| "Invalid saved Google connection.".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = name;
        Err("Google connection storage requires macOS.".into())
    }
}
pub fn save(name: &str, value: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        security_framework::passwords::set_generic_password(SERVICE, name, value.as_bytes())
            .map_err(|_| "Could not save Google connection in Keychain.".into())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (name, value);
        Err("Google connection storage requires macOS.".into())
    }
}
pub fn remove(name: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        match security_framework::passwords::delete_generic_password(SERVICE, name) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(_) => Err("Could not remove Google connection from Keychain.".into()),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = name;
        Err("Google connection storage requires macOS.".into())
    }
}
pub fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not open Google connection.".into())
}
pub fn nonce() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
pub fn callback(target: &str, state: &str) -> Result<Option<String>, String> {
    let u = url::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(|_| "Invalid sign-in response.")?;
    if u.path() != "/callback" {
        return Ok(None);
    }
    let pairs = u.query_pairs().collect::<Vec<_>>();
    if pairs.iter().filter(|(k, _)| k == "state").count() != 1
        || !pairs.iter().any(|(k, v)| k == "state" && v == state)
    {
        return Ok(None);
    }
    if pairs.iter().any(|(k, _)| k == "error") {
        return Err("Google sign-in was declined. Try Add Google account again.".into());
    }
    if pairs.iter().filter(|(k, _)| k == "code").count() != 1 {
        return Err("Google did not return an authorization code.".into());
    }
    Ok(pairs
        .into_iter()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.into_owned()))
}
async fn token(form: &[(&str, &str)], previous: Option<&Tokens>) -> Result<Tokens, String> {
    let r = client()?
        .post("https://oauth2.googleapis.com/token")
        .form(form)
        .send()
        .await
        .map_err(|_| "Google token request failed. Check your connection and retry.")?;
    if !r.status().is_success() {
        return Err("Google sign-in expired or was rejected. Reconnect this account; check the OAuth client and test-user settings.".into());
    }
    let v: serde_json::Value = r
        .json()
        .await
        .map_err(|_| "Invalid Google token response.")?;
    if let Some(scope) = v["scope"].as_str() {
        for required in SCOPES
            .split_whitespace()
            .filter(|s| s.starts_with("https://"))
        {
            if !scope.split_whitespace().any(|s| s == required) {
                return Err(
                    "Allow both read-only calendar permissions when connecting Google.".into(),
                );
            }
        }
    }
    Ok(Tokens {
        access_token: v["access_token"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("Google returned no access token.")?
            .into(),
        refresh_token: v["refresh_token"]
            .as_str()
            .or_else(|| previous.map(|p| p.refresh_token.as_str()))
            .filter(|s| !s.is_empty())
            .ok_or("Google returned no offline access. Reconnect and approve access.")?
            .into(),
        expires_at: chrono::Utc::now().timestamp()
            + v["expires_in"].as_i64().unwrap_or(3600).clamp(60, 86400),
    })
}
pub async fn refresh(client_id: &str, t: &Tokens) -> Result<Tokens, String> {
    let secret = secret("client-secret")?;
    token(
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", &secret),
            ("refresh_token", &t.refresh_token),
        ],
        Some(t),
    )
    .await
}
pub async fn authorize(
    client_id: &str,
    login_hint: Option<&str>,
    cancel: CancellationToken,
) -> Result<(String, String, Tokens), String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|_| "Could not open the local Google sign-in callback.")?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener
            .local_addr()
            .map_err(|_| "Sign-in address unavailable.")?
            .port()
    );
    let state = nonce();
    let verifier = nonce();
    let proof = challenge(&verifier);
    let client_secret = secret("client-secret")?;
    let mut url = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("access_type", "offline"),
        ("prompt", "consent select_account"),
        ("state", state.as_str()),
        ("code_challenge", proof.as_str()),
        ("code_challenge_method", "S256"),
    ]);
    if let Some(email) = login_hint {
        url.query_pairs_mut().append_pair("login_hint", email);
    }
    std::process::Command::new("open")
        .arg(url.as_str())
        .spawn()
        .map_err(|_| "Could not open Google sign-in in your browser.")?;
    let wait = async {
        loop {
            let (mut stream, peer) = listener
                .accept()
                .await
                .map_err(|_| "Google sign-in callback failed.")?;
            if !peer.ip().is_loopback() {
                continue;
            }
            let read = async {
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 1024];
                loop {
                    let count = stream.read(&mut chunk).await?;
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if bytes.windows(4).any(|w| w == b"\r\n\r\n") || bytes.len() >= 8192 {
                        break;
                    }
                }
                Ok::<_, std::io::Error>(bytes)
            };
            let bytes = match tokio::time::timeout(Duration::from_secs(3), read).await {
                Ok(Ok(b)) if b.len() < 8192 => b,
                _ => continue,
            };
            let request = String::from_utf8_lossy(&bytes);
            let mut parts = request.lines().next().unwrap_or("").split_whitespace();
            if parts.next() != Some("GET") {
                continue;
            }
            let result = callback(parts.next().unwrap_or(""), &state);
            let valid = !matches!(result, Ok(None));
            let body = if valid {
                "Return to PartyLine to finish connecting your calendar."
            } else {
                "Invalid sign-in callback. Return to the original Google sign-in window."
            };
            let response=format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",if valid{"200 OK"}else{"400 Bad Request"},body.len(),body);
            let _ = tokio::time::timeout(
                Duration::from_secs(2),
                stream.write_all(response.as_bytes()),
            )
            .await;
            if let Some(code) = result? {
                break Ok::<_, String>(code);
            }
        }
    };
    let code = tokio::select! {_ = cancel.cancelled()=>return Err("Google sign-in cancelled.".into()),result=tokio::time::timeout(Duration::from_secs(300),wait)=>result.map_err(|_|"Google sign-in timed out. Try Add Google account again.")??};
    let tokens = token(
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", &client_secret),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
        ],
        None,
    )
    .await?;
    let r = client()?
        .get("https://openidconnect.googleapis.com/v1/userinfo")
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .map_err(|_| "Could not read the Google account identity.")?;
    if !r.status().is_success() {
        return Err("Could not verify the Google account identity.".into());
    }
    let profile: serde_json::Value = r
        .json()
        .await
        .map_err(|_| "Invalid Google account response.")?;
    let id = profile["sub"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 128 && s.bytes().all(|b| b.is_ascii_alphanumeric()))
        .ok_or("Google returned an invalid account ID.")?;
    let email = profile["email"]
        .as_str()
        .filter(|s| s.len() < 320)
        .ok_or("Google returned no email address.")?;
    if profile["email_verified"] != true {
        return Err("Google email address is not verified.".into());
    }
    if cancel.is_cancelled() {
        return Err("Google sign-in cancelled.".into());
    }
    Ok((id.into(), email.into(), tokens))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_rfc_vector() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn state_and_path_are_required() {
        assert_eq!(
            callback("/callback?code=private&state=bad", "good").unwrap(),
            None
        );
        assert_eq!(
            callback("/other?code=private&state=good", "good").unwrap(),
            None
        );
        assert_eq!(
            callback("/callback?code=private&state=good", "good").unwrap(),
            Some("private".into())
        );
        assert!(callback("/callback?error=access_denied&state=good", "good").is_err());
        assert!(callback("/callback?code=a&code=b&state=good", "good").is_err());
    }
}

pub fn desktop_config(bytes: &[u8]) -> Result<(String, String), String> {
    let v: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "Choose a valid Google Desktop OAuth client JSON file.")?;
    let c = &v["installed"];
    Ok((
        c["client_id"]
            .as_str()
            .ok_or("Choose a Desktop app client, not a Web application client.")?
            .into(),
        c["client_secret"]
            .as_str()
            .ok_or("The OAuth client file has no client secret.")?
            .into(),
    ))
}
#[cfg(test)]
mod config_tests {
    use super::*;
    #[test]
    fn imports_only_desktop_clients_without_echoing_values() {
        assert_eq!(
            desktop_config(br#"{"installed":{"client_id":"id","client_secret":"secret"}}"#)
                .unwrap(),
            ("id".into(), "secret".into())
        );
        let error =
            desktop_config(br#"{"web":{"client_id":"id","client_secret":"private-secret"}}"#)
                .unwrap_err();
        assert!(!error.contains("private-secret"));
    }
}
