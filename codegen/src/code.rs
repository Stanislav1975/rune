use anyhow::Error;
use quote::{ToTokens, quote};

/// Generate the Rune's `lib.rs` file.
pub fn generate() -> Result<String, Error> {
    let preamble = generate_preamble();
    let manifest = generate_manifest();
    let call = generate_call();

    let tokens = quote! {
        #preamble
        #manifest
        #call
    };

    Ok(tokens.to_token_stream().to_string())
}

fn generate_manifest() -> impl ToTokens {
    quote! {
        #[no_mangle]
        pub extern "C" fn _manifest() -> u32 {
            let _setup = SetupGuard::default();

            let pipeline = move || {
                let _guard = PipelineGuard::default();
            };

            unsafe {
                PIPELINE = Some(Box::new(pipeline));
            }

            1
        }
    }
}

fn generate_preamble() -> impl ToTokens {
    quote! {
        //! Automatically generated by rune. DO NOT EDIT!

        #![no_std]
        #![feature(alloc_error_handler)]
        #![allow(unused_imports, dead_code)]

        extern crate alloc;

        use runic_types::{*, wasm32::*};
        use alloc::boxed::Box;

        static mut PIPELINE: Option<Box<dyn FnMut()>> = None;
    }
}

fn generate_call() -> impl ToTokens {
    quote! {
        #[no_mangle]
        pub extern "C" fn _call(
            _capability_type: i32,
            _input_type: i32,
            _capability_idx: i32,
        ) -> i32 {
            unsafe {
                let pipeline = PIPELINE.as_mut()
                    .expect("The rune hasn't been initialized");
                pipeline();

                0
            }
        }
    }
}
