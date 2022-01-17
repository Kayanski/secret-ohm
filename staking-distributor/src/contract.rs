/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use std::convert::TryInto;
use cosmwasm_std::{
    to_binary, Api, Env, Extern,
    HandleResponse, HumanAddr, InitResponse, Querier, QueryResult, ReadonlyStorage, StdError,
    StdResult, Storage, Uint128,
};

use crate::msg::{
    space_pad, HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg,
    ResponseStatus::Success, ResponseStatus::Failure, TreasuryHandleMsg, 
    RESPONSE_BLOCK_SIZE
};
use crate::rand::sha_256;
use crate::state::{
    Config, Constants, ReadonlyConfig,
    Info, Adjust,
};
use secret_toolkit::snip20;
use secret_toolkit::utils::{HandleCallback};

pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {

    let mut config = Config::from_storage(&mut deps.storage);

    let admin = msg.admin.unwrap_or(env.message.sender);
    let prng_seed_hashed = sha_256(&msg.prng_seed.0);

    config.set_constants(&Constants {
        treasury: msg.treasury.clone(),
        ohm: msg.ohm.clone(),
        epoch_length: msg.epoch_length.clone(),
        next_epoch_block: msg.next_epoch_block.clone(),
        admin: admin,
        prng_seed: prng_seed_hashed.to_vec(),
        contract_address: env.contract.address,
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
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {

    let response = match msg {

        HandleMsg::Distribute {} => distribute(deps, env),
        HandleMsg::AddRecipient { recipient, reward_rate } => add_recipient(deps, env, recipient, reward_rate.u128()),
        HandleMsg::RemoveRecipient { recipient } => remove_recipient(deps, env, recipient),
        HandleMsg::SetAdjustment { index, add, rate, target } => set_adjustment(deps, env, index.u128().try_into().unwrap(), add, rate.u128(), target.u128()),

        // Other
        HandleMsg::ChangeAdmin { address, .. } => change_admin(deps, env, address),
    };

    pad_response(response)
}

pub fn distribute<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let config = Config::from_storage(&mut deps.storage);
    let mut consts = config.constants()?;
    let mut messages = vec![];
    let data;
    if consts.next_epoch_block <= env.block.height{
        consts.next_epoch_block += consts.epoch_length;
        let rate_info = config.rate_info();
        for (i,info) in rate_info.iter().enumerate(){
            if info.rate > 0{
                //Mint rewards message
                let mint_rewards_msg = TreasuryHandleMsg::MintRewards{
                    recipient: info.recipient.clone(),
                    amount: Uint128(next_reward_at(deps,info.rate)?),
                };
                messages.push(
                    mint_rewards_msg.to_cosmos_msg( 
                        consts.treasury.code_hash.clone(),
                        consts.treasury.address.clone(),
                        None
                    )?
                );
                adjust(deps,i)?;
            }
        }
        data = Some(to_binary(&HandleAnswer::Distribute { status: Success })?);
    }else{
        data = Some(to_binary(&HandleAnswer::Distribute { status: Failure })?);
    }

    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: data
    })
}
pub fn adjust<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    index: usize
) -> StdResult<()>{
    let mut config = Config::from_storage(&mut deps.storage);
    let mut info = config.rate_info();
    let mut adjustment = config.adjustment(index).unwrap_or_default();
    if adjustment.rate != 0{
        if adjustment.add{
            info[index].rate += adjustment.rate;
            if info[index].rate >= adjustment.target{
                adjustment.rate = 0;
                info[index].rate = adjustment.target;
            }
        }else{
            info[index].rate -= adjustment.rate;
            if info[index].rate <= adjustment.target{
                adjustment.rate = 0;
                info[index].rate = adjustment.target;
            }
        }
        config.set_adjustment(index,adjustment)?;
    }
    config.set_info(info)
}

pub fn add_recipient<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: HumanAddr,
    rate: u128
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    config.add_info(vec![Info{
        recipient,
        rate
    }])?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::AddRecipient{ status: Success })?),
    })
}

pub fn remove_recipient<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: HumanAddr
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    config.remove_info(recipient)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::RemoveRecipient{ status: Success })?),
    })
}

pub fn set_adjustment<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    index: usize,
    add: bool,
    rate: u128,
    target: u128
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    config.set_adjustment(index,Adjust{
        add,
        rate,
        target,
    })?;    

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetAdjustment{ status: Success })?),
    })
}



pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::ContractInfo {} => query_contract_info(&deps.storage),
        QueryMsg::NextRewardAt {rate} => query_next_reward_at(deps,rate.u128()),
        QueryMsg::RateInfo { address } => query_rate_info(&deps.storage, address),
        QueryMsg::NextRewardFor { recipient } => query_next_reward_for(deps, recipient)
    }
}

fn next_reward_at<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, rate: u128) -> StdResult<u128>{
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let total_supply_response = snip20::token_info_query(
        &deps.querier,
        RESPONSE_BLOCK_SIZE,
        consts.ohm.code_hash.clone(),
        consts.ohm.address.clone(),
    )?;
    total_supply_response.total_supply.unwrap_or_default().u128()
    .checked_mul(rate).ok_or_else(||{
        StdError::generic_err("The reward rate is too high, sorry")
    })?
    .checked_div(1_000_000).ok_or_else(||{
        StdError::generic_err("This error is unreachable, how did you do it ?")
    })
}

fn next_reward_for<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, recipient: HumanAddr) -> StdResult<u128>{
    let config = ReadonlyConfig::from_storage(&deps.storage);
    next_reward_at(deps,config.info_by_recipient(recipient)?.rate)
}

fn query_contract_info<S: ReadonlyStorage>(storage: &S) -> QueryResult {
    let config = ReadonlyConfig::from_storage(storage);
    let constants = config.constants()?;

    to_binary(&QueryAnswer::ContractInfo {
        treasury: constants.treasury,
        ohm: constants.ohm,
        epoch_length: constants.epoch_length,
        next_epoch_block: constants.next_epoch_block,
        admin: constants.admin
    })
}

fn query_rate_info<S: ReadonlyStorage>(storage: &S, recipient: HumanAddr) -> QueryResult {
    let config = ReadonlyConfig::from_storage(storage);

    to_binary(&QueryAnswer::RateInfo {
        recipient: recipient.clone(),
        rate: Uint128(config.info_by_recipient(recipient)?.rate)
    })
}


fn query_next_reward_at<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, rate: u128) -> QueryResult{
    to_binary(&QueryAnswer::NextRewardAt {
        amount: Uint128(next_reward_at(deps,rate)?),
    })
}

fn query_next_reward_for<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, recipient: HumanAddr) -> QueryResult{
    to_binary(&QueryAnswer::NextRewardFor {
        amount: Uint128(next_reward_for(deps,recipient)?),
    })
}

fn change_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    let mut consts = config.constants()?;
    consts.admin = address;
    config.set_constants(&consts)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ChangeAdmin { status: Success })?),
    })
}

fn is_admin<S: Storage>(config: &Config<S>, account: &HumanAddr) -> StdResult<bool> {
    let consts = config.constants()?;
    if &consts.admin != account {
        return Ok(false);
    }

    Ok(true)
}

fn check_if_admin<S: Storage>(config: &Config<S>, account: &HumanAddr) -> StdResult<()> {
    if !is_admin(config, account)? {
        return Err(StdError::generic_err(
            "This is an admin command. Admin commands can only be run from admin address",
        ));
    }

    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::ResponseStatus;
    use cosmwasm_std::testing::*;
    use cosmwasm_std::{from_binary, QueryResponse, Binary};
    use crate::state::{Contract};
    use std::any::Any;

    // Helper functions

    fn init_helper(
        
    ) -> (
        StdResult<InitResponse>,
        Extern<MockStorage, MockApi, MockQuerier>,
    ) {
        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("admin", &[]);

        let init_msg = InitMsg {
            treasury: Contract{address:HumanAddr("treasury".to_string()),code_hash:"Complicated_hash".to_string()},
            ohm: Contract{address:HumanAddr("ohm".to_string()),code_hash:"Complicated_hash".to_string()},
            epoch_length: 235,
            next_epoch_block: 459,
            admin: None,
            prng_seed: Binary::from("lolz fun yay".as_bytes()),
        };

        (init(&mut deps, env, init_msg), deps)
    }
    fn extract_error_msg<T: Any>(error: StdResult<T>) -> String {
        match error {
            Ok(response) => {
                let bin_err = (&response as &dyn Any)
                    .downcast_ref::<QueryResponse>()
                    .expect("An error was expected, but no error could be extracted");
                match from_binary(bin_err).unwrap() {
                    QueryAnswer::ViewingKeyError { msg } => msg,
                    _ => panic!("Unexpected query answer"),
                }
            }
            Err(err) => match err {
                StdError::GenericErr { msg, .. } => msg,
                _ => panic!("Unexpected result from init"),
            },
        }
    }

    fn ensure_success(handle_result: HandleResponse) -> bool {
        let handle_result: HandleAnswer = from_binary(&handle_result.data.unwrap()).unwrap();

        match handle_result {
            HandleAnswer::Distribute { status }
            | HandleAnswer::AddRecipient{ status }
            | HandleAnswer::RemoveRecipient{ status }
            | HandleAnswer::SetAdjustment{ status }
            | HandleAnswer::ChangeAdmin { status }
            | HandleAnswer::SetContractStatus { status } => {
                matches!(status, ResponseStatus::Success { .. })
            },
            _ => panic!(
                "HandleAnswer not supported for success extraction: {:?}",
                handle_result
            ),
        }
    }

     #[test]
    fn test_init_sanity() {
        let (init_result, deps) = init_helper();
        assert_eq!(init_result.unwrap(), InitResponse::default());

        let config = ReadonlyConfig::from_storage(&deps.storage);
        let consts = config.constants().unwrap();
        assert_eq!(consts.treasury,Contract{address:HumanAddr("treasury".to_string()),code_hash:"Complicated_hash".to_string()});
        assert_eq!(consts.ohm,Contract{address:HumanAddr("ohm".to_string()),code_hash:"Complicated_hash".to_string()});
        assert_eq!(consts.epoch_length, 235);
        assert_eq!(consts.next_epoch_block, 459);
        assert_eq!(consts.admin, HumanAddr("admin".to_string()));
        assert_eq!(
            consts.prng_seed,
            sha_256("lolz fun yay".to_owned().as_bytes())
        );
    }

    #[test]
    fn test_distributor_contract(){
        let (init_result, mut deps) = init_helper();
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );
        let handle_msg = HandleMsg::AddRecipient {
            recipient : HumanAddr("gloubi".to_string()),
            reward_rate : Uint128(125),
        };

        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
        let result = handle_result.unwrap();

        let config = ReadonlyConfig::from_storage(&deps.storage);
        println!("{:?}",config.rate_info());
        //println!("{:?}",from_binary::<HandleAnswer>(&result.data.clone().unwrap()).unwrap());
        assert!(ensure_success(result));
    }
}