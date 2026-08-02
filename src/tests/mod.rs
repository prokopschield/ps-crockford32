//! Test suites.
//!
//! [`allocation_free`] exercises only the API that remains available with
//! the `alloc` feature disabled, so `cargo test --no-default-features`
//! covers the allocation-free surface. [`owned`] covers everything else.

mod allocation_free;

#[cfg(feature = "alloc")]
mod owned;
