//! El archivo de proyecto `.aia`: el documento más los datos que viajan con él
//! pero no son parte del dibujo, como las conversaciones con cada agente.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::mode::Agent;

/// Versión del formato de archivo `.aia`.
pub const FORMAT_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
}

/// Historial de chat por agente. No pasa por undo/redo: deshacer un dibujo no
/// borra lo que se habló.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chats {
    conversations: BTreeMap<Agent, Vec<ChatMessage>>,
}

impl Chats {
    pub fn messages(&self, agent: Agent) -> &[ChatMessage] {
        self.conversations.get(&agent).map_or(&[], Vec::as_slice)
    }

    pub fn push(&mut self, agent: Agent, message: ChatMessage) {
        self.conversations.entry(agent).or_default().push(message);
    }

    pub fn clear(&mut self, agent: Agent) {
        self.conversations.remove(&agent);
    }
}

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format_version: u32,
    document: Document,
    #[serde(default)]
    chats: Chats,
}

pub fn to_json(document: &Document, chats: &Chats) -> serde_json::Result<String> {
    #[derive(Serialize)]
    struct ProjectFileRef<'a> {
        format_version: u32,
        document: &'a Document,
        chats: &'a Chats,
    }
    serde_json::to_string_pretty(&ProjectFileRef {
        format_version: FORMAT_VERSION,
        document,
        chats,
    })
}

pub fn from_json(json: &str) -> serde_json::Result<(Document, Chats)> {
    let file: ProjectFile = serde_json::from_str(json)?;
    Ok((file.document, file.chats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_roundtrip_keeps_chats_per_agent() {
        let doc = Document::default();
        let mut chats = Chats::default();
        chats.push(
            Agent::Rigging,
            ChatMessage {
                role: ChatRole::User,
                text: "agregá un hueso al brazo".into(),
            },
        );
        let json = to_json(&doc, &chats).unwrap();
        let (doc2, chats2) = from_json(&json).unwrap();
        assert_eq!(doc2, doc);
        assert_eq!(chats2.messages(Agent::Rigging).len(), 1);
        assert!(chats2.messages(Agent::Draw).is_empty());
    }
}
