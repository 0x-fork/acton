use axum::body::Body;
use axum::http::Request;
use expect_test::expect;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn reference_serves_only_supported_operations() {
    let app = super::router();
    let response = app
        .clone()
        .oneshot(Request::get("/openapi.json").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let document: Value = serde_json::from_slice(&body).unwrap();
    let mut methods = Vec::new();

    for (path, item) in document["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            methods.push(format!("{} {path}", method.to_uppercase()));
            assert!(operation.get("tags").is_none());
        }
    }

    // Every shared response type must be registered, including nested references.
    let mut pending = vec![&document];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref") {
                    let pointer = reference.as_str().unwrap().strip_prefix('#').unwrap();
                    assert!(
                        document.pointer(pointer).is_some(),
                        "unresolved {reference}"
                    );
                }
                pending.extend(object.values());
            }
            Value::Array(array) => pending.extend(array),
            _ => {}
        }
    }

    let home = app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let storage_page = app
        .clone()
        .oneshot(Request::get("/storage").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let decoder = app
        .clone()
        .oneshot(
            Request::get("/storage-decoder.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page = app
        .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let contract = json!({
        "status": status,
        "servers": document["servers"],
        "methods": methods,
        "stream_content_types": document["paths"]["/api/streaming/sse"]["post"]
            ["responses"]["200"]["content"].as_object().unwrap().keys().collect::<Vec<_>>(),
        "stream_state_required": document["components"]["schemas"]["AccountStateEvent"]
            ["properties"]["account_state"]["required"],
        "stream_storage_required": document["components"]["schemas"]["StorageUpdateEvent"]["required"],
        "stream_types": document["components"]["schemas"]["SubscriptionType"],
        "stream_fields": document["components"]["schemas"]["Subscription"]["properties"]["fields"],
        "stream_abi": document["components"]["schemas"]["Subscription"]["properties"]["abi"],
        "stream_include_code_data": document["components"]["schemas"]["Subscription"]
            ["properties"]["include_code_data"],
        "redirect": [home.status().as_str(), home.headers()["location"].to_str().unwrap()],
        "storage_page": [storage_page.status().as_str(), storage_page.headers()["content-type"].to_str().unwrap()],
        "storage_decoder": [decoder.status().as_str(), decoder.headers()["content-type"].to_str().unwrap()],
        "page": [page.status().as_str(), page.headers()["content-type"].to_str().unwrap()],
        "wait_timeouts": [
            document["components"]["schemas"]["SendBocAndWaitRequest"]["properties"]["timeout_ms"],
            document["components"]["schemas"]["SendBocAndWaitTraceRequest"]["properties"]["timeout_ms"],
        ],
    });

    expect![[r#"
        {
          "methods": [
            "GET /api/account",
            "GET /api/masterchainInfo",
            "POST /api/runGetMethod",
            "POST /api/send",
            "POST /api/sendAndWaitTrace",
            "POST /api/sendAndWaitTransaction",
            "POST /api/streaming/sse",
            "GET /api/transactions"
          ],
          "page": [
            "200",
            "text/html; charset=utf-8"
          ],
          "redirect": [
            "307",
            "/docs"
          ],
          "servers": [
            {
              "url": "/"
            }
          ],
          "status": 200,
          "storage_decoder": [
            "200",
            "text/javascript; charset=utf-8"
          ],
          "storage_page": [
            "200",
            "text/html; charset=utf-8"
          ],
          "stream_abi": {
            "description": "Tolk ABI with a storage type; required only for `storage_fields` subscriptions.\nOne ABI applies to all subscribed addresses, at most 16 for this event type",
            "type": [
              "object",
              "null"
            ]
          },
          "stream_content_types": [
            "text/event-stream"
          ],
          "stream_fields": {
            "description": "Selected storage paths, for example seqno or settings.owner. Typed cells\nare transparent. Required with `storage_fields`; 1–64 unique paths",
            "example": [
              "seqno",
              "settings.owner"
            ],
            "items": {
              "type": "string"
            },
            "maxItems": 64,
            "minItems": 1,
            "type": [
              "array",
              "null"
            ]
          },
          "stream_include_code_data": {
            "default": true,
            "description": "Include code/data in account state events; false omits both on every event.\nOmitted or null defaults to true. Transaction events are unaffected",
            "example": false,
            "type": [
              "boolean",
              "null"
            ]
          },
          "stream_state_required": [
            "@type",
            "balance",
            "extra_currencies",
            "last_transaction_id",
            "block_id",
            "frozen_hash",
            "sync_utime",
            "state",
            "suspended"
          ],
          "stream_storage_required": [
            "type",
            "finality",
            "address",
            "mc_seqno",
            "data",
            "initial",
            "changed_fields"
          ],
          "stream_types": {
            "enum": [
              "transactions",
              "account_states",
              "storage_fields"
            ],
            "type": "string"
          },
          "wait_timeouts": [
            {
              "default": 30000,
              "description": "Total processing budget after reading the request body; defaults to 30 seconds",
              "format": "int64",
              "maximum": 120000,
              "minimum": 1000,
              "type": [
                "integer",
                "null"
              ]
            },
            {
              "default": 120000,
              "description": "Total trace budget after reading the body; defaults to two minutes",
              "format": "int64",
              "maximum": 600000,
              "minimum": 1000,
              "type": [
                "integer",
                "null"
              ]
            }
          ]
        }
    "#]]
    .assert_eq(&format!(
        "{}\n",
        serde_json::to_string_pretty(&contract).unwrap()
    ));
}
