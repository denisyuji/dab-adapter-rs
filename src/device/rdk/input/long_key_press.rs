use crate::dab::structs::DabError;
use crate::dab::structs::LongKeyPressRequest;
use crate::dab::structs::LongKeyPressResponse;
use crate::device::rdk::interface::get_keycode;
use crate::device::rdk::interface::http_post;
use crate::device::rdk::input::key::{get_focused_client, needs_direct_injection};
use serde::Serialize;
use serde_json;
use std::thread;
use std::time::Duration;
use std::time::Instant;

#[allow(non_snake_case)]
#[allow(dead_code)]
#[allow(unused_mut)]
pub fn process(_dab_request: LongKeyPressRequest) -> Result<String, DabError> {
    let mut ResponseOperator = LongKeyPressResponse::default();
    // *** Fill in the fields of the struct LongKeyPressResponse here ***

    if _dab_request.keyCode.is_empty() {
        return Err(DabError::Err400(
            "request missing 'keyCode' parameter".to_string(),
        ));
    }

    if _dab_request.durationMs == 0 {
        return Err(DabError::Err400(
            "request missing 'durationMs' parameter".to_string(),
        ));
    }

    let mut KeyCode: u16;

    match _dab_request.keyCode.as_str() {
        "KEY_FAST_FORWARD" => return Err(DabError::Err400("'KEY_FAST_FORWARD' not supported".to_string())),
        _ => {}
    }

    match get_keycode(_dab_request.keyCode.clone()) {
        Some(k) => KeyCode = *k,
        None => return Err(DabError::Err400("keyCode' not found".to_string())),
    }

    let total_time: u64 = _dab_request.durationMs as u64;

    //#########org.rdk.RDKShell.generateKey#########
    #[derive(Serialize)]
    struct GenerateKeyRequest {
        jsonrpc: String,
        id: i32,
        method: String,
        params: GenerateKeyRequestParams,
    }

    #[derive(Serialize)]
    struct GenerateKeyRequestParams {
        keys: Vec<KeyEntry>,
    }

    #[derive(Serialize)]
    struct KeyEntry {
        keyCode: u16,
        modifiers: Vec<String>,
        // Seconds; used as an integer by the RDKShell implementation.
        delay: f64,
        // Seconds between key press and key release.
        duration: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        client: Option<String>,
    }

    let start = Instant::now();

    let client = if needs_direct_injection(_dab_request.keyCode.as_str()) {
        get_focused_client()
    } else {
        None
    };

    let duration_s = (total_time as f64) / 1000.0;

    let key_entry = KeyEntry {
        keyCode: KeyCode,
        modifiers: vec![],
        delay: 0.0,
        duration: duration_s,
        client,
    };

    let req_params = GenerateKeyRequestParams {
        keys: vec![key_entry],
    };

    let request = GenerateKeyRequest {
        jsonrpc: "2.0".into(),
        id: 3,
        method: "org.rdk.RDKShell.1.generateKey".into(),
        params: req_params,
    };

    let json_string = serde_json::to_string(&request).unwrap();

    let deadline = start + Duration::from_millis(total_time);

    http_post(json_string)?;

    let now = Instant::now();
    if now < deadline {
        thread::sleep(deadline.duration_since(now));
    }

    // *******************************************************************
    Ok(serde_json::to_string(&ResponseOperator).unwrap())
}
