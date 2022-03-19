#![allow(clippy::field_reassign_with_default)] // This is triggered in `#[derive(JsonSchema)]`

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Contract, Principle, Adjust, Terms};
use crate::viewing_key::ViewingKey;
use cosmwasm_std::{Binary, HumanAddr, StdError, StdResult, Uint128};
use secret_toolkit::permit::Permit;

use secret_toolkit::utils::{HandleCallback, Query};

/// We make sure that responses from `handle` are padded to a multiple of this size.
pub const RESPONSE_BLOCK_SIZE: usize = 256;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub name: String,
    pub symbol: String,
    pub ohm: Contract,
    pub principle: Principle,
    pub treasury: Contract,
    pub dao: HumanAddr,
    pub bond_calculator: Option<Contract>,
    pub admin: Option<HumanAddr>,
    pub prng_seed: Binary,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum BondParameter { Vesting, Payout, Fee, Debt } 

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg{
    Deposit{
        max_price: Uint128,
        depositor: Option<HumanAddr>,
    },
}
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive{
        sender: HumanAddr,
        from: HumanAddr,
        amount: Uint128,
        msg: Binary,
    },
    InitializeBondTerms{
        control_variable: Uint128, 
        vesting_term: u64,
        minimum_price: Uint128,
        maximum_price: Uint128,
        max_payout: Uint128,
        fee: Uint128,
        max_debt: Uint128, 
        initial_debt: Uint128, 
    },
    SetBondTerm{
        parameter: BondParameter,
        input: Uint128,
    },
    SetAdjustment{
        addition: bool,
        increment: Uint128,
        target: Uint128,
        buffer: u64,
    },
    SetStaking{
        staking: Contract,
    },
    Redeem{
        recipient:  HumanAddr,
        stake: bool,
    },
    RecoverLostToken{
        token: Contract
    },
    ChangeAdmin {
        address: HumanAddr,
        padding: Option<String>,
    },
    RevokePermit {
        permit_name: String,
        padding: Option<String>,
    },  
    CreateViewingKey {
        entropy: String,
        padding: Option<String>,
    },
    SetViewingKey {
        key: String,
        padding: Option<String>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {

    InitializeBondTerms{
        status: ResponseStatus,
    },
    SetBondTerms{
        status: ResponseStatus,
    },
    SetAdjustment{
        status: ResponseStatus,
    },
    SetStaking{
        status: ResponseStatus,
    },
    Deposit{
        status: ResponseStatus,
        payout: Uint128
    },
    Redeem{
        status: ResponseStatus,
        payout: Uint128
    },
    StakeOrSend{
        status: ResponseStatus,
    },
    RecoverLostToken{
        status: ResponseStatus,
    },



    AddRecipient{
        status: ResponseStatus,
    },
    RemoveRecipient{
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
    TokenInfo {},
    ContractInfo {},
    MaxPayout{},
    PayoutFor{
        block_height: u64,
        value: Uint128
    },
    BondPrice{
        block_height: u64,
    },
    BondPriceInUsd{
        block_height: u64,
    },
    DebtRatio{
        block_height: u64,
    },
    StandardizedDebtRatio{
        block_height: u64,
    },
    CurrentDebt{
        block_height: u64,
    },
    DebtDecay{
        block_height: u64,
    },
    BondInfo{
        address: HumanAddr,
        key: String,
    },
    PercentVestedFor{
        address: HumanAddr,
        block_height: u64,
        key: String,
    },
    PendingPayoutFor{
        address: HumanAddr,
        block_height: u64,
        key: String,
    }, 
    WithPermit {
        permit: Permit,
        query: QueryWithPermit,
    },
    Balance {
        address: HumanAddr,
        key: String,
    },
    BondTerms{},
}

impl QueryMsg {
    pub fn get_validation_params(&self) -> (Vec<&HumanAddr>, ViewingKey) {
        match self {
            Self::Balance { address, key } => (vec![address], ViewingKey(key.clone())),
            Self::BondInfo { address, key } => (vec![address], ViewingKey(key.clone())),
            Self::PercentVestedFor { address, key, .. } => (vec![address], ViewingKey(key.clone())),
            Self::PendingPayoutFor { address, key, .. } => (vec![address], ViewingKey(key.clone())),
            _ => panic!("This query type does not require authentication"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryWithPermit { 
    BondInfo{},
    PercentVestedFor{
        block_height: u64
    },
    PendingPayoutFor{
        block_height: u64
    }, 
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    TokenInfo {
        name: String,
        symbol: String,
        decimals: u8,
        total_supply: Option<Uint128>,
    },
    ContractInfo {
        ohm: Contract,
        principle: Principle,
        treasury: Contract,
        dao: HumanAddr,
        bond_calculator: Option<Contract>,

        staking: Option<Contract>,

        terms: Option<Terms>,
        adjustment: Option<Adjust>,

        admin: HumanAddr,
        total_debt: Uint128,
        last_decay: u64
    },
    MaxPayout{
        payout: Uint128
    },
    PayoutFor{
        payout: Uint128
    },
    BondPrice{
        price: Uint128
    },
    BondPriceInUsd{
        price: Uint128
    },
    DebtRatio{
        ratio: Uint128
    },
    StandardizedDebtRatio{
        ratio: Uint128
    },
    CurrentDebt{
        debt: Uint128
    },
    DebtDecay{
        decay: Uint128
    },
    Bond{
        payout: Uint128, 
        vesting: u64, 
        last_block: u64, 
        price_paid: Uint128, 
    },
    PercentVestedFor{
        percent: Uint128
    },
    PendingPayoutFor{
        payout: Uint128
    },
    ViewingKeyError {
        msg: String,
    },
    Balance {
        amount: Uint128,
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
    Deposit{
        profit: Uint128,
    }
}

impl HandleCallback for TreasuryHandleMsg{
    const BLOCK_SIZE: usize = RESPONSE_BLOCK_SIZE;
}

//Other contracts messages
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingHandleMsg{
    Stake{
        recipient: HumanAddr,
    }
}

impl HandleCallback for StakingHandleMsg{
    const BLOCK_SIZE: usize = RESPONSE_BLOCK_SIZE;
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BondCalculatorQueryMsg{
    Markdown{
        pair: Contract
    },
}

impl Query for BondCalculatorQueryMsg {
    const BLOCK_SIZE: usize = RESPONSE_BLOCK_SIZE;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Markdown{
    pub value: Uint128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MarkdownResponse{
    pub markdown: Markdown,
}

   

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryQueryMsg{
    ValueOf{
        token: HumanAddr,
        amount: Uint128
    },
}

impl Query for TreasuryQueryMsg {
    const BLOCK_SIZE: usize = RESPONSE_BLOCK_SIZE;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ValueOf{
    pub value: Uint128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ValueOfResponse{
    pub value_of: ValueOf,
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
