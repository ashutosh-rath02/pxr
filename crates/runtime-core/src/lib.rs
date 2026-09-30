//! Deterministic, allocation-free execution boundary. All time is controller-local milliseconds.
#![no_std]
#![forbid(unsafe_code)]

mod model;
pub mod profile;
mod runtime;

pub use model::*;
pub use runtime::Runtime;
