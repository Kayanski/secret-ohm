use std::any::type_name;
use std::convert::TryFrom;

use cosmwasm_std::{CanonicalAddr, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage, bucket, bucket_read};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::viewing_key::ViewingKey;
use serde::de::DeserializeOwned;

pub static CONFIG_KEY: &[u8] = b"config";
pub const PREFIX_TXS: &[u8] = b"transfers";

pub const KEY_CONSTANTS: &[u8] = b"constants";
pub const KEY_TOTAL_DEBT: &[u8] = b"total_debt";
pub const KEY_LAST_DECAY: &[u8] = b"last_decay";
pub const KEY_INFO: &[u8] = b"info";
pub const KEY_ADJUSTMENTS: &[u8] = b"adjustments";

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BONDS: &[u8] = b"bonds";
pub const PREFIX_VIEW_KEY: &[u8] = b"viewingkey";



// Config
#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Constants {
    pub name: String,
    pub symbol: String,
    pub ohm: Contract,
    pub ohm_decimals: u8,
    pub principle: Principle,
    pub principle_decimals: u8,
    pub treasury: Contract,
    pub dao: HumanAddr,
    pub bond_calculator: Option<Contract>,

    pub staking: Option<Contract>,

    pub terms: Option<Terms>,
    pub adjustment: Option<Adjust>,

    pub admin: HumanAddr,
    pub prng_seed: Vec<u8>,
    pub contract_address: HumanAddr
}



// Info for creating new bonds
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Terms {
    pub control_variable: Uint128, // scaling variable for price
    pub vesting_term: u64, // in blocks
    pub minimum_price: Uint128, // vs principle value
    pub maximum_price: Uint128, // (to limit the token price)
    pub max_payout: Uint128 ,// in thousandths of a %. i.e. 500 = 0.5%
    pub fee: Uint128, // as % of bond payout, in hundreths. ( 500 = 5% = 0.05 for every 1 paid)
    pub max_debt: Uint128, // 9 decimal debt ratio, max % total supply created as debt
}

// Info for bond holder
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Bond{
    pub payout: Uint128, // OHM remaining to be paid
    pub vesting: u64, // Blocks left to vest
    pub last_block: u64, // Last interaction
    pub price_paid: Uint128, // In DAI, for front end viewing
}

impl Default for Bond{
    fn default() -> Self{
        Bond{
            payout: Uint128(0),
            vesting: 0,
            last_block: 0,
            price_paid: Uint128(0)
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Adjust {
    pub add: bool, // addition or subtraction
    pub rate: Uint128, // increment
    pub target: Uint128, // BCV when adjustment finished
    pub buffer: u64, // minimum length (in blocks) between adjustments
    pub last_block: u64, // block when last adjustment made
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Info{
    pub rate: Uint128, // in ten-thousandths ( 5000 = 0.5% )
    pub recipient: HumanAddr
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Contract{
    pub address : HumanAddr,
    pub code_hash : String
}


#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Principle{
    pub token: Contract,
    pub pair: Option<Contract>
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

    pub fn total_debt(&self) -> u128 {
        self.as_readonly().total_debt()
    }

    pub fn last_decay(&self) -> u64 {
        self.as_readonly().last_decay()
    }

    pub fn rate_info(&self) -> Vec<Info> {
        self.as_readonly().rate_info()
    }

    pub fn info_by_recipient(&self, recipient: HumanAddr) -> Info {
        self.as_readonly().info_by_recipient(recipient)
    }

    pub fn adjustment(&self, index: usize) -> Adjust {
        self.as_readonly().adjustment(index)
    }
}

fn ser_bin_data<T: Serialize>(obj: &T) -> StdResult<Vec<u8>> {
    bincode2::serialize(&obj).map_err(|e| StdError::serialize_err(type_name::<T>(), e))
}

fn deser_bin_data<T: DeserializeOwned>(data: &[u8]) -> StdResult<T> {
    bincode2::deserialize::<T>(&data).map_err(|e| StdError::serialize_err(type_name::<T>(), e))
}

fn set_bin_data<T: Serialize, S: Storage>(storage: &mut S, key: &[u8], data: &T) -> StdResult<()> {
    let bin_data = ser_bin_data(data)?;

    storage.set(key, &bin_data);
    Ok(())
}

fn get_bin_data<T: DeserializeOwned, S: ReadonlyStorage>(storage: &S, key: &[u8]) -> StdResult<T> {
    let bin_data = storage.get(key);

    match bin_data {
        None => Err(StdError::not_found("Key not found in storage")),
        Some(bin_data) => Ok(deser_bin_data(&bin_data)?),
    }
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

    pub fn total_debt(&self) -> u128 {
        self.as_readonly().total_debt()
    }

    pub fn set_total_debt(&mut self, debt: u128) {
        self.storage.set(KEY_TOTAL_DEBT, &debt.to_be_bytes());
    }

    pub fn last_decay(&self) -> u64 {
        self.as_readonly().last_decay()
    }

    pub fn set_last_decay(&mut self, decay: u64) {
        self.storage.set(KEY_LAST_DECAY, &decay.to_be_bytes());
    }

    pub fn rate_info(&self) -> Vec<Info> {
        self.as_readonly().rate_info()
    }

    pub fn info_by_recipient(&self, recipient: HumanAddr) -> Info {
        self.as_readonly().info_by_recipient(recipient)
    }

    pub fn set_info(&mut self, info_to_set: Vec<Info>) -> StdResult<()> {
        set_bin_data(&mut self.storage, KEY_INFO, &info_to_set)
    }

    pub fn add_info(&mut self, info_to_add: Vec<Info>) -> StdResult<()> {
        let mut info = self.rate_info();
        info.extend(info_to_add);

        self.set_info(info)
    }

    pub fn remove_info(&mut self, recipient_to_remove: HumanAddr) -> StdResult<()> {
        let mut info = self.rate_info();

        info.retain(|x| {x.recipient != recipient_to_remove});
        
        self.set_info(info)
    }

    pub fn set_adjustment(&mut self, index: usize, adjust: Adjust) -> StdResult<()> {
        bucket(KEY_ADJUSTMENTS, &mut self.storage).save(&index.to_be_bytes(),&adjust)
    }

    pub fn adjustment(&self, index: usize) -> Adjust {
        self.as_readonly().adjustment(index)
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

    fn total_debt(&self) -> u128 {
        let debt_bytes = self
            .0
            .get(KEY_TOTAL_DEBT)
            .expect("no total debt stored in config");
        // This unwrap is ok because we know we stored things correctly
        slice_to_u128(&debt_bytes).unwrap()
    }

    fn last_decay(&self) -> u64 {
        let decay_bytes = self
            .0
            .get(KEY_LAST_DECAY)
            .expect("no last decay stored in config");
        // This unwrap is ok because we know we stored things correctly
        slice_to_u64(&decay_bytes).unwrap()
    }

    fn rate_info(&self) -> Vec<Info> {
        get_bin_data(self.0, KEY_INFO).unwrap()
    }

    pub fn info_by_recipient(&self, recipient: HumanAddr) -> Info {
        let recipient_info: Vec<Info> = self.rate_info()
        .into_iter()
        .filter(|voc| voc.recipient == recipient.clone())
        .collect();
        recipient_info.get(0).unwrap().clone()
    }

    fn adjustment(&self, index: usize) -> Adjust {
        bucket_read(KEY_ADJUSTMENTS, self.0).load(&index.to_be_bytes()).unwrap()
    }
}

// BondInfo

pub struct ReadonlyBondInfo<'a, S: ReadonlyStorage> {
    storage: ReadonlyPrefixedStorage<'a, S>,
}

impl<'a, S: ReadonlyStorage> ReadonlyBondInfo<'a, S> {
    pub fn from_storage(storage: &'a S) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(PREFIX_BONDS, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyBondInfoImpl<ReadonlyPrefixedStorage<S>> {
        ReadonlyBondInfoImpl(&self.storage)
    }

    pub fn bond(&self, account: &CanonicalAddr) -> Bond {
        self.as_readonly().bond(account)
    }
}

pub struct BondInfo<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> BondInfo<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(PREFIX_BONDS, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyBondInfoImpl<PrefixedStorage<S>> {
        ReadonlyBondInfoImpl(&self.storage)
    }

    pub fn bond(&self, account: &CanonicalAddr) -> Bond {
        self.as_readonly().bond(account)
    }

    pub fn set_bond(&mut self, account: &CanonicalAddr, bond: Bond) -> StdResult<()> {
        set_bin_data(&mut self.storage, account.as_slice(), &bond)
    }
}

/// This struct refactors out the readonly methods that we need for `BondInfo` and `ReadonlyBondInfo`
/// in a way that is generic over their mutability.
///
/// This was the only way to prevent code duplication of these methods because of the way
/// that `ReadonlyPrefixedStorage` and `PrefixedStorage` are implemented in `cosmwasm-std`
struct ReadonlyBondInfoImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyBondInfoImpl<'a, S> {
    pub fn bond(&self, account: &CanonicalAddr) -> Bond {
        get_bin_data(self.0, account.as_slice()).unwrap_or_default()
    }
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

/// Converts 16 bytes value into u128
/// Errors if data found that is not 16 bytes
fn slice_to_u64(data: &[u8]) -> StdResult<u64> {
    match <[u8; 8]>::try_from(data) {
        Ok(bytes) => Ok(u64::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 8 byte expected.",
        )),
    }
}
