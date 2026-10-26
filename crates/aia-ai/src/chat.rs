//! A chat request running on a worker thread, so the interface never freezes.

use std::sync::mpsc::{Receiver, channel};

use aia_core::ChatMessage;

use crate::config::ProviderConfig;
use crate::provider::{self, Reply, Request};

pub enum ChatEvent {
    /// A piece of the answer, as it arrives.
    Text(String),
    /// What is happening (for example, waiting because of a rate limit).
    Status(String),
    Finished(Reply),
    Failed(String),
}

/// Request in progress. Dropping it stops listening (the thread ends when the answer does).
pub struct ChatJob {
    events: Receiver<ChatEvent>,
}

impl ChatJob {
    /// Starts the request. `wake` is called on every event (for example to repaint the UI).
    pub fn start(
        provider: ProviderConfig,
        key: Option<String>,
        model: String,
        system: String,
        messages: Vec<ChatMessage>,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (sender, events) = channel();
        std::thread::spawn(move || {
            let request = Request {
                model: &model,
                system: &system,
                messages: &messages,
            };
            let mut on_text = |text: &str| {
                let _ = sender.send(ChatEvent::Text(text.to_string()));
                wake();
            };
            let mut on_status = |status: &str| {
                let _ = sender.send(ChatEvent::Status(status.to_string()));
                wake();
            };
            let result = provider::stream(&provider, key.as_deref(), &request, &mut on_text, &mut on_status);
            let _ = sender.send(match result {
                Ok(reply) => ChatEvent::Finished(reply),
                Err(error) => ChatEvent::Failed(error),
            });
            wake();
        });
        Self { events }
    }

    /// Events received since the last call.
    pub fn poll(&self) -> Vec<ChatEvent> {
        self.events.try_iter().collect()
    }
}
