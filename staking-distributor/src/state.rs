use std::any::type_name;

use cosmwasm_std::{CanonicalAddr, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage, bucket, bucket_read};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::viewing_key::ViewingKey;
use serde::de::DeserializeOwned;

pub static CONFIG_KEY: &[u8] = b"config";
pub const PREFIX_TXS: &[u8] = b"transfers";

pub const KEY_CONSTANTS: &[u8] = b"constants";
pub const KEY_INFO: &[u8] = b"info";
pub const KEY_ADJUSTMENTS: &[u8] = b"adjustments";

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_VIEW_KEY: &[u8] = b"viewingkey";



// Config

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Constants {
    pub ohm: Contract,
    pub treasury: Contract,
    pub epoch_length: u64,
    pub next_epoch_block: u64,
    pub admin: HumanAddr,
    pub prng_seed: Vec<u8>,
    pub contract_address: HumanAddr
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Info{
    pub rate: u128, // in ten-thousandths ( 5000 = 0.5% )
    pub recipient: HumanAddr
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Adjust {
    pub add: bool,
    pub rate: u128,
    pub target: u128,
}


#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Contract{
    pub address : HumanAddr,
    pub code_hash : String
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

    pub fn rate_info(&self) -> Vec<Info> {
        self.as_readonly().rate_info()
    }

    pub fn info_by_recipient(&self, recipient: HumanAddr) -> StdResult<Info> {
        self.as_readonly().info_by_recipient(recipient)
    }

    pub fn adjustment(&self, index: usize) -> StdResult<Adjust> {
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

    pub fn rate_info(&self) -> Vec<Info> {
        self.as_readonly().rate_info()
    }

    pub fn info_by_recipient(&self, recipient: HumanAddr) -> StdResult<Info> {
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

    pub fn adjustment(&self, index: usize) -> StdResult<Adjust> {
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

    fn rate_info(&self) -> Vec<Info> {
        get_bin_data(self.0, KEY_INFO).unwrap_or_default()
    }

    pub fn info_by_recipient(&self, recipient: HumanAddr) -> StdResult<Info> {
        let recipient_info: Vec<Info> = self.rate_info()
        .into_iter()
        .filter(|voc| voc.recipient == recipient.clone())
        .collect();
        recipient_info.get(0).clone().ok_or_else(||{
            StdError::generic_err("No reward rate for this recipient")
        }).map(|x|{x.clone()})
    }

    fn adjustment(&self, index: usize) -> StdResult<Adjust> {
        bucket_read(KEY_ADJUSTMENTS, self.0).load(&index.to_be_bytes())
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
