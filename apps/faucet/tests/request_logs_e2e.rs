use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header::USER_AGENT},
    middleware,
    routing::post,
};
use faucet::middlewares::{
    ACTON_CLIENT_HEADER, DEVICE_UID_HEADER, enter_request_span, require_actonscan_origin,
    require_airdrop_headers,
};
use tower::ServiceExt;
use tracing::instrument::WithSubscriber;

#[derive(Clone, Default)]
struct LogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// Keep log capture in its own test process because tracing callsite interest is global.
#[tokio::test]
async fn info_logs_include_cli_and_browser_client_versions() {
    for json in [false, true] {
        let logs = LogWriter::default();
        let writer = logs.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_ansi(false)
            .without_time()
            .with_writer(move || writer.clone());
        let subscriber = if json {
            tracing::Dispatch::new(
                subscriber
                    .json()
                    .flatten_event(true)
                    .with_current_span(true)
                    .with_span_list(false)
                    .finish(),
            )
        } else {
            tracing::Dispatch::new(subscriber.finish())
        };
        let handler = post(|| async { tracing::info!("Received faucet request") })
            .route_layer(middleware::from_fn(require_actonscan_origin));
        let app = Router::new()
            .route("/challenge", handler.clone())
            .route("/claim", handler)
            .route_layer(middleware::from_fn(require_airdrop_headers))
            .layer(middleware::from_fn(enter_request_span));

        for path in ["/challenge", "/claim"] {
            for (user_agent, browser_client) in [
                ("acton/1.2.0", None),
                ("acton/1.3.0-trunk", None),
                ("Mozilla/5.0", Some("actonscan/1.0.0")),
                ("Mozilla/5.0", Some("actonscan/1.1.0-beta.1")),
                ("acton/1.2.0", Some("actonscan/1.0.0")),
            ] {
                logs.0.lock().unwrap().clear();
                let mut request = Request::post(path)
                    .header(USER_AGENT, user_agent)
                    .header(DEVICE_UID_HEADER, "default");
                if let Some(browser_client) = browser_client {
                    request = request
                        .header(ACTON_CLIENT_HEADER, browser_client)
                        .header("origin", "https://actonscan.com");
                }
                let response = app
                    .clone()
                    .oneshot(request.body(Body::empty()).unwrap())
                    .with_subscriber(subscriber.clone())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);

                let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
                let client = browser_client.unwrap_or(user_agent);
                assert_eq!(output.lines().count(), 1, "{output}");
                if json {
                    let event: serde_json::Value = serde_json::from_str(&output).unwrap();
                    assert_eq!(event["level"], "INFO");
                    assert_eq!(event["span"]["client"], client);
                    assert!(event["span"].get("user_agent").is_none());
                    assert!(event["span"].get("browser_client").is_none());
                    assert!(event["span"]["request_id"].is_string());
                } else {
                    assert!(output.contains("INFO"), "{output}");
                    assert!(output.contains(&format!("client=\"{client}\"")), "{output}");
                    assert!(!output.contains("user_agent="), "{output}");
                    assert!(!output.contains("browser_client="), "{output}");
                    assert!(output.contains("request_id="), "{output}");
                }
            }
        }
    }
}
