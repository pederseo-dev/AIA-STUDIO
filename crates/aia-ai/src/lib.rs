//! Base de la IA de AIA Studio.
//!
//! Por ahora solo define la forma de la configuración: proveedores, modelos y
//! skills asignadas a cada modo. El chat y la ejecución de herramientas
//! (que serán los [`aia_core::Command`] expuestos como tools) llegan después.

pub mod config;

pub use config::{AiSettings, ModeAiConfig, ModelRef, ProviderConfig, ProviderKind, SkillRef};
