//! Parameters used by the VM.

use util::gas::Gas;
use util::address::Address;
use util::bigint::U256;
use block::Header;

#[derive(Debug, Clone)]
/// Block header.
pub struct HeaderParams {
    /// Block coinbase, the address that mines the block.
    pub beneficiary: Address,
    /// Block timestamp.
    pub timestamp: u64,
    /// The current block number.
    pub number: U256,
    /// Difficulty of the block.
    pub difficulty: U256,
    /// Total block gas limit.
    pub gas_limit: Gas
}

impl<'a> From<&'a Header> for HeaderParams {
    fn from(val: &'a Header) -> HeaderParams {
        HeaderParams {
            beneficiary: val.beneficiary,
            timestamp: val.timestamp,
            number: val.number,
            difficulty: val.difficulty,
            gas_limit: val.gas_limit,
        }
    }
}

#[derive(Debug, Clone)]
/// A VM context. See the Yellow Paper for more information.
pub struct Context {
    /// Address that is executing this runtime.
    pub address: Address,
    /// Caller of the runtime.
    pub caller: Address,
    /// Code to be executed.
    pub code: Vec<u8>,
    /// Data associated with this execution.
    pub data: Vec<u8>,
    /// Gas limit.
    pub gas_limit: Gas,
    /// Gas price.
    pub gas_price: Gas,
    /// The origin of the context. The same as caller when it is from
    /// a transaction.
    pub origin: Address,
    /// Value passed for this runtime.
    pub value: U256,
    /// Apprent value in the execution context.
    pub apprent_value: U256,
}

pub use block::Log;
