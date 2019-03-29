//! Flow control instructions.

use super::State;
use crate::{Memory, Patch};
use bigint::{M256, U256};

pub fn sload<M: Memory, P: Patch>(state: &mut State<M, P>) {
    pop!(state, index: U256);
    let value = state.account_state.storage_read(state.context.address, index).unwrap();
    push!(state, value);
}

pub fn sstore<M: Memory, P: Patch>(state: &mut State<M, P>) {
    pop!(state, index: U256, value: M256);
    state
        .account_state
        .storage_write(state.context.address, index, value)
        .unwrap();
}

pub fn mload<M: Memory, P: Patch>(state: &mut State<M, P>) {
    pop!(state, index: U256);
    let value = state.memory.read(index);
    push!(state, value);
}

pub fn mstore<M: Memory, P: Patch>(state: &mut State<M, P>) {
    pop!(state, index: U256, value: M256);
    state.memory.write(index, value).unwrap();
}

pub fn mstore8<M: Memory, P: Patch>(state: &mut State<M, P>) {
    pop!(state, index: U256, value: M256);
    state.memory.write_raw(index, (value.0.low_u32() & 0xFF) as u8).unwrap();
}
