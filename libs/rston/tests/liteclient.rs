#![cfg(feature = "liteclient")]

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use rston::adnl::{
    AdnlPeer,
    crypto::{KeyPair, SecretKey},
};
use rston::liteclient::{
    ConfigGlobal, ConfigLiteServer, ConfigPublicKey, LiteClient, LiteClientOptions, LiteClientPool,
    LiteError, peer::LitePeer,
};
use rston::tl::{
    LiteQueryRaw, Message, Request, Response, TlError,
    request::{LiteQuery, WaitMasterchainSeqno},
    response::{CurrentTime, Error as ServerError},
};
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};

struct TestServer {
    config: ConfigLiteServer,
    task: JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TestServer {
    async fn start(reply: impl Fn(Vec<u8>) -> Vec<u8> + Send + Sync + 'static) -> Self {
        Self::with_delay(Duration::ZERO, reply).await
    }

    async fn with_delay(
        delay: Duration,
        reply: impl Fn(Vec<u8>) -> Vec<u8> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let keys = KeyPair::from(&SecretKey::from_bytes([7; 32]));
        let config = ConfigLiteServer {
            ip: u32::from(Ipv4Addr::LOCALHOST) as i32,
            port,
            id: ConfigPublicKey::Ed25519 {
                key: keys.public_key.to_bytes(),
            },
        };
        let reply = Arc::new(reply);

        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.unwrap();
                        let reply = reply.clone();
                        connections.spawn(async move {
                            let adnl = AdnlPeer::handle_handshake(stream, |_| Some(keys)).await.unwrap();
                            let service = tower::service_fn(move |message: Message| {
                                let reply = reply.clone();
                                async move {
                                    let Message::Query { query_id, query } = message else {
                                        return Err(LiteError::UnexpectedMessage);
                                    };
                                    let query: LiteQueryRaw = tl_proto::deserialize(&query).unwrap();
                                    tokio::time::sleep(delay).await;
                                    Ok(Message::Answer { query_id, answer: reply(query.data) })
                                }
                            });
                            let _ = tokio_tower::multiplex::Server::new(LitePeer::new(adnl), service).await;
                        });
                    }
                    Some(result) = connections.join_next() => result.unwrap(),
                }
            }
        });

        Self { config, task }
    }

    async fn client(&self) -> LiteClient {
        LiteClient::connect_liteserver(&self.config, LiteClientOptions::default())
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn raw_queries_preserve_unknown_constructors() {
    let server = TestServer::start(|request| {
        assert_eq!(request, [1, 2, 3, 4]);
        vec![9, 8, 7]
    })
    .await;
    let mut client = server.client().await;

    let answer = client.query_raw([1, 2, 3, 4]).await.unwrap();

    assert_eq!(answer, [9, 8, 7]);
}

#[tokio::test]
async fn typed_queries_decode_the_expected_response() {
    let server = TestServer::start(|request| {
        assert_eq!(
            tl_proto::deserialize::<Request>(&request).unwrap(),
            Request::GetTime
        );
        tl_proto::serialize(Response::CurrentTime(CurrentTime { now: 123 }))
    })
    .await;
    let mut client = server.client().await;

    let answer: CurrentTime = client.query_typed(Request::GetTime).await.unwrap();

    assert_eq!(answer.now, 123);
}

#[tokio::test]
async fn typed_queries_preserve_server_error_codes() {
    let server = TestServer::start(|_| {
        tl_proto::serialize(Response::Error(ServerError {
            code: 651,
            message: "block not found".into(),
        }))
    })
    .await;
    let mut client = server.client().await;

    let error = client
        .query_typed::<CurrentTime>(Request::GetTime)
        .await
        .unwrap_err();

    let LiteError::ServerError(error) = error else {
        panic!("expected a server error")
    };
    assert_eq!(error.code, 651);
    assert_eq!(error.message.to_string(), "block not found");
}

#[tokio::test]
async fn typed_queries_reject_trailing_bytes() {
    let server = TestServer::start(|_| {
        let mut answer = tl_proto::serialize(Response::CurrentTime(CurrentTime { now: 123 }));
        answer.extend_from_slice(&[0; 4]);
        answer
    })
    .await;
    let mut client = server.client().await;

    let error = client
        .query_typed::<CurrentTime>(Request::GetTime)
        .await
        .unwrap_err();

    assert!(matches!(error, LiteError::TlError(TlError::InvalidData)));
}

#[tokio::test]
async fn typed_queries_reject_an_unexpected_response() {
    let server = TestServer::start(|_| {
        tl_proto::serialize(Response::SendMsgStatus(
            rston::tl::response::SendMsgStatus { status: 1 },
        ))
    })
    .await;
    let mut client = server.client().await;

    let error = client
        .query_typed::<CurrentTime>(Request::GetTime)
        .await
        .unwrap_err();

    assert!(matches!(error, LiteError::UnexpectedMessage));
}

#[tokio::test]
async fn sequence_wait_is_consumed_by_one_query() {
    let (requests, mut received) = tokio::sync::mpsc::unbounded_channel();
    let server = TestServer::start(move |request| {
        requests.send(request).unwrap();
        tl_proto::serialize(Response::CurrentTime(CurrentTime { now: 123 }))
    })
    .await;
    let mut client = server.client().await;
    client.wait_masterchain_seqno(42, Duration::from_secs(2));

    client
        .query_typed::<CurrentTime>(Request::GetTime)
        .await
        .unwrap();
    client
        .query_typed::<CurrentTime>(Request::GetTime)
        .await
        .unwrap();

    let first = received.recv().await.unwrap();
    let mut remaining = first.as_slice();
    let wait = <WaitMasterchainSeqno as tl_proto::TlRead>::read_from(&mut remaining).unwrap();
    assert_eq!(wait.seqno, 42);
    assert_eq!(wait.timeout_ms, 2000);
    assert_eq!(
        tl_proto::deserialize::<Request>(remaining).unwrap(),
        Request::GetTime
    );
    assert_eq!(
        received.recv().await.unwrap(),
        tl_proto::serialize(Request::GetTime)
    );

    // The typed server decoder must consume both the bytes wrapper and wait prefix.
    let wrapped = tl_proto::serialize(LiteQueryRaw { data: first });
    let mut remaining = wrapped.as_slice();
    let parsed = <LiteQuery as tl_proto::TlRead>::read_from(&mut remaining).unwrap();
    assert_eq!(parsed.wrapped_request.wait_masterchain_seqno, Some(wait));
    assert_eq!(parsed.wrapped_request.request, Request::GetTime);
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn request_deadline_expires_without_retrying() {
    let (requests, mut received) = tokio::sync::mpsc::unbounded_channel();
    let server = TestServer::with_delay(Duration::from_millis(100), move |request| {
        requests.send(request).unwrap();
        vec![7]
    })
    .await;
    let mut client = server.client().await;
    client.set_request_timeout(Duration::from_millis(10));

    let error = client.query_raw([1, 2, 3, 4]).await.unwrap_err();

    assert!(matches!(
        error,
        LiteError::Timeout {
            operation: "lite_query",
            ..
        }
    ));
    let request = tokio::time::timeout(Duration::from_secs(1), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request, [1, 2, 3, 4]);
    assert!(matches!(
        received.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));

    // A late answer to the cancelled query must not become the next query's answer.
    client.set_request_timeout(Duration::from_secs(1));
    assert_eq!(client.query_raw([5, 6, 7, 8]).await.unwrap(), [7]);
    assert_eq!(received.recv().await.unwrap(), [5, 6, 7, 8]);
}

#[tokio::test]
async fn pool_rotates_between_servers_and_reconnects_after_disconnect() {
    let first = TestServer::start(|_| vec![1]).await;
    let second = TestServer::start(|_| vec![2]).await;
    let config = ConfigGlobal {
        liteservers: vec![first.config.clone(), second.config.clone()],
    };
    let mut pool = LiteClientPool::new(config, LiteClientOptions::default()).unwrap();

    assert_eq!(pool.query_raw([0; 4]).await.unwrap(), [1]);
    assert_eq!(pool.query_raw([0; 4]).await.unwrap(), [2]);
    assert_eq!(pool.query_raw([0; 4]).await.unwrap(), [1]);

    pool.disconnect();
    assert_eq!(pool.query_raw([0; 4]).await.unwrap(), [2]);
}

#[tokio::test]
async fn pool_skips_connection_failures_before_sending() {
    let unavailable = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unavailable_port = unavailable.local_addr().unwrap().port();
    drop(unavailable);
    let server = TestServer::start(|_| vec![7]).await;
    let mut failed = server.config.clone();
    failed.port = unavailable_port;
    let config = ConfigGlobal {
        liteservers: vec![failed, server.config.clone()],
    };
    let mut pool = LiteClientPool::new(config, LiteClientOptions::default()).unwrap();

    assert_eq!(pool.query_raw([0; 4]).await.unwrap(), [7]);
}

#[test]
fn pool_rejects_an_empty_server_list() {
    let result = LiteClientPool::new(
        ConfigGlobal {
            liteservers: vec![],
        },
        LiteClientOptions::default(),
    );

    assert!(matches!(result, Err(LiteError::NoServers)));
}

#[test]
fn network_config_reads_signed_ips_and_base64_identities() {
    let keys = KeyPair::from(&SecretKey::from_bytes([7; 32]));
    let encoded = base64::engine::general_purpose::STANDARD.encode(keys.public_key.as_bytes());
    let json = format!(
        r#"{{"liteservers":[{{"ip":-1062731519,"port":1234,"id":{{"@type":"pub.ed25519","key":"{encoded}"}}}}],"validator":{{}}}}"#
    );

    let config: ConfigGlobal = json.parse().unwrap();
    let server = &config.liteservers[0];

    assert_eq!(server.socket_addr().to_string(), "192.168.1.1:1234");
    assert_eq!(server.public_key(), keys.public_key.as_bytes());
    let round_trip: ConfigGlobal =
        serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
    assert_eq!(round_trip.liteservers[0].public_key(), server.public_key());
}

#[test]
fn network_config_rejects_invalid_identity_lengths_and_ports() {
    let short_key = base64::engine::general_purpose::STANDARD.encode([7; 31]);
    let json = format!(
        r#"{{"liteservers":[{{"ip":0,"port":1234,"id":{{"@type":"pub.ed25519","key":"{short_key}"}}}}]}}"#
    );
    assert!(json.parse::<ConfigGlobal>().is_err());

    let key = KeyPair::from(&SecretKey::from_bytes([7; 32]));
    let encoded = base64::engine::general_purpose::STANDARD.encode(key.public_key.as_bytes());
    let json = format!(
        r#"{{"liteservers":[{{"ip":0,"port":0,"id":{{"@type":"pub.ed25519","key":"{encoded}"}}}}]}}"#
    );
    assert!(json.parse::<ConfigGlobal>().is_err());
}
