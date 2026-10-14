use super::*;

#[tokio::test]
async fn rejects_config_without_liteservers() {
    let config = r#"{"liteservers":[]}"#.parse::<GlobalConfig>().unwrap();
    let error = LiteClient::connect(&config).await.err().unwrap();
    assert!(matches!(error, SourceError::GlobalConfig(_)));
}
