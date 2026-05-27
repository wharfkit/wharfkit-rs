use futures_util::StreamExt;
use uuid::Uuid;
use wharfkit_buoy_client::{BuoyClient, PostOptions};

#[tokio::test]
#[ignore = "network: live cb.anchor.link"]
async fn live_roundtrip_cb_anchor() {
    let client = BuoyClient::new("https://cb.anchor.link");
    let uuid = Uuid::new_v4();
    let listening = client.channel(uuid);

    let stream = listening
        .listen()
        .await
        .expect("listen against cb.anchor.link");
    tokio::pin!(stream);

    let posting = {
        let posting = client.channel(uuid);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            posting
                .post(b"live-test-payload", PostOptions::default())
                .await
        })
    };

    let received = tokio::time::timeout(std::time::Duration::from_secs(10), stream.next())
        .await
        .expect("timeout waiting on ws delivery")
        .expect("stream ended")
        .expect("ws err");

    assert_eq!(received, b"live-test-payload");
    posting.await.unwrap().unwrap();
}
