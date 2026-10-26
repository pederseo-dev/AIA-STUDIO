//! AIA Studio core: document model (Moho style), commands and undo/redo.
//!
//! No UI or GPU dependencies. Every change to the document goes through a [`Command`],
//! whether it comes from the user or from an AI agent.

pub mod command;
pub mod document;
pub mod editor;
pub mod mesh;
pub mod mode;
pub mod project;
pub mod style;
mod topology;

pub use command::{Command, CommandError};
pub use document::{Document, Layer, LayerId, LayerKind, LayerTransform};
pub use editor::Editor;
pub use mesh::{Curve, CurveId, CurveNode, DEFAULT_CURVATURE, EdgeRef, Handle, HandleMode, Loop, Mesh, MeshPoint, PointId, Shape, ShapeId};
pub use mode::{Agent, Mode};
pub use project::{ChatMessage, ChatRole, Chats, RawContent};
pub use style::{Color, StrokeStyle, Style};

/// Re-exported so the other crates use the same kurbo version.
pub use kurbo;
