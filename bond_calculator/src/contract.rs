/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use cosmwasm_std::{
    to_binary, Api, Env, Extern,
    HandleResponse, InitResponse, Querier, QueryResult, StdError,
    StdResult, Storage, Uint128,
};

use crate::msg::{
    HandleMsg, InitMsg, QueryAnswer, QueryMsg,
    space_pad, PairQueryMsg, PairResponse,
    RESPONSE_BLOCK_SIZE,
};
use crate::state::{
    Config, Constants, ReadonlyConfig, Contract
};

use crate::secretswap_utils::{AssetInfo, PairInfo};
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

fn sqrrt(a: u128) -> u128{
    let mut c;
    if a > 3 {
        c = a;
        let mut b = a/2+1;
        while b < c {
            c = b;
            b = (( a / b ) + b) / 2 ;
        }
    } else if a != 0 {
        c = 1;
    } else{
        c = 0;
    }
    c
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
    let pair_response: PairResponse = pair_query_msg.query(
        querier,
        pair.code_hash.clone(),
        pair.address.clone(),
    )?;

    Ok(pair_response.pair_info)
}


fn get_k_value<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> StdResult<u128>{
    //We start by querying the token pairs and volumes
    let pair_info = query_pair_info(&deps.querier,pair.clone())?;
    //We now query the decimals of each token
    let token0;
    match pair_info.asset_infos[0].clone(){
        AssetInfo::Token{ contract_addr,token_code_hash, .. } => {
            token0 = snip20::token_info_query(
                &deps.querier,
                RESPONSE_BLOCK_SIZE,
                token_code_hash,
                contract_addr,
            )?.decimals;
        }
        AssetInfo::NativeToken{..}
            => return Err(StdError::generic_err("No native token in pairs are allowed"))
    }

    let token1;
    match pair_info.asset_infos[1].clone(){
        AssetInfo::Token{ contract_addr,token_code_hash, .. } => {
            token1 = snip20::token_info_query(
                &deps.querier,
                RESPONSE_BLOCK_SIZE,
                token_code_hash,
                contract_addr,
            )?.decimals;
        }
        AssetInfo::NativeToken{..}
            => return Err(StdError::generic_err("No native token in pairs are allowed"))
    }
    let reserve0 = pair_info.asset0_volume.u128();
    let reserve1 = pair_info.asset1_volume.u128();

    let pair_decimals = snip20::token_info_query(
                            &deps.querier,
                            RESPONSE_BLOCK_SIZE,
                            pair_info.token_code_hash,
                            pair_info.liquidity_token,
                        )?.decimals;
    let decimals = token0 + token1 - pair_decimals;

    Ok(reserve0*reserve1/(10u128.pow(decimals.into())))
}

fn query_k_value<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> QueryResult{
    to_binary(&QueryAnswer::GetKValue{
        value: Uint128(get_k_value(deps,pair)?)
    })  
}

fn get_total_value<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    pair: Contract
) -> StdResult<u128>{
    Ok(sqrrt(get_k_value(deps,pair)?)*2)
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

    Ok(total_value*amount/total_supply / (10u128.pow(18u32)))
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


    let pair_info = query_pair_info(&deps.querier,pair.clone())?;
    let reserve0 = pair_info.asset0_volume.u128();
    let reserve1 = pair_info.asset1_volume.u128();

    let reserve; 
    let token0 = match pair_info.asset_infos[1].clone(){
        AssetInfo::Token{ contract_addr, .. } => {
            contract_addr
        }
        AssetInfo::NativeToken{..}
            => return Err(StdError::generic_err("No native token in pairs are allowed"))
    };


    if token0 == ohm.address{
        reserve = reserve1;
    } else {
        reserve = reserve0;
    }
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
    to_binary(&QueryAnswer::Valuation{
        value: Uint128(get_markdown(deps,pair)?),
    })  
}
