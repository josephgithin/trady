use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct ShutdownSignal {
    sender: Arc<broadcast::Sender<()>>,
}

impl ShutdownSignal {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1);
        Self {
            sender: Arc::new(sender),
        }
    }

    pub fn signal(&self) {
        let _ = self.sender.send(());
    }

    pub async fn wait(&self) {
        let mut rx = self.sender.subscribe();
        let _ = rx.recv().await;
    }
}

