use super::super::library::calendar_model::{classify, Signal};
pub fn sample() -> Result<Vec<Signal>, String> {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn partyline_microphone_processes() -> *mut std::ffi::c_char;
            fn partyline_free_detection(value: *mut std::ffi::c_char);
        }
        let v: serde_json::Value = unsafe {
            let ptr = partyline_microphone_processes();
            if ptr.is_null() {
                return Err("Could not inspect call activity.".into());
            }
            let parsed = serde_json::from_slice(std::ffi::CStr::from_ptr(ptr).to_bytes());
            partyline_free_detection(ptr);
            parsed.map_err(|_| "Invalid microphone activity response.")?
        };
        if let Some(e) = v["error"].as_str() {
            return Err(e.into());
        }
        let mut unique = std::collections::BTreeMap::new();
        for b in v["bundles"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b.as_str())
        {
            if let Some((app, evidence)) = classify(b) {
                unique.insert(
                    app.to_string(),
                    Signal {
                        id: app.into(),
                        app: app.into(),
                        evidence: evidence.into(),
                    },
                );
            }
        }
        Ok(unique.into_values().collect())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Meeting detection currently requires macOS.".into())
    }
}
