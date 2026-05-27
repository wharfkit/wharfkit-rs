use axum::{body::Bytes, extract::Path, http::StatusCode, routing::post, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub type PostLog = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
pub type PostBroadcast = broadcast::Sender<(String, Vec<u8>)>;

pub struct MockBuoyServer {
    pub addr: SocketAddr,
    pub posts: PostLog,
    pub broadcaster: PostBroadcast,
    shutdown: CancellationToken,
    handle: Option<JoinHandle<()>>,
}

impl MockBuoyServer {
    pub async fn start() -> Self {
        let (tx, _) = broadcast::channel(64);
        let tx_clone = tx.clone();
        let posts = Arc::new(Mutex::new(Vec::new()));
        let posts_clone = posts.clone();
        let app = Router::new().route(
            "/:uuid",
            post(move |Path(uuid): Path<String>, body: Bytes| {
                let tx = tx_clone.clone();
                let posts = posts_clone.clone();
                async move {
                    posts.lock().await.push((uuid.clone(), body.to_vec()));
                    let _ = tx.send((uuid, body.to_vec()));
                    StatusCode::OK
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let shutdown = CancellationToken::new();
        let shutdown_signal = shutdown.clone();
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async move { shutdown_signal.cancelled().await })
                .await;
        });
        Self {
            addr,
            posts,
            broadcaster: tx,
            shutdown,
            handle: Some(handle),
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for MockBuoyServer {
    fn drop(&mut self) {
        self.shutdown.cancel();
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}
