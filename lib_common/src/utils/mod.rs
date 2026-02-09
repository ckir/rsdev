//! # Utilities Module
//!
//! This module serves as a collection point for various general-purpose utility
//! functions and helper modules that are widely applicable across the `lib_common`
//! crate and the broader `rsdev` project.
//!
//! ## Purpose:
//! The goal is to consolidate common, reusable logic that doesn't fit into more
//! specific modules (like `core` or `markets`). This promotes code reuse and
//! helps maintain a cleaner structure for specialized components.
//!
//! ## Contained Modules:
//!
//! - **`misc`**: A submodule for miscellaneous functions, including system
//!   information retrieval (`sys_info`) and general helper functions (`utils`).
//!
//! This module aims to prevent code duplication and provide a well-organized
//! home for functions that support various aspects of the application.

#![doc(html_logo_url = "https://example.com/logo.png")] // Placeholder
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

/// Miscellaneous utility functions, including system information and general helpers.
pub mod misc;