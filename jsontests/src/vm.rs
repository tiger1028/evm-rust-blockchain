use std::collections::{HashMap, BTreeMap};
use std::rc::Rc;
use serde::{Serialize, Deserialize};
use primitive_types::{H160, H256};
use evm::gasometer;
use evm::backend::{Backend, ApplyBackend, MemoryBackend, MemoryVicinity, MemoryAccount};
use evm::executor::StackExecutor;
use crate::utils::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Test {
	pub env: Env,
	pub exec: Exec,
	pub pre: HashMap<String, Account>,
	pub gas: Option<String>,
	pub out: Option<String>,
	pub post: Option<HashMap<String, Account>>,
}

impl Test {
	pub fn unwrap_to_pre_state(&self) -> BTreeMap<H160, MemoryAccount> {
		self.pre.iter().map(|(k, v)| {
			(unwrap_to_h160(&k), v.unwrap_to_account())
		}).collect()
	}

	pub fn unwrap_to_post_state(&self) -> BTreeMap<H160, MemoryAccount> {
		self.post.as_ref().unwrap().iter().map(|(k, v)| {
			(unwrap_to_h160(&k), v.unwrap_to_account())
		}).collect()
	}

	pub fn unwrap_to_vicinity(&self) -> MemoryVicinity {
		MemoryVicinity {
			gas_price: unwrap_to_u256(&self.exec.gas_price),
			origin: unwrap_to_h160(&self.exec.origin),
			block_hashes: Vec::new(),
			block_number: unwrap_to_u256(&self.env.current_number),
			block_coinbase: unwrap_to_h160(&self.env.current_coinbase),
			block_timestamp: unwrap_to_u256(&self.env.current_timestamp),
			block_difficulty: unwrap_to_u256(&self.env.current_difficulty),
			block_gas_limit: unwrap_to_u256(&self.env.current_gas_limit),
		}
	}

	pub fn unwrap_to_code(&self) -> Rc<Vec<u8>> {
		Rc::new(unwrap_to_vec(&self.exec.code))
	}

	pub fn unwrap_to_data(&self) -> Rc<Vec<u8>> {
		Rc::new(unwrap_to_vec(&self.exec.data))
	}

	pub fn unwrap_to_context(&self) -> evm::Context {
		evm::Context {
			address: unwrap_to_h160(&self.exec.address),
			caller: unwrap_to_h160(&self.exec.caller),
			apparent_value: unwrap_to_u256(&self.exec.value),
		}
	}

	pub fn unwrap_to_return_value(&self) -> Vec<u8> {
		unwrap_to_vec(&self.out.as_ref().unwrap())
	}

	pub fn unwrap_to_gas_limit(&self) -> usize {
		unwrap_to_u256(&self.exec.gas).as_usize()
	}

	pub fn unwrap_to_post_gas(&self) -> usize {
		unwrap_to_u256(&self.gas.as_ref().unwrap()).as_usize()
	}
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Env {
	pub current_coinbase: String,
	pub current_difficulty: String,
	pub current_gas_limit: String,
	pub current_number: String,
	pub current_timestamp: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Exec {
	pub address: String,
	pub caller: String,
	pub code: String,
	pub data: String,
	pub gas: String,
	pub gas_price: String,
	pub origin: String,
	pub value: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
	pub balance: String,
	pub code: String,
	pub nonce: String,
	pub storage: HashMap<String, String>,
}

impl Account {
	pub fn unwrap_to_account(&self) -> MemoryAccount {
		MemoryAccount {
			balance: unwrap_to_u256(&self.balance),
			code: unwrap_to_vec(&self.code),
			nonce: unwrap_to_u256(&self.nonce),
			storage: self.storage.iter().map(|(k, v)| {
				let ku = unwrap_to_u256(&k);
				let mut k = H256::default();
				ku.to_big_endian(&mut k[..]);

				let vu = unwrap_to_u256(&v);
				let mut v = H256::default();
				vu.to_big_endian(&mut v[..]);

				(k, v)
			}).collect(),
		}
	}
}

pub fn test(name: &str, test: Test) {
	use std::io::{self, Write};

	print!("Running test {} ... ", name);
	io::stdout().flush().ok().expect("Could not flush stdout");

	let original_state = test.unwrap_to_pre_state();
	let vicinity = test.unwrap_to_vicinity();
	let gasometer_config = gasometer::Config::frontier();
	let mut backend = MemoryBackend::new(&vicinity, original_state);
	let mut executor = StackExecutor::new(
		&backend,
		test.unwrap_to_gas_limit(),
		&gasometer_config,
	);

	let code = test.unwrap_to_code();
	let data = test.unwrap_to_data();
	let context = test.unwrap_to_context();
	let mut runtime = evm::Runtime::new(code, data, 1024, 1000000, context);

	let reason = executor.execute(&mut runtime);
	let (gas, values, logs) = executor.finalize();
	backend.apply(values, logs);

	if test.out.is_none() {
		print!("{:?} ", reason);

		assert!(reason.is_error());
		assert!(test.post.is_none() && test.gas.is_none());

		println!("succeed");
	} else {
		let expected_post_gas = test.unwrap_to_post_gas();
		print!("{:?} ", reason);

		assert_eq!(runtime.machine().return_value(), test.unwrap_to_return_value());
		assert_eq!(backend.state(), &test.unwrap_to_post_state());
		assert_eq!(gas, expected_post_gas);
		println!("succeed");
	}
}
