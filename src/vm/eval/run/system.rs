//! System operations instructions

use util::address::Address;
use util::bigint::{U256, M256, H256};
use util::gas::Gas;
use vm::{Memory, Log, ValidTransaction};
use block::TransactionAction;
use super::{Control, State};

use std::cmp::min;
use sha3::{Digest, Keccak256};
use vm::eval::util::{l64, copy_from_memory};

pub fn suicide<M: Memory + Default>(state: &mut State<M>) {
    pop!(state, address: Address);
    let balance = state.account_state.balance(state.context.address).unwrap();
    if !state.removed.contains(&state.context.address) {
        state.removed.push(state.context.address);
    }
    state.account_state.increase_balance(address, balance);

    let balance = state.account_state.balance(state.context.address).unwrap();
    state.account_state.decrease_balance(state.context.address, balance);
}

pub fn log<M: Memory + Default>(state: &mut State<M>, topic_len: usize) {
    pop!(state, index: U256, len: U256);
    let data = copy_from_memory(&state.memory, index, len);
    let mut topics = Vec::new();
    for _ in 0..topic_len {
        topics.push(H256::from(state.stack.pop().unwrap()));
    }

    state.logs.push(Log {
        address: state.context.address,
        data: data,
        topics: topics,
    });
}

pub fn sha3<M: Memory + Default>(state: &mut State<M>) {
    pop!(state, from: U256, len: U256);
    let data = copy_from_memory(&state.memory, from, len);
    let ret = Keccak256::digest(data.as_slice());
    push!(state, M256::from(ret.as_slice()));
}

macro_rules! try_callstack_limit {
    ( $state:expr ) => {
        if $state.depth > $state.patch.callstack_limit {
            push!($state, M256::zero());
            return None;
        }
    }
}

macro_rules! try_balance {
    ( $state:expr, $value:expr, $gas:expr ) => {
        if $state.account_state.balance($state.context.address).unwrap() < $value {
            push!($state, M256::zero());
            return None;
        }
    }
}

pub fn create<M: Memory + Default>(state: &mut State<M>, after_gas: Gas) -> Option<Control> {
    let l64_after_gas = if state.patch.call_create_l64_after_gas { l64(after_gas) } else { after_gas };

    pop!(state, value: U256);
    pop!(state, init_start: U256, init_len: U256);

    try_callstack_limit!(state);
    try_balance!(state, value, Gas::zero());

    let init = copy_from_memory(&state.memory, init_start, init_len);
    let transaction = ValidTransaction {
        caller: state.context.address,
        gas_price: state.context.gas_price,
        gas_limit: l64_after_gas,
        value: value,
        input: init,
        action: TransactionAction::Create,
    };
    let context = transaction.into_context(
        Gas::zero(), Some(state.context.origin), &mut state.account_state, true
    ).unwrap();

    push!(state, context.address.into());
    Some(Control::InvokeCreate(context))
}

pub fn call<M: Memory + Default>(state: &mut State<M>, stipend_gas: Gas, after_gas: Gas, as_self: bool) -> Option<Control> {
    let l64_after_gas = if state.patch.call_create_l64_after_gas { l64(after_gas) } else { after_gas };

    pop!(state, gas: Gas, to: Address, value: U256);
    pop!(state, in_start: U256, in_len: U256, out_start: U256, out_len: U256);
    let gas_limit = min(gas, l64_after_gas) + stipend_gas;

    try_callstack_limit!(state);
    try_balance!(state, value, gas_limit);

    let input = copy_from_memory(&state.memory, in_start, in_len);
    let transaction = ValidTransaction {
        caller: state.context.address,
        gas_price: state.context.gas_price,
        gas_limit: gas_limit,
        value: value,
        input: input,
        action: TransactionAction::Call(to),
    };

    let mut context = transaction.into_context(
        Gas::zero(), Some(state.context.origin), &mut state.account_state, true
    ).unwrap();
    if as_self {
        context.address = state.context.address;
    }

    push!(state, M256::from(1u64));
    Some(Control::InvokeCall(context, (out_start, out_len)))
}

pub fn delegate_call<M: Memory + Default>(state: &mut State<M>, after_gas: Gas) -> Option<Control> {
    let l64_after_gas = if state.patch.call_create_l64_after_gas { l64(after_gas) } else { after_gas };

    pop!(state, gas: Gas, to: Address);
    pop!(state, in_start: U256, in_len: U256, out_start: U256, out_len: U256);
    let gas_limit = min(gas, l64_after_gas);

    try_callstack_limit!(state);

    let input = copy_from_memory(&state.memory, in_start, in_len);
    let transaction = ValidTransaction {
        caller: state.context.caller,
        gas_price: state.context.gas_price,
        gas_limit: gas_limit,
        value: state.context.value,
        input: input,
        action: TransactionAction::Call(to),
    };

    let mut context = transaction.into_context(
        Gas::zero(), Some(state.context.origin), &mut state.account_state, true
    ).unwrap();
    context.value = U256::zero();
    context.address = state.context.address;

    push!(state, M256::from(1u64));
    Some(Control::InvokeCall(context, (out_start, out_len)))
}
