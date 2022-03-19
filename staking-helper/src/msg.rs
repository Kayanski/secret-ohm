#![allow(clippy::field_reassign_with_default)] // This is triggered in `#[derive(JsonSchema)]`

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Uint128, HumanAddr, Binary};

use crate::state::Contract;

use secret_toolkit::utils::{HandleCallback};

pub const RESPONSE_BLOCK_SIZE: usize = 256;


#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub ohm: Contract,
    pub staking: Contract,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    Stake {
        recipient: HumanAddr,
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
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    Stake {
        status: ResponseStatus,
    },

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ContractInfo{},
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    ContractInfo {
        ohm: Contract,
        staking: Contract,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingHandleMsg {
    Stake{
        recipient: HumanAddr
    },
    Claim{
        recipient: HumanAddr
    }
}

impl HandleCallback for StakingHandleMsg {
    const BLOCK_SIZE: usize = RESPONSE_BLOCK_SIZE;
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
