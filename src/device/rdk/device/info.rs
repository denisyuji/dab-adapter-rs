use std::time::{SystemTime, UNIX_EPOCH};

use crate::dab::structs::DabError;
use crate::dab::structs::DeviceInfoRequest;
use crate::dab::structs::DisplayType;
use crate::dab::structs::GetDeviceInformationResponse;
use crate::dab::structs::NetworkInterface;
use crate::dab::structs::NetworkInterfaceType;
use crate::device::rdk::interface::get_device_id;
use crate::device::rdk::interface::http_post;
use crate::device::rdk::interface::{deserialize_string_or_number, get_rdk_device_info, get_thunder_property};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(non_snake_case)]
#[allow(dead_code)]
#[allow(unused_mut)]
pub fn process(_dab_request: DeviceInfoRequest) -> Result<String, DabError> {
    let mut ResponseOperator = GetDeviceInformationResponse::default();
    // *** Fill in the fields of the struct DeviceInformation here ***

    //#########org.rdk.DisplaySettings.getConnectedVideoDisplays#########
    #[derive(Serialize)]
    struct GetConnectedVideoDisplaysRequest {
        jsonrpc: String,
        id: i32,
        method: String,
    }

    let request = GetConnectedVideoDisplaysRequest {
        jsonrpc: "2.0".into(),
        id: 3,
        method: "org.rdk.DisplaySettings.getConnectedVideoDisplays".into(),
    };

    #[derive(Deserialize)]
    struct GetConnectedVideoDisplaysResponse {
        jsonrpc: String,
        id: i32,
        result: GetConnectedVideoDisplaysResult,
    }

    #[derive(Deserialize)]
    struct GetConnectedVideoDisplaysResult {
        success: bool,
        connectedVideoDisplays: Vec<String>,
    }

    let json_string = serde_json::to_string(&request).unwrap();
    let response = http_post(json_string)?;
    let ConnectedVideoDisplays: GetConnectedVideoDisplaysResponse;
    ConnectedVideoDisplays = serde_json::from_str(&response).unwrap();

    //######### Map from Static Hashmap: Begin #########

    ResponseOperator.manufacturer = get_rdk_device_info("manufacturer")?;
    ResponseOperator.model = get_rdk_device_info("model")?;
    ResponseOperator.serialNumber = get_rdk_device_info("serialnumber")?;
    ResponseOperator.chipset = get_rdk_device_info("chipset")?;
    // Both firmwareVersion and firmwareBuild are same for RDKV devices.
    ResponseOperator.firmwareVersion = get_rdk_device_info("firmwareversion")?;
    ResponseOperator.firmwareBuild = get_rdk_device_info("firmwareversion")?;

    //######### Map from Static Hashmap: End #########

    //#########org.rdk.RDKShell.getScreenResolution#########
    #[derive(Serialize)]
    struct GetScreenResolutionRequest {
        jsonrpc: String,
        id: i32,
        method: String,
    }
    // Equivalent to DisplayInfo.width and DisplayInfo.height
    let request = GetScreenResolutionRequest {
        jsonrpc: "2.0".into(),
        id: 3,
        method: "org.rdk.RDKShell.getScreenResolution".into(),
    };

    #[derive(Deserialize)]
    struct GetScreenResolutionResponse {
        jsonrpc: String,
        id: i32,
        result: GetScreenResolutionResult,
    }

    #[derive(Deserialize)]
    struct GetScreenResolutionResult {
        w: u32,
        h: u32,
        success: bool,
    }

    let json_string = serde_json::to_string(&request).unwrap();
    let response = http_post(json_string)?;

    let ScreenResolution: GetScreenResolutionResponse;
    ScreenResolution = serde_json::from_str(&response).unwrap();

    //#########org.rdk.Network.getInterfaces#########
    #[derive(Serialize)]
    struct GetInterfacesRequest {
        jsonrpc: String,
        id: i32,
        method: String,
    }

    let request = GetInterfacesRequest {
        jsonrpc: "2.0".into(),
        id: 3,
        method: "org.rdk.Network.getInterfaces".into(),
    };

    #[derive(Deserialize)]
    struct Interface {
        interface: String,
        macAddress: String,
        enabled: bool,
        connected: bool,
    }

    #[derive(Deserialize)]
    struct GetInterfacesResult {
        interfaces: Vec<Interface>,
    }

    #[derive(Deserialize)]
    struct GetInterfacesResponse {
        jsonrpc: String,
        id: i32,
        result: GetInterfacesResult,
    }

    let json_string = serde_json::to_string(&request).unwrap();
    let response = http_post(json_string)?;
    let mut Interfaces: GetInterfacesResponse;
    Interfaces = serde_json::from_str(&response).unwrap();

    //#########DeviceInfo.systeminfo#########
 
    let mut device_uptime: u64 = match get_thunder_property("DeviceInfo.systeminfo","uptime") {
        Ok(uptime) => uptime.parse::<u64>().unwrap_or(0),
        Err(err) => return Err(err),
    };

    //######### Correlate Fields #########
    // Sort interfaces to prioritize connected ones, with Ethernet before WiFi
    Interfaces.result.interfaces.sort_by(|a, b| {
        match (a.connected, b.connected) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                // If both connected or both disconnected, prioritize Ethernet over WiFi
                match (a.interface.as_str(), b.interface.as_str()) {
                    ("ETHERNET", "WIFI") => std::cmp::Ordering::Less,
                    ("WIFI", "ETHERNET") => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            }
        }
    });

    for iface in Interfaces.result.interfaces.iter_mut() {
        let mut interface = NetworkInterface {
            r#type: NetworkInterfaceType::Other,
            ..Default::default()
        };

        match iface.interface.clone().as_str() {
            "ETHERNET" => interface.r#type = NetworkInterfaceType::Ethernet,
            "WIFI" => interface.r#type = NetworkInterfaceType::Wifi,
            _ => interface.r#type = NetworkInterfaceType::Other,
        }

        interface.connected = iface.connected;
        interface.macAddress = iface.macAddress.clone();
        
        // Only get IP settings for connected interfaces
        if !iface.connected {
            ResponseOperator.networkInterfaces.push(interface);
            continue;
        }
        
        // #########org.rdk.Network.getIPSettings#########

        #[derive(Serialize)]
        struct GetIPSettingsRequest {
            jsonrpc: String,
            id: i32,
            method: String,
            params: GetIPSettingsRequestParams,
        }

        #[derive(Serialize)]
        struct GetIPSettingsRequestParams {
            interface: String,
        }

        let req_params = GetIPSettingsRequestParams {
            interface: iface.interface.clone(),
        };

        let request = GetIPSettingsRequest {
            jsonrpc: "2.0".into(),
            id: 3,
            method: "org.rdk.Network.getIPSettings".into(),
            params: req_params,
        };

        let json_string = serde_json::to_string(&request).unwrap();
        let response = http_post(json_string)?;

        // Check if the response indicates failure before deserializing
        let response_value: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| DabError::Err500(format!("Failed to parse IP settings response: {}", e)))?;
        
        if response_value.get("result")
            .and_then(|r| r.get("success"))
            .and_then(|s| s.as_bool())
            == Some(false)
        {
            // If success is false, skip IP settings but still add the interface
            ResponseOperator.networkInterfaces.push(interface);
            continue;
        }

        // Safely extract IP address and DNS entries from the dynamic JSON, accepting
        // both strings and numbers (which are converted to strings).
        let to_string = |v: &Value| -> Option<String> {
            match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            }
        };

        if let Some(result) = response_value.get("result") {
            if let Some(v) = result.get("ipaddr").and_then(to_string) {
                interface.ipAddress = v;
            }

            for key in ["primarydns", "secondarydns"] {
                if let Some(dns) = result.get(key).and_then(to_string) {
                    if !dns.is_empty() {
                        interface.dns.push(dns);
                    }
                }
            }
        }

        ResponseOperator.networkInterfaces.push(interface);
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string());

    match now_ms {
        Err(err) => return Err(DabError::Err400(err)),
        _ => {}
    }

    let ms_since_epoch = (now_ms.unwrap().as_secs() - device_uptime) * 1000;

    ResponseOperator.uptimeSince = ms_since_epoch;
    // DAB device/info needs : Current screen resolution width & height measured in pixels
    ResponseOperator.screenWidthPixels = ScreenResolution.result.w;
    ResponseOperator.screenHeightPixels = ScreenResolution.result.h;
    ResponseOperator.deviceId = get_device_id()?;

    if ConnectedVideoDisplays.result.connectedVideoDisplays.len() > 0
        && ConnectedVideoDisplays.result.connectedVideoDisplays[0].contains("HDMI")
    {
        ResponseOperator.displayType = DisplayType::External;
    } else {
        ResponseOperator.displayType = DisplayType::Native;
    }

    // *******************************************************************
    Ok(serde_json::to_string(&ResponseOperator).unwrap())
}