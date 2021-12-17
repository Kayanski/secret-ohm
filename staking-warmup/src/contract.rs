/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use cosmwasm_std::{
    to_binary, Api, Env, Extern, HumanAddr,
    HandleResponse, InitResponse, Querier, QueryResult, StdError,
    StdResult, Storage, Uint128,
};

use crate::msg::{
    HandleMsg, InitMsg, QueryAnswer, QueryMsg, HandleAnswer,
    space_pad,
    RESPONSE_BLOCK_SIZE,ResponseStatus::Success
};
use crate::state::{
    Config, Constants, ReadonlyConfig,
};

use secret_toolkit::snip20;

/// We make sure that responses from `handle` are padded to a multiple of this size.
pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    
    let mut config = Config::from_storage(&mut deps.storage);
    config.set_constants(&Constants {
        sohm: msg.sohm,
        staking: msg.staking,
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



pub fn retrieve<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    staker: HumanAddr,
    amount: u128
) -> StdResult<HandleResponse> {

    let config = ReadonlyConfig::from_storage(&deps.storage);
    if config.constants()?.staking.address != env.message.sender {
        return Err(StdError::generic_err(
            "This command can only be ran from the staking contract",
        ));
    }

    let messages = vec![
        snip20::transfer_msg(
            staker,
            Uint128(amount),
            None,
            RESPONSE_BLOCK_SIZE,
            config.constants()?.sohm.code_hash,
            config.constants()?.sohm.address,
            )?
    ];
    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Retrieve {
            status: Success,
        })?),
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {

    let response = match msg {
        HandleMsg::Retrieve{staker, amount,..} => retrieve(deps,env,staker, amount.u128()),
    };

    pad_response(response)
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::ContractInfo{ } => query_contract_info(&deps.storage),
    }
}

fn query_contract_info<S: Storage>(storage: &S) -> QueryResult{
    let consts = ReadonlyConfig::from_storage(storage).constants()?;

    to_binary(
        &QueryAnswer::ContractInfo{
            sohm:consts.sohm,
            staking: consts.staking,
        }
    )

}
