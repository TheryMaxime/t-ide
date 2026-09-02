//! T-ide desktop backend core.
//!
//! The crate is split into the layers described in `plan.md`:
//! agent engine, storage, local network server, and security primitives.

pub mod agent;
pub mod config;
pub mod error;
pub mod network;
pub mod security;
pub mod storage;

#[cfg(test)]
mod test_support;

pub use error::{Error, Result};
