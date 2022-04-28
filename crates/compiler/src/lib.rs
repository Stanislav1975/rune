//! The Rune compiler.
//!
//! # Phases
//!
//! The compilation process is split into several phases which build on each
//! other, with user-injectable [`hooks`] called after each phase finishes.
//!
//! The phases are:
//!
//! 1. [`parse`]
//! 2. [`lowering`]
//! 3. [`type_check`]
//! 4. [`codegen`]
//!
//! # Stability
//!
//! This crate contains the internal types used by the Rune compiler so they can
//! be used externally. While this can give you a lot of flexibility and let you
//! extract a lot of information about a Rune, the compiler is a continually
//! evolving codebase.
//!
//! **This API should be considered unstable and subject to change.**

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;

#[cfg(test)]
#[macro_use]
mod macros;

mod build_context;
pub mod codegen;
pub mod compile;
mod diagnostics;
pub mod hooks;
pub mod lowering;
pub mod parse;
mod phases;
pub mod serialize;
mod toolchain;
pub mod type_check;
mod inputs;

pub use crate::{
    build_context::{BuildContext, FeatureFlags, Verbosity},
    diagnostics::Diagnostics,
    phases::{build, build_with_hooks, Phase},
    toolchain::rust_toolchain,
};
