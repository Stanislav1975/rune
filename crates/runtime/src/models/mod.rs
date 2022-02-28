//! Functions for handling various "well-known" model formats.

#[cfg(feature = "tflite")]
mod tflite;

use crate::callbacks::{ModelMetadata, Model};

#[cfg(feature = "tflite")]
pub use self::tflite::load_tflite;

use anyhow::Error;
pub use hotg_rune_core::{TFJS_MIMETYPE, TF_MIMETYPE, TFLITE_MIMETYPE};

pub fn default_model_handler(
    _id: u32,
    meta: &ModelMetadata<'_>,
    model: &[u8],
) -> Result<Box<dyn Model>, Error> {
    let ModelMetadata {
        mimetype,
        inputs,
        outputs,
        ..
    } = *meta;

    match mimetype {
        #[cfg(feature = "tflite")]
        TFLITE_MIMETYPE => load_tflite(model, inputs, outputs),
        _ => Err(Error::msg("Unsupported model format")),
    }
}
