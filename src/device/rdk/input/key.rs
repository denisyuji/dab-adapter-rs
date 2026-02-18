pub mod list;

use crate::device::rdk::interface::rdk_request;
use crate::device::rdk::interface::RdkResponse;
use serde::Deserialize;

// Keys that need to be sent directly to the focused app to bypass
// platform-level key intercepts.
pub(crate) fn needs_direct_injection(key_code: &str) -> bool {
    matches!(key_code, "KEY_VOLUME_UP" | "KEY_VOLUME_DOWN" | "KEY_MUTE")
}

fn is_system_client(client: &str) -> bool {
    client.starts_with("subtec_")
        || client.starts_with("test-")
        || client == "rdkshell_display"
}

pub(crate) fn get_focused_client() -> Option<String> {
    #[derive(Deserialize)]
    struct GetZOrderResult {
        clients: Vec<String>,
        success: bool,
    }

    rdk_request::<RdkResponse<GetZOrderResult>>("org.rdk.RDKShell.1.getZOrder")
        .ok()
        .and_then(|response| {
            if !response.result.success {
                return None;
            }

            response
                .result
                .clients
                .into_iter()
                .find(|c| !is_system_client(c))
        })
}
