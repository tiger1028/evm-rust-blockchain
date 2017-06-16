use utils::bigint::{U256, M256};
use utils::gas::Gas;
use vm::errors::{RequireError, MachineError};
use vm::commit::AccountState;
use vm::Memory;
use super::{Machine, MachineStatus};
use super::utils::copy_into_memory;
use super::cost::code_deposit_gas;

/// # Lifecycle of a Machine
///
/// When a new non-invoked transaction is created, `initialize_call`
/// or `initialize_create` should be called. After this, the machine
/// can be stepped as normal. When the machine meets a CALL/CALLCODE
/// or CREATE instruction, a sub-machine will be created. This
/// submachine should first call `invoke_call` or
/// `invoke_create`. After the submachine is finished, it should call
/// `apply_sub`. When the non-invoked transaction is finished, it
/// should first call `code_deposit` if it is a contract creation
/// transaction. After that, it should call `finalize`.

impl<M: Memory + Default> Machine<M> {
    pub fn initialize_call(&mut self, preclaimed_value: U256) {
        self.state.account_state.decrease_balance(self.state.context.caller, preclaimed_value);
        self.state.account_state.decrease_balance(self.state.context.caller, self.state.context.value);
        self.state.account_state.increase_balance(self.state.context.address, self.state.context.value);
    }

    pub fn invoke_call(&mut self) {
        self.state.account_state.decrease_balance(self.state.context.caller, self.state.context.value);
        self.state.account_state.increase_balance(self.state.context.address, self.state.context.value);
    }

    pub fn initialize_create(&mut self, preclaimed_value: U256) {
        self.state.account_state.decrease_balance(self.state.context.caller, preclaimed_value);
        self.state.account_state.decrease_balance(self.state.context.caller, self.state.context.value);
        self.state.account_state.create(self.state.context.address, self.state.context.value);
    }

    pub fn invoke_create(&mut self) {
        self.state.account_state.decrease_balance(self.state.context.caller, self.state.context.value);
        self.state.account_state.create(self.state.context.address, self.state.context.value);
    }

    pub fn code_deposit(&mut self) -> Result<(), RequireError> {
        match self.status() {
            MachineStatus::ExitedOk | MachineStatus::ExitedErr(_) => (),
            _ => panic!(),
        }

        let deposit_cost = code_deposit_gas(self.state.out.len());
        if deposit_cost > self.state.available_gas() {
            if !self.state.patch.force_code_deposit {
                self.status = MachineStatus::ExitedErr(MachineError::EmptyGas);
            } else {
                self.state.account_state.code_deposit(self.state.context.address, &[]);
            }
        } else {
            self.state.used_gas = self.state.used_gas + deposit_cost;
            self.state.account_state.code_deposit(self.state.context.address,
                                                  self.state.out.as_slice());
        }
        Ok(())
    }

    pub fn finalize(&mut self, real_used_gas: Gas, preclaimed_value: U256, fresh_account_state: &AccountState) -> Result<(), RequireError> {
        match self.status() {
            MachineStatus::ExitedOk => (),
            MachineStatus::ExitedErr(_) => {
                // If exited with error, reset all changes.
                self.state.account_state = fresh_account_state.clone();
                self.state.account_state.decrease_balance(self.state.context.caller, preclaimed_value);
                self.state.logs = Vec::new();
                self.state.removed = Vec::new();
            },
            _ => panic!(),
        }

        let gas_dec = real_used_gas * self.state.context.gas_price;
        self.state.account_state.increase_balance(self.state.context.caller, preclaimed_value);
        self.state.account_state.decrease_balance(self.state.context.caller, gas_dec.into());

        match self.status() {
            MachineStatus::ExitedOk => Ok(()),
            MachineStatus::ExitedErr(_) => Ok(()),
            _ => panic!(),
        }
    }

    #[allow(unused_variables)]
    /// Apply a sub runtime into the current runtime. This sub runtime
    /// should have been created by the current runtime's `derive`
    /// function. Depending whether the current runtime is invoking a
    /// ContractCreation or MessageCall instruction, it will apply
    /// various states back.
    pub fn apply_sub(&mut self, sub: Machine<M>) {
        use std::mem::swap;
        let mut status = MachineStatus::Running;
        swap(&mut status, &mut self.status);
        match status {
            MachineStatus::InvokeCreate(_) => {
                self.apply_create(sub);
            },
            MachineStatus::InvokeCall(_, (out_start, out_len)) => {
                self.apply_call(sub, out_start, out_len);
            },
            _ => panic!(),
        }
    }

    fn apply_create(&mut self, sub: Machine<M>) {
        if self.state.available_gas() < sub.state.used_gas {
            panic!();
        }

        match sub.status() {
            MachineStatus::ExitedOk => {
                let sub_total_used_gas = sub.state.total_used_gas();

                self.state.account_state = sub.state.account_state;
                self.state.blockhash_state = sub.state.blockhash_state;
                self.state.logs = sub.state.logs;
                self.state.removed = sub.state.removed;
                self.state.used_gas = self.state.used_gas + sub_total_used_gas;
                self.state.refunded_gas = self.state.refunded_gas + sub.state.refunded_gas;
                if self.state.available_gas() >= code_deposit_gas(sub.state.out.len()) {
                    self.state.used_gas = self.state.used_gas + code_deposit_gas(sub.state.out.len());
                    self.state.account_state.code_deposit(sub.state.context.address,
                                                          sub.state.out.as_slice());
                } else {
                    self.state.account_state.code_deposit(sub.state.context.address, &[]);
                }

            },
            MachineStatus::ExitedErr(_) => {
                self.state.used_gas = self.state.used_gas + sub.state.context.gas_limit;
                self.state.stack.pop().unwrap();
                self.state.stack.push(M256::zero()).unwrap();
            },
            _ => panic!(),
        }
    }

    fn apply_call(&mut self, sub: Machine<M>, out_start: M256, out_len: M256) {
        if self.state.available_gas() < sub.state.used_gas {
            panic!();
        }

        match sub.status() {
            MachineStatus::ExitedOk => {
                let sub_total_used_gas = sub.state.total_used_gas();

                self.state.account_state = sub.state.account_state;
                self.state.blockhash_state = sub.state.blockhash_state;
                self.state.logs = sub.state.logs;
                self.state.removed = sub.state.removed;
                self.state.used_gas = self.state.used_gas + sub_total_used_gas;
                self.state.refunded_gas = self.state.refunded_gas + sub.state.refunded_gas;
                copy_into_memory(&mut self.state.memory, sub.state.out.as_slice(),
                                 out_start, M256::zero(), out_len);
            },
            MachineStatus::ExitedErr(_) => {
                self.state.used_gas = self.state.used_gas + sub.state.context.gas_limit;
                self.state.stack.pop().unwrap();
                self.state.stack.push(M256::zero()).unwrap();
            },
            _ => panic!(),
        }
    }
}
