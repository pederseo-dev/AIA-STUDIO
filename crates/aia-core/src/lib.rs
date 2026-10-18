//! Núcleo de AIA Studio: modelo del documento, comandos y undo/redo.
//!
//! No depende de UI ni de GPU. Todo cambio al documento pasa por un [`Command`],
//! tanto si viene del usuario como (más adelante) de un agente de IA.

pub mod command;
pub mod document;
pub mod editor;
pub mod mode;
pub mod path;
pub mod project;

pub use command::{Command, CommandError};
pub use document::{Color, Document, Layer, LayerId, Shape, ShapeId, StrokeStyle, Style};
pub use editor::Editor;
pub use mode::{Agent, Mode};
pub use path::{Handle, Node, NodeKind, VectorPath};
pub use project::{ChatMessage, ChatRole, Chats};

/// Re-export para que los demás crates usen la misma versión de kurbo.
pub use kurbo;
