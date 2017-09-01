#![deny(unused_import_braces,
        unused_comparisons, unused_must_use,
        unused_variables, non_shorthand_field_patterns,
        unreachable_code)]

extern crate trie;
extern crate sputnikvm;
extern crate sha3;
extern crate block;
extern crate rlp;

use sputnikvm::{H256, U256, M256, Address};
use sputnikvm::vm::{self, ValidTransaction, HeaderParams, Memory, TransactionVM, VM,
                    AccountCommitment, Patch, SeqMemory, AccountState};
use sputnikvm::vm::errors::{PreExecutionError, RequireError};
use sha3::{Keccak256, Digest};
use trie::{Trie, SecureTrie, FixedSecureTrie, DatabaseGuard, MemoryDatabase, MemoryDatabaseGuard, Database, DatabaseOwned};
use block::{Account, Transaction};
use std::collections::HashMap;
use std::cmp::min;

pub struct Stateful<D> {
    database: D,
    root: H256,
}

impl<D> Stateful<D> {
    pub fn new(database: D, root: H256) -> Self {
        Self {
            database,
            root
        }
    }
}

impl<D: Default> Default for Stateful<D> {
    fn default() -> Self {
        Self {
            database: D::default(),
            root: MemoryDatabase::new().create_empty().root(),
        }
    }
}

impl<D: DatabaseOwned> Stateful<D> {
    fn is_empty_hash(hash: H256) -> bool {
        hash == H256::from(Keccak256::digest(&[]).as_slice())
    }

    pub fn code(&self, hash: H256) -> Option<Vec<u8>> {
        let code_hashes = self.database.create_guard();

        if Self::is_empty_hash(hash) {
            Some(Vec::new())
        } else {
            code_hashes.get(hash)
        }
    }

    pub fn call<M: Memory + Default>(
        &self, transaction: ValidTransaction, block: HeaderParams,
        patch: &'static Patch, most_recent_block_hashes: &[H256]
    ) -> TransactionVM<M> {
        assert!(U256::from(most_recent_block_hashes.len()) >=
                min(block.number, U256::from(256)));

        let mut vm = TransactionVM::new(transaction, block.clone(), patch);
        let state = self.database.create_fixed_secure_trie(self.root);
        let code_hashes = self.database.create_guard();

        loop {
            match vm.fire() {
                Ok(()) => break,
                Err(RequireError::Account(address)) => {
                    let account: Option<Account> = state.get(&address);

                    match account {
                        Some(account) => {
                            let code = if Self::is_empty_hash(account.code_hash) {
                                Vec::new()
                            } else {
                                code_hashes.get(account.code_hash).unwrap()
                            };

                            vm.commit_account(AccountCommitment::Full {
                                nonce: account.nonce,
                                address: address,
                                balance: account.balance,
                                code: code,
                            }).unwrap();
                        },
                        None => {
                            vm.commit_account(AccountCommitment::Nonexist(address)).unwrap();
                        },
                    }
                },
                Err(RequireError::AccountCode(address)) => {
                    let account: Option<Account> = state.get(&address);

                    match account {
                        Some(account) => {
                            let code = if Self::is_empty_hash(account.code_hash) {
                                Vec::new()
                            } else {
                                code_hashes.get(account.code_hash).unwrap()
                            };

                            vm.commit_account(AccountCommitment::Code {
                                address: address,
                                code: code,
                            }).unwrap();
                        },
                        None => {
                            vm.commit_account(AccountCommitment::Nonexist(address)).unwrap();
                        },
                    }
                },
                Err(RequireError::AccountStorage(address, index)) => {
                    let account: Option<Account> = state.get(&address);

                    match account {
                        Some(account) => {
                            let storage = self.database.create_fixed_secure_trie(account.storage_root);
                            let value = storage.get(&H256::from(index)).unwrap_or(M256::zero());

                            vm.commit_account(AccountCommitment::Storage {
                                address: address,
                                index, value
                            }).unwrap();
                        },
                        None => {
                            vm.commit_account(AccountCommitment::Nonexist(address)).unwrap();
                        },
                    }
                },
                Err(RequireError::Blockhash(number)) => {
                    let index = (block.number - number).as_usize();
                    vm.commit_blockhash(number, most_recent_block_hashes[index]).unwrap();
                },
            }
        }

        vm
    }

    pub fn transit(
        &mut self, accounts: &[vm::Account]
    ) {
        let mut state = self.database.create_fixed_secure_trie(self.root);
        let mut code_hashes = self.database.create_guard();

        for account in accounts {
            match account.clone() {
                vm::Account::Full {
                    nonce, address, balance, changing_storage, code
                } => {
                    let changing_storage: HashMap<U256, M256> = changing_storage.into();

                    let mut account: Account = state.get(&address).unwrap();

                    let mut storage_trie = self.database.create_fixed_secure_trie(account.storage_root);
                    for (key, value) in changing_storage {
                        if value == M256::zero() {
                            storage_trie.remove(&H256::from(key));
                        } else {
                            storage_trie.insert(H256::from(key), value);
                        }
                    }

                    account.balance = balance;
                    account.nonce = nonce;
                    account.storage_root = storage_trie.root();
                    assert!(account.code_hash == H256::from(Keccak256::digest(&code).as_slice()));

                    state.insert(address, account);
                },
                vm::Account::IncreaseBalance(address, value) => {
                    match state.get(&address) {
                        Some(mut account) => {
                            account.balance = account.balance + value;
                            state.insert(address, account);
                        },
                        None => {
                            let account = Account {
                                nonce: U256::zero(),
                                balance: value,
                                storage_root: self.database.create_empty().root(),
                                code_hash: H256::from(Keccak256::digest(&[]).as_slice())
                            };
                            state.insert(address, account);
                        }
                    }
                },
                vm::Account::DecreaseBalance(address, value) => {
                    let mut account: Account = state.get(&address).unwrap();

                    account.balance = account.balance - value;

                    state.insert(address, account);
                },
                vm::Account::Create {
                    nonce, address, balance, storage, code, exists
                } => {
                    if !exists {
                        state.remove(&address);
                    } else {
                        let storage: HashMap<U256, M256> = storage.into();

                        let mut storage_trie = self.database.create_fixed_secure_empty();
                        for (key, value) in storage {
                            if value == M256::zero() {
                                storage_trie.remove(&H256::from(key));
                            } else {
                                storage_trie.insert(H256::from(key), value);
                            }
                        }

                        let code_hash = H256::from(Keccak256::digest(&code).as_slice());
                        code_hashes.set(code_hash, code);

                        let account = Account {
                            nonce: nonce,
                            balance: balance,
                            storage_root: storage_trie.root(),
                            code_hash
                        };

                        state.insert(address, account);
                    }
                },
            }
        }

        self.root = state.root();
    }

    pub fn execute<M: Memory + Default>(
        &mut self, transaction: ValidTransaction, block: HeaderParams,
        patch: &'static Patch, most_recent_block_hashes: &[H256]
    ) -> TransactionVM<M> {
        let vm = self.call(transaction, block, patch, most_recent_block_hashes);
        let mut accounts = Vec::new();
        for account in vm.accounts() {
            accounts.push(account.clone());
        }
        self.transit(&accounts);
        vm
    }

    pub fn to_valid(
        &self, transaction: Transaction, patch: &'static Patch
    ) -> Result<ValidTransaction, PreExecutionError> {
        let state = self.database.create_fixed_secure_trie(self.root);
        let code_hashes = self.database.create_guard();
        let mut account_state = AccountState::default();

        loop {
            match ValidTransaction::from_transaction(&transaction, &account_state, patch) {
                Ok(val) => return val,
                Err(RequireError::Account(address)) => {
                    let account: Option<Account> = state.get(&address);

                    match account {
                        Some(account) => {
                            let code = if Self::is_empty_hash(account.code_hash) {
                                Vec::new()
                            } else {
                                code_hashes.get(account.code_hash).unwrap()
                            };

                            account_state.commit(AccountCommitment::Full {
                                nonce: account.nonce,
                                address: address,
                                balance: account.balance,
                                code: code,
                            }).unwrap();
                        },
                        None => {
                            account_state.commit(AccountCommitment::Nonexist(address)).unwrap();
                        },
                    }
                },
                Err(RequireError::AccountCode(address)) => {
                    let account: Option<Account> = state.get(&address);

                    match account {
                        Some(account) => {
                            let code = if Self::is_empty_hash(account.code_hash) {
                                Vec::new()
                            } else {
                                code_hashes.get(account.code_hash).unwrap()
                            };

                            account_state.commit(AccountCommitment::Code {
                                address: address,
                                code: code,
                            }).unwrap();
                        },
                        None => {
                            account_state.commit(AccountCommitment::Nonexist(address)).unwrap();
                        },
                    }
                },
                Err(RequireError::AccountStorage(address, index)) => {
                    let account: Option<Account> = state.get(&address);

                    match account {
                        Some(account) => {
                            let storage = self.database.create_fixed_secure_trie(account.storage_root);
                            let value = storage.get(&H256::from(index)).unwrap_or(M256::zero());

                            account_state.commit(AccountCommitment::Storage {
                                address: address,
                                index, value
                            }).unwrap();
                        },
                        None => {
                            account_state.commit(AccountCommitment::Nonexist(address)).unwrap();
                        },
                    }
                },
                Err(RequireError::Blockhash(_)) => {
                    panic!()
                },
            }
        }
    }

    pub fn root(&self) -> H256 {
        self.root
    }

    pub fn state_of<'a>(&'a self, root: H256) -> FixedSecureTrie<<D as Database<'a>>::Guard, Address, Account> {
        self.database.create_fixed_secure_trie::<Address, Account>(root)
    }

    pub fn state<'a>(&'a self) -> FixedSecureTrie<<D as Database<'a>>::Guard, Address, Account> {
        self.state_of(self.root())
    }

    pub fn storage_state_of<'a>(&'a self, root: H256) -> FixedSecureTrie<<D as Database<'a>>::Guard, H256, M256> {
        self.database.create_fixed_secure_trie::<H256, M256>(root)
    }

    pub fn storage_state<'a>(&'a self, address: Address) -> Option<FixedSecureTrie<<D as Database<'a>>::Guard, H256, M256>> {
        let state = self.state();
        let account = state.get(&address);

        match account {
            Some(account) => {
                Some(self.storage_state_of(account.storage_root))
            },
            None => None,
        }
    }
}

pub type MemoryStateful = Stateful<MemoryDatabase>;
