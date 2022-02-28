//! The Rune Runtime.
//!
//! # Feature Flags
//!
//! This crate has the following cargo feature flags:
//!
//! - `builtins` - enable various builtin outputs and capabilities (on by
//!   default)
//! - `tflite` - enable support for TensorFlow Lite models (on by default)
//! - `wasm3` - enable the [WASM3](https://github.com/wasm3/wasm3) engine
//! - `wasmer` - enable the [wasmer](https://wasmer.io/) engine

#![cfg_attr(feature = "unstable_doc_cfg", feature(doc_cfg))]

mod callbacks;
mod engine;
pub mod models;
mod runtime;
mod tensor;

#[cfg(feature = "builtins")]
pub mod builtins;

pub use crate::{
    callbacks::{NodeMetadata, ModelMetadata, Model},
    runtime::Runtime,
    tensor::{Tensor, ElementType, TensorElement},
};
