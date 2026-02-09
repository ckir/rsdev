//! # Core Module
//! 
//! This module aggregates the heart of the ReStream engine.
//! It handles memory guarding, symbol registration, and the 
//! primary distribution dispatcher.

pub mod registry;
pub mod memory_guard;
pub mod dispatcher;
pub mod upstream_manager;

// Re-exporting for workspace-wide access via lib_common::core::*
pub use registry::Registry;
pub use memory_guard::{GlobalMemoryGuard, ClientPriority};
pub use dispatcher::{Dispatcher, ReStreamFrame};
pub use upstream_manager::{UpstreamManager, OperationMode};
