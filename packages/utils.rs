use cosmwasm_std::{HumanAddr};

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Contract{
    pub address : HumanAddr,
    pub code_hash : String
}