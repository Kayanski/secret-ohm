use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
   ReadonlyStorage, StdResult, Storage, Uint128
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

use secret_toolkit::storage::{AppendStore, AppendStoreMut};

use crate::state::Config;

const PREFIX_REBASE: &[u8] = b"rebase";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Rebase{
    id:u64,
    epoch: u64,
    rebase: Uint128, // 18 decimals
    total_staked_before: Uint128,
    total_staked_after: Uint128,
    amount_rebased: Uint128,
    index: Uint128,
    block_time: u64,
    block_height: u64
}

fn increment_rebase_count<S: Storage>(store: &mut S) -> StdResult<u64> {
    let mut config = Config::from_storage(store);
    let id = config.rebase_count() + 1;
    config.set_rebase_count(id)?;
    Ok(id)
}

pub fn store_rebase<S: Storage>(
    store: &mut S,
    epoch: u64,
    rebase: u128, // 18 decimals
    total_staked_before: u128,
    total_staked_after: u128,
    amount_rebased: u128,
    index: u128,
    block: &cosmwasm_std::BlockInfo,
) -> StdResult<()> {
    let id = increment_rebase_count(store)?;
    let rebase = Rebase{
        id,
        epoch,
        rebase: Uint128(rebase),
        total_staked_before: Uint128(total_staked_before),
        total_staked_after: Uint128(total_staked_after),
        amount_rebased: Uint128(amount_rebased),
        index: Uint128(index),
        block_time: block.time,
        block_height: block.height
    };
    append_rebase(store,&rebase)?;
    Ok(())
}

fn append_rebase<S: Storage>(
    store: &mut S,
    rebase: &Rebase,
) -> StdResult<()> {
    let mut store = PrefixedStorage::new(&PREFIX_REBASE, store);
    let mut store = AppendStoreMut::attach_or_create(&mut store)?;
    store.push(rebase)
}

pub fn get_rebases<S: ReadonlyStorage>(
    storage: &S,
    page: u32,
    page_size: u32,
) -> StdResult<(Vec<Rebase>, u64)> {

    let store = ReadonlyPrefixedStorage::new(&PREFIX_REBASE, storage);

    // Try to access the storage of rebase
    // If it doesn't exist yet, return an empty list of rebase.
    let store = AppendStore::<Rebase, _, _>::attach(&store);
    let store = if let Some(result) = store {
        result?
    } else {
        return Ok((vec![], 0));
    };

    // Take `page_size` rebases starting from the latest rebase potentially skipping `page * page_size`
    // rebases from the start.
    let rebases : StdResult<Vec<Rebase>> = store
        .iter()
        .rev()
        .skip((page * page_size) as _)
        .take(page_size as _).collect();

    rebases.map(|rebases| (rebases, store.len() as u64))
}