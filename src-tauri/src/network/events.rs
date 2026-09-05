//! Fan-out of agent output to every connected client (T029).
//!
//! The agent engine publishes protocol frames here; each WebSocket connection
//! subscribes and forwards them, so the desktop IDE and every paired device see
//! the same live transcript.

use tokio::sync::broadcast;

use crate::network::protocol::ServerMessage;

/// Number of frames buffered per subscriber before the slowest client starts
/// missing entries; missed entries are recovered with `transcript.catchup`.
const CAPACITY: usize = 1024;

/// Broadcast channel shared by the agent engine and the WebSocket server.
#[derive(Clone, Debug)]
pub struct SessionEvents {
    sender: broadcast::Sender<ServerMessage>,
}

impl SessionEvents {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }

    /// Subscribe to every frame published from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.sender.subscribe()
    }

    /// Publish a frame. Frames sent while nobody is connected are dropped.
    pub fn publish(&self, message: ServerMessage) {
        let _ = self.sender.send(message);
    }
}

impl Default for SessionEvents {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn every_subscriber_receives_published_frames() {
        let events = SessionEvents::new();
        let mut first = events.subscribe();
        let mut second = events.subscribe();

        events.publish(ServerMessage::TranscriptComplete { session_id: 7 });

        for receiver in [&mut first, &mut second] {
            assert_eq!(
                receiver.recv().await.unwrap(),
                ServerMessage::TranscriptComplete { session_id: 7 }
            );
        }
    }

    #[tokio::test]
    async fn publishing_without_subscribers_is_not_an_error() {
        let events = SessionEvents::new();
        events.publish(ServerMessage::TranscriptComplete { session_id: 1 });
    }
}
