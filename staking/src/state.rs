use std::any::type_name;
use std::convert::TryFrom;

use cosmwasm_std::{
    CanonicalAddr, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{bucket, bucket_read, PrefixedStorage, ReadonlyPrefixedStorage};

use secret_toolkit::storage::{TypedStore, TypedStoreMut};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{status_level_to_u8, u8_to_status_level, ContractStatusLevel};
use crate::viewing_key::ViewingKey;

use primitive_types::U256;

pub static CONFIG_KEY: &[u8] = b"config";
pub const PREFIX_TXS: &[u8] = b"transfers";

pub const KEY_CONSTANTS: &[u8] = b"constants";
pub const KEY_CONTRACTS: &[u8] = b"contracts";
pub const KEY_TOTAL_SUPPLY: &[u8] = b"total_supply";
pub const KEY_CURRENTLY_STAKING: &[u8] = b"currently_staking";
pub const KEY_CONTRACT_STATUS: &[u8] = b"contract_status";
pub const KEY_WARMUP_INFO: &[u8] = b"warmup_info";

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";
pub const PREFIX_VIEW_KEY: &[u8] = b"viewingkey";
pub const PREFIX_RECEIVERS: &[u8] = b"receivers";

// Config

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Constants {
    pub admin: HumanAddr,
    pub prng_seed: Vec<u8>,
    pub ohm: Contract,
    pub sohm: Contract,
    pub epoch: Epoch,
    pub total_bonus: Uint128,
    pub warmup_period: u64,
    // the address of this contract, used to validate query permits
    pub contract_address: HumanAddr,
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct ConfigContracts {
    pub distributor: Contract,
    pub warmup: Contract,
    pub locker: Contract,
}

impl Default for ConfigContracts {
    fn default() -> Self {
        Self {
            distributor: Contract::default(),
            warmup: Contract::default(),
            locker: Contract::default(),
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ContractType {
    Distributor,
    WarmupContract,
    Locker,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Contract {
    pub address: HumanAddr,
    pub code_hash: String,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            address: HumanAddr::default(),
            code_hash: String::default(),
        }
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Epoch {
    pub length: u64,
    pub number: u64,
    pub end_block: u64,
    pub distribute: Uint128,
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Claim {
    pub deposit: Uint128,
    pub gons: String,
    pub expiry: u64,
    pub lock: bool, // prevents malicious delays
}

impl Default for Claim {
    fn default() -> Self {
        Self {
            deposit: Uint128(0),
            gons: U256::to_string(&U256::zero()),
            expiry: 0,
            lock: false,
        }
    }
}

pub struct ReadonlyConfig<'a, S: ReadonlyStorage> {
    storage: ReadonlyPrefixedStorage<'a, S>,
}

impl<'a, S: ReadonlyStorage> ReadonlyConfig<'a, S> {
    pub fn from_storage(storage: &'a S) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyConfigImpl<ReadonlyPrefixedStorage<S>> {
        ReadonlyConfigImpl(&self.storage)
    }

    pub fn constants(&self) -> StdResult<Constants> {
        self.as_readonly().constants()
    }

    pub fn contracts(&self) -> StdResult<ConfigContracts> {
        self.as_readonly().contracts()
    }

    pub fn total_supply(&self) -> u128 {
        self.as_readonly().total_supply()
    }

    pub fn currently_staking(&self) -> u128 {
        self.as_readonly().currently_staking()
    }

    pub fn contract_status(&self) -> ContractStatusLevel {
        self.as_readonly().contract_status()
    }

    pub fn warmup_info(&self, address: &CanonicalAddr) -> Claim {
        self.as_readonly().warmup_info(address)
    }
}

fn ser_bin_data<T: Serialize>(obj: &T) -> StdResult<Vec<u8>> {
    bincode2::serialize(&obj).map_err(|e| StdError::serialize_err(type_name::<T>(), e))
}


fn set_bin_data<T: Serialize, S: Storage>(storage: &mut S, key: &[u8], data: &T) -> StdResult<()> {
    let bin_data = ser_bin_data(data)?;

    storage.set(key, &bin_data);
    Ok(())
}


pub struct Config<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> Config<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(PREFIX_CONFIG, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyConfigImpl<PrefixedStorage<S>> {
        ReadonlyConfigImpl(&self.storage)
    }

    pub fn constants(&self) -> StdResult<Constants> {
        self.as_readonly().constants()
    }

    pub fn set_constants(&mut self, constants: &Constants) -> StdResult<()> {
        set_bin_data(&mut self.storage, KEY_CONSTANTS, constants)
    }

    pub fn contracts(&self) -> StdResult<ConfigContracts> {
        self.as_readonly().contracts()
    }

    pub fn set_contracts(&mut self, contracts: &ConfigContracts) -> StdResult<()> {
        set_bin_data(&mut self.storage, KEY_CONTRACTS, contracts)
    }

    pub fn total_supply(&self) -> u128 {
        self.as_readonly().total_supply()
    }

    pub fn set_total_supply(&mut self, supply: u128) {
        self.storage.set(KEY_TOTAL_SUPPLY, &supply.to_be_bytes());
    }

    pub fn currently_staking(&self) -> u128 {
        self.as_readonly().currently_staking()
    }

    pub fn is_currently_staking(&mut self, amount: u128) {
        self.storage.set(KEY_CURRENTLY_STAKING, &amount.to_be_bytes());
    }

    pub fn contract_status(&self) -> ContractStatusLevel {
        self.as_readonly().contract_status()
    }

    pub fn set_contract_status(&mut self, status: ContractStatusLevel) {
        let status_u8 = status_level_to_u8(status);
        self.storage
            .set(KEY_CONTRACT_STATUS, &status_u8.to_be_bytes());
    }

    pub fn set_warmup_info(&mut self, address: &CanonicalAddr, claim: Claim) -> StdResult<()> {
        bucket(KEY_WARMUP_INFO, &mut self.storage).save(address.as_slice(), &claim)
    }

    pub fn warmup_info(&self, address: &CanonicalAddr) -> Claim {
        self.as_readonly().warmup_info(address)
    }
}

/// This struct refactors out the readonly methods that we need for `Config` and `ReadonlyConfig`
/// in a way that is generic over their mutability.
///
/// This was the only way to prevent code duplication of these methods because of the way
/// that `ReadonlyPrefixedStorage` and `PrefixedStorage` are implemented in `cosmwasm-std`
struct ReadonlyConfigImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyConfigImpl<'a, S> {
    fn constants(&self) -> StdResult<Constants> {
        let consts_bytes = self
            .0
            .get(KEY_CONSTANTS)
            .ok_or_else(|| StdError::generic_err("no constants stored in configuration"))?;
        bincode2::deserialize::<Constants>(&consts_bytes)
            .map_err(|e| StdError::serialize_err(type_name::<Constants>(), e))
    }

    fn contracts(&self) -> StdResult<ConfigContracts> {
        let consts_bytes = self
            .0
            .get(KEY_CONTRACTS)
            .ok_or_else(|| StdError::generic_err("no contracts stored in configuration"))?;
        bincode2::deserialize::<ConfigContracts>(&consts_bytes)
            .map_err(|e| StdError::serialize_err(type_name::<ConfigContracts>(), e))
    }

    fn total_supply(&self) -> u128 {
        let supply_bytes = self
            .0
            .get(KEY_TOTAL_SUPPLY)
            .expect("no total supply stored in config");
        // This unwrap is ok because we know we stored things correctly
        slice_to_u128(&supply_bytes).unwrap()
    }

    fn currently_staking(&self) -> u128 {
        let amount_bytes = self
            .0
            .get(KEY_CURRENTLY_STAKING)
            .expect("you forgot to state if the contract was currently staking something");
        slice_to_u128(&amount_bytes).unwrap()
    }

    fn contract_status(&self) -> ContractStatusLevel {
        let supply_bytes = self
            .0
            .get(KEY_CONTRACT_STATUS)
            .expect("no contract status stored in config");

        // These unwraps are ok because we know we stored things correctly
        let status = slice_to_u8(&supply_bytes).unwrap();
        u8_to_status_level(status).unwrap()
    }

    fn warmup_info(&self, address: &CanonicalAddr) -> Claim {
        bucket_read(KEY_WARMUP_INFO, self.0)
            .load(address.as_slice())
            .unwrap_or_default()
    }
}

// Balances

pub struct ReadonlyBalances<'a, S: ReadonlyStorage> {
    storage: ReadonlyPrefixedStorage<'a, S>,
}

impl<'a, S: ReadonlyStorage> ReadonlyBalances<'a, S> {
    pub fn from_storage(storage: &'a S) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(PREFIX_BALANCES, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyBalancesImpl<ReadonlyPrefixedStorage<S>> {
        ReadonlyBalancesImpl(&self.storage)
    }

    pub fn account_amount(&self, account: &CanonicalAddr) -> u128 {
        self.as_readonly().account_amount(account)
    }
}

pub struct Balances<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> Balances<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(PREFIX_BALANCES, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyBalancesImpl<PrefixedStorage<S>> {
        ReadonlyBalancesImpl(&self.storage)
    }

    pub fn balance(&self, account: &CanonicalAddr) -> u128 {
        self.as_readonly().account_amount(account)
    }

    pub fn set_account_balance(&mut self, account: &CanonicalAddr, amount: u128) {
        self.storage.set(account.as_slice(), &amount.to_be_bytes())
    }
}

/// This struct refactors out the readonly methods that we need for `Balances` and `ReadonlyBalances`
/// in a way that is generic over their mutability.
///
/// This was the only way to prevent code duplication of these methods because of the way
/// that `ReadonlyPrefixedStorage` and `PrefixedStorage` are implemented in `cosmwasm-std`
struct ReadonlyBalancesImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyBalancesImpl<'a, S> {
    pub fn account_amount(&self, account: &CanonicalAddr) -> u128 {
        let account_bytes = account.as_slice();
        let result = self.0.get(account_bytes);
        match result {
            // This unwrap is ok because we know we stored things correctly
            Some(balance_bytes) => slice_to_u128(&balance_bytes).unwrap(),
            None => 0,
        }
    }
}

// Allowances

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, Default, JsonSchema)]
pub struct Allowance {
    pub amount: Uint128,
    pub expiration: Option<u64>,
}

impl Allowance {
    pub fn is_expired_at(&self, block: &cosmwasm_std::BlockInfo) -> bool {
        match self.expiration {
            Some(time) => block.time >= time,
            None => false, // allowance has no expiration
        }
    }
}

pub fn read_allowance<S: Storage>(
    store: &S,
    owner: &CanonicalAddr,
    spender: &CanonicalAddr,
) -> StdResult<Allowance> {
    let owner_store =
        ReadonlyPrefixedStorage::multilevel(&[PREFIX_ALLOWANCES, owner.as_slice()], store);
    let owner_store = TypedStore::attach(&owner_store);
    let allowance = owner_store.may_load(spender.as_slice());
    allowance.map(Option::unwrap_or_default)
}

pub fn write_allowance<S: Storage>(
    store: &mut S,
    owner: &CanonicalAddr,
    spender: &CanonicalAddr,
    allowance: Allowance,
) -> StdResult<()> {
    let mut owner_store =
        PrefixedStorage::multilevel(&[PREFIX_ALLOWANCES, owner.as_slice()], store);
    let mut owner_store = TypedStoreMut::attach(&mut owner_store);

    owner_store.store(spender.as_slice(), &allowance)
}

// Viewing Keys

pub fn write_viewing_key<S: Storage>(store: &mut S, owner: &CanonicalAddr, key: &ViewingKey) {
    let mut balance_store = PrefixedStorage::new(PREFIX_VIEW_KEY, store);
    balance_store.set(owner.as_slice(), &key.to_hashed());
}

pub fn read_viewing_key<S: Storage>(store: &S, owner: &CanonicalAddr) -> Option<Vec<u8>> {
    let balance_store = ReadonlyPrefixedStorage::new(PREFIX_VIEW_KEY, store);
    balance_store.get(owner.as_slice())
}

// Receiver Interface

pub fn get_receiver_hash<S: ReadonlyStorage>(
    store: &S,
    account: &HumanAddr,
) -> Option<StdResult<String>> {
    let store = ReadonlyPrefixedStorage::new(PREFIX_RECEIVERS, store);
    store.get(account.as_str().as_bytes()).map(|data| {
        String::from_utf8(data)
            .map_err(|_err| StdError::invalid_utf8("stored code hash was not a valid String"))
    })
}

pub fn set_receiver_hash<S: Storage>(store: &mut S, account: &HumanAddr, code_hash: String) {
    let mut store = PrefixedStorage::new(PREFIX_RECEIVERS, store);
    store.set(account.as_str().as_bytes(), code_hash.as_bytes());
}

// Helpers

/// Converts 16 bytes value into u128
/// Errors if data found that is not 16 bytes
fn slice_to_u128(data: &[u8]) -> StdResult<u128> {
    match <[u8; 16]>::try_from(data) {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 16 byte expected.",
        )),
    }
}

/// Converts 1 byte value into u8
/// Errors if data found that is not 1 byte
fn slice_to_u8(data: &[u8]) -> StdResult<u8> {
    if data.len() == 1 {
        Ok(data[0])
    } else {
        Err(StdError::generic_err(
            "Corrupted data found. 1 byte expected.",
        ))
    }
}
