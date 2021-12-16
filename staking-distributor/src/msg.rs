#![allow(clippy::field_reassign_with_default)] // This is triggered in `#[derive(JsonSchema)]`

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::Contract;
use crate::viewing_key::ViewingKey;
use cosmwasm_std::{Binary, HumanAddr, StdError, StdResult, Uint128};
use secret_toolkit::permit::Permit;

use secret_toolkit::utils::{HandleCallback, Query};

/// We make sure that responses from `handle` are padded to a multiple of this size.
pub const RESPONSE_BLOCK_SIZE: usize = 256;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub treasury: Contract,
    pub ohm: Contract,
    pub epoch_length: u64,
    pub next_epoch_block: u64,
    pub admin: HumanAddr,
    pub prng_seed: Binary,
}


#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Distribute {},
    AddRecipient{
        recipient: HumanAddr,
        reward_rate: Uint128,
    },
    RemoveRecipient{
        recipient: HumanAddr
    },
    SetAdjustment{
        index: Uint128,
        add: bool,
        rate: Uint128,
        target: Uint128
    },
    ChangeAdmin {
        address: HumanAddr,
        padding: Option<String>,
    },
    RevokePermit {
        permit_name: String,
        padding: Option<String>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    Distribute {
        status: ResponseStatus,
    },
    AddRecipient{
        status: ResponseStatus,
    },
    RemoveRecipient{
        status: ResponseStatus,
    },
    SetAdjustment{
        status: ResponseStatus,
    },
    
    CreateViewingKey {
        key: ViewingKey,
    },

    // Other
    SetViewingKey {
        status: ResponseStatus,
    },

    // Other
    ChangeAdmin {
        status: ResponseStatus,
    },
    SetContractStatus {
        status: ResponseStatus,
    },

    // Permit
    RevokePermit {
        status: ResponseStatus,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo {},
    RateInfo{
        address: HumanAddr,
        key: String,
    },
    NextRewardAt { 
        rate: Uint128,
    },
    NextRewardFor{
        recipient: HumanAddr,
    },
    WithPermit {
        permit: Permit,
        query: QueryWithPermit,
    },
}

impl QueryMsg {
    pub fn get_validation_params(&self) -> (Vec<&HumanAddr>, ViewingKey) {
        match self {
            Self::RateInfo { address, key } => (vec![address], ViewingKey(key.clone())),
            _ => panic!("This query type does not require authentication"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryWithPermit {
    RateInfo {},
    NextRewardFor{},
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    ContractInfo {
        ohm: Contract,
        treasury: Contract,
        epoch_length: u64,
        next_epoch_block: u64,
        admin: HumanAddr
    },
    RateInfo{
        recipient: HumanAddr,
        rate: Uint128
    },
    NextRewardAt { 
        amount: Uint128,
    },
    NextRewardFor{
        amount: Uint128,
    },
    ViewingKeyError {
        msg: String,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct CreateViewingKeyResponse {
    pub key: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatusLevel {
    NormalRun,
    StopAllButRedeems,
    StopAll,
}


//Other contracts messages
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryHandleMsg{
     MintRewards{
        recipient : HumanAddr,
        amount : Uint128
    },
}

impl HandleCallback for TreasuryHandleMsg{
    const BLOCK_SIZE: usize = RESPONSE_BLOCK_SIZE;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OhmQueryMsg{
    GetTotalSupply{

    },
}

impl Query for OhmQueryMsg {
    const BLOCK_SIZE: usize = RESPONSE_BLOCK_SIZE;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TotalSupplyResponse{
    pub total_supply: Uint128,
}

   









pub fn status_level_to_u8(status_level: ContractStatusLevel) -> u8 {
    match status_level {
        ContractStatusLevel::NormalRun => 0,
        ContractStatusLevel::StopAllButRedeems => 1,
        ContractStatusLevel::StopAll => 2,
    }
}

pub fn u8_to_status_level(status_level: u8) -> StdResult<ContractStatusLevel> {
    match status_level {
        0 => Ok(ContractStatusLevel::NormalRun),
        1 => Ok(ContractStatusLevel::StopAllButRedeems),
        2 => Ok(ContractStatusLevel::StopAll),
        _ => Err(StdError::generic_err("Invalid state level")),
    }
}

// Take a Vec<u8> and pad it up to a multiple of `block_size`, using spaces at the end.
pub fn space_pad(block_size: usize, message: &mut Vec<u8>) -> &mut Vec<u8> {
    let len = message.len();
    let surplus = len % block_size;
    if surplus == 0 {
        return message;
    }

    let missing = block_size - surplus;
    message.reserve(missing);
    message.extend(std::iter::repeat(b' ').take(missing));
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{from_slice, StdResult};

    #[derive(Serialize, Deserialize, JsonSchema, Debug, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum Something {
        Var { padding: Option<String> },
    }

    #[test]
    fn test_deserialization_of_missing_option_fields() -> StdResult<()> {
        let input = b"{ \"var\": {} }";
        let obj: Something = from_slice(input)?;
        assert_eq!(
            obj,
            Something::Var { padding: None },
            "unexpected value: {:?}",
            obj
        );
        Ok(())
    }
}
