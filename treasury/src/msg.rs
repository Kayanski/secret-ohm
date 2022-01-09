#![allow(clippy::field_reassign_with_default)] // This is triggered in `#[derive(JsonSchema)]`

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, HumanAddr, StdError, StdResult, Uint128};
use crate::state::{Contract, ManagingRole, Constants};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct InitialBalance {
    pub address: HumanAddr,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub name: String,
    pub admin: Option<HumanAddr>,
    pub prng_seed: Binary,
    pub ohm : Contract,
    pub sohm : Contract,
    pub reserve_tokens : Option<Vec<Contract>>,
    pub liquidity_tokens : Option<Vec<Contract>>,
    pub blocks_needed_for_queue : u64
}


#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg{
    Deposit{
        profit : Uint128
    },
    RepayDebt{
        
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
    Withdraw{
        token : HumanAddr,
        amount : Uint128
    },
    IncurDebt{
        token : HumanAddr,
        amount : Uint128
    },
    Manage{
        token : HumanAddr,
        amount : Uint128
    },
    MintRewards{
        token : HumanAddr,
        amount : Uint128
    },
    AuditReserves{

    },
    Queue{
        address : HumanAddr,
        role : ManagingRole
    },
    ToggleQueue{
        address : HumanAddr,
        role : ManagingRole
    },
    ToggleTokenQueue{
        token : Contract,
        role : ManagingRole,
        calculator: Option<Contract>
    },

    // Admin
    ChangeAdmin {
        address: HumanAddr,
        padding: Option<String>,
    },
    SetContractStatus {
        level: ContractStatusLevel,
        padding: Option<String>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveAnswer {
    Deposit {
        status: ResponseStatus,
    },
    RepayDebt {
        status: ResponseStatus,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {

    Deposit {
        status: ResponseStatus,
    },

    Withdraw {
        status: ResponseStatus,
    },

    IncurDebt {
        status: ResponseStatus,
    },

    Manage {
        status: ResponseStatus,
    },

    MintRewards {
        status: ResponseStatus,
    },

    AuditReserves {
        status: ResponseStatus,
    },

    QueueAddress {
        status: ResponseStatus,
    },

    ToggleQueue {
        status: ResponseStatus,
    },

    // Other
    ChangeAdmin {
        status: ResponseStatus,
    },
    SetContractStatus {
        status: ResponseStatus,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo {},
    Contracts {
        role: ManagingRole
    },
    ManagingAddresses {
        role: ManagingRole
    },
    ContractStatus {},
    ValueOf{
        token: HumanAddr,
        amount: Uint128
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    ContractInfo{
        name: String,
        admin: HumanAddr,
        prng_seed: Vec<u8>,
        ohm : Contract,
        sohm : Contract,
        blocks_needed_for_queue : u64,
    },
    TokensInfo{
        tokens : Vec<Contract>
    },
    ManagersInfo{
        addresses : Vec<HumanAddr>
    },
    ContractStatus {
        status: ContractStatusLevel,
    },
    ValueOf{
        value: Uint128,
    }
}
pub fn contract_info_from_constants(constants : &Constants) -> QueryAnswer{
    QueryAnswer::ContractInfo{
        name : constants.name.clone(),
        admin : constants.admin.clone(),
        prng_seed : constants.prng_seed.clone(),
        ohm : constants.ohm.clone(),
        sohm : constants.sohm.clone(),
        blocks_needed_for_queue : constants.blocks_needed_for_queue.clone(),
    }
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
