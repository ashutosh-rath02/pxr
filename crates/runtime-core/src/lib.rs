//! Deterministic, allocation-free execution boundary. All time is controller-local milliseconds.
#![no_std]
#![cfg_attr(not(kani), forbid(unsafe_code))]

mod model;
pub mod profile;
#[cfg(kani)]
mod proofs;
mod runtime;

pub use model::*;
pub use runtime::Runtime;
