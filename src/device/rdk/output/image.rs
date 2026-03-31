use crate::dab::structs::CaptureScreenshotRequest;
use crate::dab::structs::CaptureScreenshotResponse;
use crate::dab::structs::DabError;
use crate::device::rdk::interface::http_post;
use crate::device::rdk::interface::{get_service_state, is_local_device, service_activate};
use serde::Serialize;

use base64::{engine::general_purpose, Engine as _};
use bytes::Bytes;
use crossbeam::channel::{self, Receiver, Sender};
use hyper::server::Server;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use local_ip_address::local_ip;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::thread;
use tokio::runtime::Runtime;
use tokio::time::Duration;

struct UploadServer {
    rx: Receiver<Bytes>,
}

static UPLOAD_SERVER: OnceLock<Result<UploadServer, DabError>> = OnceLock::new();

fn ensure_upload_server() -> Result<&'static UploadServer, DabError> {
    match UPLOAD_SERVER.get_or_init(|| {
        let (tx, rx) = channel::unbounded();

        thread::Builder::new()
            .name("screen-capture-upload".to_string())
            .spawn(move || {
                let rt = Runtime::new().expect("failed to create screenshot runtime");
                rt.block_on(async move {
                    let make_svc = make_service_fn(move |_conn| {
                        let tx = tx.clone();
                        async move {
                            Ok::<_, Infallible>(service_fn(move |req| handle_req(req, tx.clone())))
                        }
                    });

                    let addr = SocketAddr::from(([0, 0, 0, 0], 7878));
                    if let Err(err) = Server::bind(&addr).serve(make_svc).await {
                        eprintln!("screenshot upload server failed: {}", err);
                    }
                });
            })
            .map_err(|e| DabError::Err500(format!("Failed to start upload server: {}", e)))?;

        Ok(UploadServer { rx })
    }) {
        Ok(server) => Ok(server),
        Err(err) => Err(match err {
            DabError::Err400(msg) => DabError::Err400(msg.clone()),
            DabError::Err500(msg) => DabError::Err500(msg.clone()),
            DabError::Err501(msg) => DabError::Err501(msg.clone()),
        }),
    }
}

fn upload_url() -> Result<String, DabError> {
    let host = if is_local_device() {
        "127.0.0.1".to_string()
    } else {
        local_ip()
            .map(|ip| ip.to_string())
            .map_err(|e| DabError::Err500(format!("Failed to resolve local IP: {}", e)))?
    };

    Ok(format!("http://{}:7878/upload", host))
}

#[allow(non_snake_case)]
#[allow(dead_code)]
#[allow(unused_mut)]
pub fn process(_dab_request: CaptureScreenshotRequest) -> Result<String, DabError> {
    //######### Activate org.rdk.ScreenCapture #########
    if get_service_state("org.rdk.ScreenCapture")? != "activated" {
        service_activate("org.rdk.ScreenCapture".to_string())?;
        thread::sleep(Duration::from_millis(500));
    }

    let mut ResponseOperator = CaptureScreenshotResponse::default();
    let upload_server = ensure_upload_server()?;

    while upload_server.rx.try_recv().is_ok() {}

    //#########org.rdk.ScreenCapture.uploadScreenCapture#########
    #[derive(Serialize)]
    struct UploadScreenCaptureRequest {
        jsonrpc: String,
        id: i32,
        method: String,
        params: UploadScreenCaptureRequestParams,
    }

    #[derive(Serialize)]
    struct UploadScreenCaptureRequestParams {
        url: String,
        callGUID: String,
    }

    let req_params = UploadScreenCaptureRequestParams {
        url: upload_url()?,
        callGUID: "12345".to_string(),
    };

    let request = UploadScreenCaptureRequest {
        jsonrpc: "2.0".into(),
        id: 3,
        method: "org.rdk.ScreenCapture.uploadScreenCapture".into(),
        params: req_params,
    };

    let json_string = serde_json::to_string(&request).unwrap();
    http_post(json_string)?;

    match upload_server.rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(data) => {
            let b64 = general_purpose::STANDARD.encode(&data);
            let b64 = format!("data:image/png;base64,{}", b64);

            ResponseOperator.outputImage = b64;
            Ok(serde_json::to_string(&ResponseOperator).unwrap())
        }
        Err(channel::RecvTimeoutError::Timeout) => Err(DabError::Err500(
            "Timed out waiting for a screenshot upload".to_string(),
        )),
        Err(channel::RecvTimeoutError::Disconnected) => Err(DabError::Err500(
            "The screenshot upload server stopped unexpectedly".to_string(),
        )),
    }
}

async fn handle_req(
    req: Request<Body>,
    tx: Sender<Bytes>,
) -> Result<Response<Body>, Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/upload") => {
            let whole_body = hyper::body::to_bytes(req.into_body()).await.unwrap();

            if tx.send(whole_body).is_err() {
                return Ok(Response::new(Body::from("Error processing the request")));
            }
            Ok(Response::new(Body::from("File processed successfully")))
        }
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}
