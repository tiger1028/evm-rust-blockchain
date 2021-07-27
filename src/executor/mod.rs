//! # EVM executors
//!
//! Executors are structs that hook gasometer and the EVM core together. It
//! also handles the call stacks in EVM.

mod stack;

pub use self::stack::{
	MemoryStackState, PrecompileOutput, Precompiles, StackExecutor, StackExitKind, StackState,
	StackSubstateMetadata,
};
