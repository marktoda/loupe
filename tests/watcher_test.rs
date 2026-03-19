use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn discovers_existing_jsonl_files() {
    let dir = TempDir::new().unwrap();
    let f = dir.path().join("run1.jsonl");
    std::fs::write(&f, concat!(
        r#"{"type":"system","subtype":"init","model":"test","tools":[],"session_id":"s1","uuid":"u1"}"#,
        "\n"
    )).unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let cancel = tokio_util::sync::CancellationToken::new();
    let cancel_clone = cancel.clone();

    tokio::spawn(async move {
        loupe::watcher::run_watcher(dir.path().to_path_buf(), tx, cancel_clone).await.unwrap();
    });

    // Should discover the existing file
    let event = timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    assert!(matches!(event, loupe::events::AppEvent::RunDiscovered { .. }));

    // Should get RunUpdated with parsed items
    let event = timeout(Duration::from_secs(2), rx.recv()).await.unwrap().unwrap();
    assert!(matches!(event, loupe::events::AppEvent::RunUpdated { .. }));

    cancel.cancel();
}
