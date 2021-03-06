use std::fmt;

use cosmwasm_std::{
    to_binary, Api, BankMsg, CanonicalAddr, Coin, CosmosMsg, Env, Extern, HumanAddr, Querier,
    StdError, StdResult, Storage, Uint128, WasmMsg,
};
use schemars::JsonSchema;
use secret_toolkit::snip20::HandleMsg;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}", self.amount, self.info)
    }
}

//static DECIMAL_FRACTION: Uint128 = Uint128(1_000_000_000_000_000_000u128);

impl Asset {
    pub fn is_native_token(&self) -> bool {
        self.info.is_native_token()
    }

    pub fn compute_tax<S: Storage, A: Api, Q: Querier>(
        &self,
        _deps: &Extern<S, A, Q>,
    ) -> StdResult<Uint128> {
        // let amount = self.amount;
        // if let AssetInfo::NativeToken { denom } = &self.info {
        //     // let terra_querier = TerraQuerier::new(&deps.querier);
        //     // let tax_rate: Decimal = (terra_querier.query_tax_rate()?).rate;
        //     // let tax_cap: Uint128 = (terra_querier.query_tax_cap(denom.to_string())?).cap;
        //     Ok(std::cmp::min(
        //         (amount
        //             - amount.multiply_ratio(
        //                 DECIMAL_FRACTION,
        //                 DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
        //             ))?,
        //         tax_cap,
        //     ))
        // } else {
        //     Ok(Uint128::zero())
        // }
        Ok(Uint128::zero())
    }

    pub fn deduct_tax<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Coin> {
        let amount = self.amount;
        if let AssetInfo::NativeToken { denom } = &self.info {
            Ok(Coin {
                denom: denom.to_string(),
                amount: (amount - self.compute_tax(deps)?)?,
            })
        } else {
            Err(StdError::generic_err("cannot deduct tax from token asset"))
        }
    }

    pub fn into_msg<S: Storage, A: Api, Q: Querier>(
        self,
        deps: &Extern<S, A, Q>,
        sender: HumanAddr,
        recipient: HumanAddr,
    ) -> StdResult<CosmosMsg> {
        let amount = self.amount;

        match &self.info {
            AssetInfo::Token {
                contract_addr,
                token_code_hash,
                ..
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),

                callback_code_hash: token_code_hash.clone(),

                msg: to_binary(&HandleMsg::Send {
                    recipient,
                    amount,
                    padding: None,
                    msg: None,
                })?,
                send: vec![],
            })),
            AssetInfo::NativeToken { .. } => Ok(CosmosMsg::Bank(BankMsg::Send {
                from_address: sender,
                to_address: recipient,
                amount: vec![self.deduct_tax(deps)?],
            })),
        }
    }

    pub fn assert_sent_native_token_balance(&self, env: &Env) -> StdResult<()> {
        if let AssetInfo::NativeToken { denom } = &self.info {
            match env.message.sent_funds.iter().find(|x| x.denom == *denom) {
                Some(coin) => {
                    if self.amount == coin.amount {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
                None => {
                    if self.amount.is_zero() {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("Native token balance mismatch between the argument and the transferred"))
                    }
                }
            }
        } else {
            Ok(())
        }
    }

    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetRaw> {
        Ok(AssetRaw {
            info: match &self.info {
                AssetInfo::NativeToken { denom } => AssetInfoRaw::NativeToken {
                    denom: denom.to_string(),
                },
                AssetInfo::Token {
                    contract_addr,
                    token_code_hash,
                    viewing_key,
                } => AssetInfoRaw::Token {
                    contract_addr: deps.api.canonical_address(&contract_addr)?,
                    token_code_hash: token_code_hash.clone(),
                    viewing_key: viewing_key.clone(),
                },
            },
            amount: self.amount,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token {
        contract_addr: HumanAddr,
        token_code_hash: String,
        viewing_key: String,
    },
    NativeToken {
        denom: String,
    },
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AssetInfo::NativeToken { denom } => write!(f, "{}", denom),
            AssetInfo::Token { contract_addr, .. } => write!(f, "{}", contract_addr),
        }
    }
}

impl AssetInfo {
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetInfoRaw> {
        match self {
            AssetInfo::NativeToken { denom } => Ok(AssetInfoRaw::NativeToken {
                denom: denom.to_string(),
            }),
            AssetInfo::Token {
                contract_addr,
                viewing_key,
                token_code_hash,
            } => Ok(AssetInfoRaw::Token {
                contract_addr: deps.api.canonical_address(&contract_addr)?,
                viewing_key: viewing_key.clone(),
                token_code_hash: token_code_hash.clone(),
            }),
        }
    }

    pub fn is_native_token(&self) -> bool {
        match self {
            AssetInfo::NativeToken { .. } => true,
            AssetInfo::Token { .. } => false,
        }
    }
    pub fn equal(&self, asset: &AssetInfo) -> bool {
        match self {
            AssetInfo::Token { contract_addr, .. } => {
                let self_contract_addr = contract_addr;
                match asset {
                    AssetInfo::Token { contract_addr, .. } => self_contract_addr == contract_addr,
                    AssetInfo::NativeToken { .. } => false,
                }
            }
            AssetInfo::NativeToken { denom, .. } => {
                let self_denom = denom;
                match asset {
                    AssetInfo::Token { .. } => false,
                    AssetInfo::NativeToken { denom, .. } => self_denom == denom,
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetRaw {
    pub info: AssetInfoRaw,
    pub amount: Uint128,
}

impl AssetRaw {
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Asset> {
        Ok(Asset {
            info: match &self.info {
                AssetInfoRaw::NativeToken { denom } => AssetInfo::NativeToken {
                    denom: denom.to_string(),
                },
                AssetInfoRaw::Token {
                    contract_addr,
                    viewing_key,
                    token_code_hash,
                } => AssetInfo::Token {
                    contract_addr: deps.api.human_address(&contract_addr)?,
                    viewing_key: viewing_key.clone(),
                    token_code_hash: token_code_hash.clone(),
                },
            },
            amount: self.amount,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Factory {
    pub address: HumanAddr,
    pub code_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum AssetInfoRaw {
    Token {
        contract_addr: CanonicalAddr,
        token_code_hash: String,
        viewing_key: String,
    },
    NativeToken {
        denom: String,
    },
}

impl AssetInfoRaw {
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetInfo> {
        match self {
            AssetInfoRaw::NativeToken { denom } => Ok(AssetInfo::NativeToken {
                denom: denom.to_string(),
            }),
            AssetInfoRaw::Token {
                contract_addr,
                viewing_key,
                token_code_hash,
            } => Ok(AssetInfo::Token {
                contract_addr: deps.api.human_address(&contract_addr)?,
                viewing_key: viewing_key.clone(),
                token_code_hash: token_code_hash.clone(),
            }),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            AssetInfoRaw::NativeToken { denom } => denom.as_bytes(),
            AssetInfoRaw::Token { contract_addr, .. } => contract_addr.as_slice(),
        }
    }

    pub fn equal(&self, asset: &AssetInfoRaw) -> bool {
        match self {
            AssetInfoRaw::Token { contract_addr, .. } => {
                let self_contract_addr = contract_addr;
                match asset {
                    AssetInfoRaw::Token { contract_addr, .. } => {
                        self_contract_addr == contract_addr
                    }
                    AssetInfoRaw::NativeToken { .. } => false,
                }
            }
            AssetInfoRaw::NativeToken { denom, .. } => {
                let self_denom = denom;
                match asset {
                    AssetInfoRaw::Token { .. } => false,
                    AssetInfoRaw::NativeToken { denom, .. } => self_denom == denom,
                }
            }
        }
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolInfo {
    pub assets: [Asset; 2],
    pub total_share: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairInfo {
    pub asset_infos: [AssetInfo; 2],
    pub contract_addr: HumanAddr,
    pub liquidity_token: HumanAddr,
    pub token_code_hash: String,
    pub asset0_volume: Uint128,
    pub asset1_volume: Uint128,
    pub factory: Factory,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairInfoRaw {
    pub asset_infos: [AssetInfoRaw; 2],
    pub contract_addr: CanonicalAddr,
    pub liquidity_token: CanonicalAddr,
    pub token_code_hash: String,
    pub asset0_volume: Uint128,
    pub asset1_volume: Uint128,
    pub factory: Factory,
}

impl PairInfoRaw {
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<PairInfo> {
        Ok(PairInfo {
            liquidity_token: deps.api.human_address(&self.liquidity_token)?,
            contract_addr: deps.api.human_address(&self.contract_addr)?,
            asset_infos: [
                self.asset_infos[0].to_normal(&deps)?,
                self.asset_infos[1].to_normal(&deps)?,
            ],
            token_code_hash: self.token_code_hash.clone(),
            asset0_volume: self.asset0_volume.clone(),
            asset1_volume: self.asset1_volume.clone(),
            factory: self.factory.clone(),
        })
    }

}