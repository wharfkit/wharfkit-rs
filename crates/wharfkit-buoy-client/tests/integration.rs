use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use std::net::SocketAddr;
use tokio::sync::broadcast;
use uuid::Uuid;
use wharfkit_buoy_client::{BuoyClient, DeliveryStatus, PostOptions};

async fn start_mock_server() -> (SocketAddr, broadcast::Sender<(String, Vec<u8>)>) {
    let (tx, _) = broadcast::channel::<(String, Vec<u8>)>(16);
    let tx_for_route = tx.clone();
    let app = Router::new().route(
        "/:uuid",
        post(move |Path(uuid): Path<String>, body: axum::body::Bytes| {
            let tx = tx_for_route.clone();
            async move {
                let _ = tx.send((uuid, body.to_vec()));
                StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, tx)
}

#[tokio::test]
async fn buoy_client_post_succeeds_against_mock() {
    let (addr, bcast) = start_mock_server().await;
    let mut subscriber = bcast.subscribe();
    drop(bcast);

    let client = BuoyClient::new(format!("http://{addr}"));
    let uuid = Uuid::new_v4();
    let channel = client.channel(uuid);

    let status = channel
        .post(b"hello", PostOptions::default())
        .await
        .unwrap();

    assert_eq!(status, DeliveryStatus::Delivered);

    let (received_uuid, received_bytes) =
        tokio::time::timeout(std::time::Duration::from_secs(2), subscriber.recv())
            .await
            .expect("broadcast timed out")
            .unwrap();
    assert_eq!(received_uuid, uuid.to_string());
    assert_eq!(received_bytes, b"hello");
}
