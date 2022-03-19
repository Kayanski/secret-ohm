/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use cosmwasm_std::{
    to_binary, Api, Env, Extern, HumanAddr,
    HandleResponse, InitResponse, Querier, QueryResult, StdError,
    StdResult, Storage, Uint128, Binary, from_binary
};

use crate::msg::{
    HandleMsg, InitMsg, QueryAnswer, QueryMsg, HandleAnswer,
    space_pad,
    RESPONSE_BLOCK_SIZE,ResponseStatus::Success, StakingHandleMsg, ReceiveMsg
};
use crate::state::{
    Config, Constants, ReadonlyConfig
};

use secret_toolkit::{snip20, utils::HandleCallback};

/// We make sure that responses from `handle` are padded to a multiple of this size.
pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    
    let mut config = Config::from_storage(&mut deps.storage);
    config.set_constants(&Constants {
        ohm: msg.ohm.clone(),
        staking: msg.staking,
    })?;

    // We need to register the contract with the ohm token
    let messages = vec![
        snip20::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            RESPONSE_BLOCK_SIZE,
            msg.ohm.code_hash.clone(),
            msg.ohm.address.clone(),
        )?,
    ];

    Ok(InitResponse {
        messages,
        log: vec![],
    })
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


pub fn stake<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    recipient: HumanAddr,
    amount: u128,
) -> StdResult<HandleResponse> {

    let config = ReadonlyConfig::from_storage(&deps.storage);
    let consts = config.constants()?;

    let send_callback_msg = StakingHandleMsg::Stake{
        recipient: recipient.clone()
    };

    // We send the claim message from the warmup contract
    let claim_msg = StakingHandleMsg::Claim {
        recipient: recipient.clone(),
    };

    let messages = vec![
        // Start by sending funds to the staking contract for staking
        snip20::send_msg(
            consts.staking.address.clone(),
            Uint128(amount),
            Some(to_binary(&send_callback_msg)?),
            None,
            RESPONSE_BLOCK_SIZE,
            consts.ohm.code_hash,
            consts.ohm.address,
        )?,
        // Then claim the funds from warmup
        claim_msg.to_cosmos_msg(
            consts.staking.code_hash,
            consts.staking.address,
            None,
        )?,
    ];


    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Stake {
            status: Success,
        })?),
    })
}

pub fn receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    _from: HumanAddr,
    amount: u128,
    msg: Binary,
) -> StdResult<HandleResponse> {
    let msg: ReceiveMsg = from_binary(&msg)?;
    let token = env.message.sender.clone();
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    match msg {
        ReceiveMsg::Stake { recipient, .. } => {
            if token == consts.ohm.address {
                stake(deps, env, recipient, amount)
            } else {
                Err(StdError::generic_err(
                    "You can't stake anything else than the treasury token",
                ))
            }
        }
    }
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {

    let response = match msg {
        HandleMsg::Receive {
            from, amount, msg, ..
        } =>receive(deps, env, from, amount.u128(), msg),
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
            ohm:consts.ohm,
            staking: consts.staking,
        }
    )

}
