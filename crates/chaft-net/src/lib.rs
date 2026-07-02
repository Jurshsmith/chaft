use async_trait::async_trait;
use chaft_types::{EventId, SignedEvent, WorkspaceId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("transport unavailable: {0}")]
    Unavailable(&'static str),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("protocol error: {0}")]
    Protocol(String),
}

impl From<std::io::Error> for NetError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerAddress {
    pub peer_id: PeerId,
    pub endpoint: String,
}

#[async_trait]
pub trait ChaftTransport: Send + Sync {
    async fn connect(&self, peer: PeerAddress) -> Result<(), NetError>;
    async fn fetch_inventory(&self, peer: &PeerAddress) -> Result<Vec<EventId>, NetError>;
    async fn fetch_workspace_inventory(
        &self,
        peer: &PeerAddress,
        _workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, NetError> {
        self.fetch_inventory(peer).await
    }
    async fn publish_event(&self, peer: &PeerAddress, event: SignedEvent) -> Result<(), NetError>;
    async fn fetch_events(
        &self,
        peer: &PeerAddress,
        event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError>;
}
