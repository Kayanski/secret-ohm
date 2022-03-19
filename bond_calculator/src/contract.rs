/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use cosmwasm_std::{
    to_binary, Api, Env, Extern,
    HandleResponse, InitResponse, Querier, QueryResult, StdError,
    StdResult, Storage, Uint128,
};

use crate::msg::{
    HandleMsg, InitMsg, QueryAnswer, QueryMsg,
    space_pad, PairQueryMsg, 
    RESPONSE_BLOCK_SIZE,
};
use crate::state::{
    Config, Constants, ReadonlyConfig, Contract
};
use primitive_types::U256;

use crate::secretswap_utils::{AssetInfo, PairInfo, PoolInfo};
use secret_toolkit::snip20;

use secret_toolkit::utils::{Query};
/// We make sure that responses from `handle` are padded to a multiple of this size.
pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    
    let mut config = Config::from_storage(&mut deps.storage);
    config.set_constants(&Constants {
        ohm: msg.ohm,
    })?;

    Ok(InitResponse::default())
}

fn pad_response(response: StdResult<HandleResponse>) -> StdResult<HandleResponse> {
    response.map(|mut response| {
        response.data = response.data.map(|mut data| {
            space_pad(RESPONSE_BLOCK_SIZE, &mut data.0);
            data
        });
        response
    })
}



pub fn handle<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: HandleMsg,
) -> StdResult<HandleResponse> {


    pad_response(Err(StdError::generic_err("No Handle commands for this contract")))
}

fn sqrrt(a: U256) -> u128{
    let mut c;
    let two = U256::from(2);
    let three = U256::from(3);
    if a > three {
        c = a;
        let mut b = a / two + U256::one();
        while b < c {
            c = b;
            b = (( a / b ) + b) / two ;
        }
    } else if a != U256::zero() {
        c = U256::one();
    } else{
        c = U256::zero();
    }
    c.as_u128()
}



pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::GetKValue { pair } => query_k_value(deps,pair),
        QueryMsg::GetTotalValue { pair } => query_total_value(deps,pair),
        QueryMsg::Valuation { pair, amount } => query_valuation(deps,pair, amount.u128()),
        QueryMsg::Markdown { pair } => query_markdown(deps,pair),
    }
}

fn query_pair_info<Q: Querier>(querier: &Q, pair: Contract) -> StdResult<PairInfo>{
    let pair_query_msg = PairQueryMsg::Pair{};
    let pair_response: PairInfo = pair_query_msg.query(
        querier,
        pair.code_hash.clone(),
        pair.address.clone(),
    )?;

    Ok(pair_response)
}

fn query_pool_info<Q: Querier>(querier: &Q, pair: Contract) -> StdResult<PoolInfo>{
    let pool_query_msg = PairQueryMsg::Pool{};
    let pool_response: PoolInfo = pool_query_msg.query(
        querier,
        pair.code_hash.clone(),
        pair.address.clone(),
    )?;

    Ok(pool_response)
}


fn get_k_value<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> StdResult<U256>{

    let pool_info = query_pool_info(&deps.querier,pair.clone())?;

    // We query the volumes
    let reserve0 = U256::from(pool_info.assets[0].amount.u128());
    let reserve1 = U256::from(pool_info.assets[1].amount.u128());

    Ok(reserve0*reserve1)
}

fn query_k_value<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> QueryResult{
    to_binary(&QueryAnswer::GetKValue{
        value: U256::to_string(&get_k_value(deps,pair)?),
    })  
}

fn get_total_value<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> StdResult<u128>{
    let ohm = ReadonlyConfig::from_storage(&deps.storage).constants()?.ohm;

    // We query the decimals of the tokens
    let pool_info = query_pool_info(&deps.querier,pair.clone())?;

    // We first query decimals for the two tokens : 
    let decimals = pool_info.assets.iter().map(|x| 
        match x.info.clone(){
            AssetInfo::Token{ contract_addr, token_code_hash, .. } => {
                Ok(
                    snip20::token_info_query(
                        &deps.querier,
                        RESPONSE_BLOCK_SIZE,
                        token_code_hash,
                        contract_addr,
                    )?.decimals
                )
            }
            AssetInfo::NativeToken{..}
                => return Err(StdError::generic_err("No native token in pairs are allowed"))
        }
    ).collect::<StdResult<Vec<u8>>>()?;

    let token0 = match pool_info.assets[0].info.clone(){
        AssetInfo::Token{ contract_addr, .. } => {
            contract_addr
        }
        AssetInfo::NativeToken{..}
            => return Err(StdError::generic_err("No native token in pairs are allowed"))
    };


    let (ohm_decimals, other_decimals) = if token0 == ohm.address{
        (U256::from(decimals[0]),U256::from(decimals[1]))
    }else{
        (U256::from(decimals[1]),U256::from(decimals[0]))
    };

    let mut k = get_k_value(deps,pair)?;
    if ohm_decimals > other_decimals{
        k *= U256::from(10).pow(U256::from(ohm_decimals - other_decimals))
    }else{
        k /= U256::from(10).pow(U256::from(other_decimals - ohm_decimals))
    }

    Ok(sqrrt(k)*2)
}

fn query_total_value<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> QueryResult{
    to_binary(&QueryAnswer::GetTotalValue{
        value: Uint128(get_total_value(deps,pair)?)
    })  
}

fn get_valuation<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract,
    amount: u128
) -> StdResult<u128>{
    let total_value = get_total_value(deps,pair.clone())?;

    let pair_info = query_pair_info(&deps.querier,pair.clone())?;

    let total_supply = snip20::token_info_query(
                &deps.querier,
                RESPONSE_BLOCK_SIZE,
                pair_info.token_code_hash,
                pair_info.liquidity_token,
            )?.total_supply.unwrap().u128();

    Ok(total_value*amount/total_supply)
}

fn query_valuation<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract,
    amount: u128
) -> QueryResult{
    to_binary(&QueryAnswer::Valuation{
        value: Uint128(get_valuation(deps,pair,amount)?),
    })  
}

fn get_markdown<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> StdResult<u128>{ 
    let ohm = ReadonlyConfig::from_storage(&deps.storage).constants()?.ohm;

    let pool_info = query_pool_info(&deps.querier,pair.clone())?;

    let reserve0 = pool_info.assets[0].amount.u128();
    let reserve1 = pool_info.assets[1].amount.u128();

    let token0 = match pool_info.assets[0].info.clone(){
        AssetInfo::Token{ contract_addr, .. } => {
            contract_addr
        }
        AssetInfo::NativeToken{..}
            => return Err(StdError::generic_err("No native token in pairs are allowed"))
    };


    let reserve = if token0 == ohm.address{
        reserve1
    } else {
        reserve0
    };

    let ohm_decimals = snip20::token_info_query(
        &deps.querier,
        RESPONSE_BLOCK_SIZE,
        ohm.code_hash,
        ohm.address
        )?.decimals;
    Ok(reserve * 2 * 10u128.pow(ohm_decimals.into()) / get_total_value(deps,pair)?)
}

fn query_markdown<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> QueryResult{
    to_binary(&QueryAnswer::Markdown{
        value: Uint128(get_markdown(deps,pair)?),
    })  
}
