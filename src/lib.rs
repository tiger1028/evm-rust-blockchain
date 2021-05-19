//! Ethereum Virtual Machine implementation in Rust

#![deny(warnings)]
#![forbid(unsafe_code, unused_variables)]

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use evm_core::*;
pub use evm_runtime::*;
pub use evm_gasometer as gasometer;

#[cfg(feature = "tracing")]
pub mod tracing;

#[cfg(feature = "tracing")]
macro_rules! event {
	($x:expr) => {
		use crate::tracing::Event::*;
		$x.emit();
	}
}

#[cfg(not(feature = "tracing"))]
macro_rules! event {
	($x:expr) => { }
}

pub mod executor;
pub mod backend;
