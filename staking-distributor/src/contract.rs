/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use std::convert::TryInto;
use cosmwasm_std::{
    to_binary, Api, Binary, Env, Extern,
    HandleResponse, HumanAddr, InitResponse, Querier, QueryResult, ReadonlyStorage, StdError,
    StdResult, Storage, Uint128,
};

use crate::msg::QueryWithPermit;
use crate::msg::{
    space_pad, HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg,
    ResponseStatus::Success, ResponseStatus::Failure, TreasuryHandleMsg, OhmQueryMsg, 
    TotalSupplyResponse,
    RESPONSE_BLOCK_SIZE
};
use crate::rand::sha_256;
use crate::state::{
    read_viewing_key, 
    write_viewing_key, Config, Constants, ReadonlyConfig,
    Info, Adjust
};
use crate::viewing_key::{ViewingKey, VIEWING_KEY_SIZE};
use secret_toolkit::permit::{validate, Permission, Permit, RevokedPermits};
use secret_toolkit::utils::{HandleCallback, Query};

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
        HandleMsg::RevokePermit { permit_name, .. } => revoke_permit(deps, env, permit_name),
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
    let mut adjustment = config.adjustment(index);
    if adjustment.rate != 0{
        if adjustment.add{
            info[index].rate += adjustment.rate;
            if info[index].rate >= adjustment.target{
                adjustment.rate = 0;
            }
        }else{
            info[index].rate -= adjustment.rate;
            if info[index].rate <= adjustment.target{
                adjustment.rate = 0;
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
        QueryMsg::WithPermit { permit, query } => permit_queries(deps, permit, query),
        _ => viewing_keys_queries(deps, msg),
    }
}

fn next_reward_at<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, rate: u128) -> StdResult<u128>{
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let total_supply_query_msg = OhmQueryMsg::GetTotalSupply{};
    let total_supply_response: TotalSupplyResponse = total_supply_query_msg.query(
        &deps.querier,
        consts.ohm.code_hash.clone(),
        consts.ohm.address.clone(),
    )?;
    total_supply_response.total_supply.u128()
    .checked_mul(rate).ok_or_else(||{
        StdError::generic_err("The reward rate is too high, sorry")
    })?
    .checked_div(1_000_000).ok_or_else(||{
        StdError::generic_err("This error is unreachable, how did you do it ?")
    })
}

fn next_reward_for<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, recipient: HumanAddr) -> StdResult<u128>{
    let config = ReadonlyConfig::from_storage(&deps.storage);
    next_reward_at(deps,config.info_by_recipient(recipient).rate)
}


fn permit_queries<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    permit: Permit,
    query: QueryWithPermit,
) -> Result<Binary, StdError> {
    // Validate permit content
    let contract_address = ReadonlyConfig::from_storage(&deps.storage)
        .constants()?
        .contract_address;

    let account = validate(deps, PREFIX_REVOKED_PERMITS, &permit, contract_address)?;

    // Permit validated! We can now execute the query.
    match query {
        QueryWithPermit::RateInfo {} => {
            if !permit.check_permission(&Permission::Balance) {
                return Err(StdError::generic_err(format!(
                    "No permission to query rate, got permissions {:?}",
                    permit.params.permissions
                )));
            }

            query_rate_info(&deps.storage, account)
        }
        QueryWithPermit::NextRewardFor {} => {
            if !permit.check_permission(&Permission::Balance) {
                return Err(StdError::generic_err(format!(
                    "No permission to query rate, got permissions {:?}",
                    permit.params.permissions
                )));
            }

            query_next_reward_for(deps, account)
        }
    }
}

pub fn viewing_keys_queries<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> QueryResult {
    let (addresses, key) = msg.get_validation_params();

    for address in addresses {
        let canonical_addr = deps.api.canonical_address(address)?;

        let expected_key = read_viewing_key(&deps.storage, &canonical_addr);

        if expected_key.is_none() {
            // Checking the key will take significant time. We don't want to exit immediately if it isn't set
            // in a way which will allow to time the command and determine if a viewing key doesn't exist
            key.check_viewing_key(&[0u8; VIEWING_KEY_SIZE]);
        } else if key.check_viewing_key(expected_key.unwrap().as_slice()) {
            return match msg {
                QueryMsg::RateInfo { address, .. } => query_rate_info(&deps.storage, address),
                QueryMsg::NextRewardFor { recipient, .. } => query_next_reward_for(deps, recipient),
                _ => panic!("This query type does not require authentication"),
            };
        }
    }

    to_binary(&QueryAnswer::ViewingKeyError {
        msg: "Wrong viewing key for this address or viewing key not set".to_string(),
    })
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
        rate: Uint128(config.info_by_recipient(recipient).rate)
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

pub fn try_set_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    key: String,
) -> StdResult<HandleResponse> {
    let vk = ViewingKey(key);

    let message_sender = deps.api.canonical_address(&env.message.sender)?;
    write_viewing_key(&mut deps.storage, &message_sender, &vk);

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetViewingKey { status: Success })?),
    })
}

pub fn try_create_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    entropy: String,
) -> StdResult<HandleResponse> {
    let constants = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let prng_seed = constants.prng_seed;

    let key = ViewingKey::new(&env, &prng_seed, (&entropy).as_ref());

    let message_sender = deps.api.canonical_address(&env.message.sender)?;
    write_viewing_key(&mut deps.storage, &message_sender, &key);

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::CreateViewingKey { key })?),
    })
}

fn revoke_permit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    permit_name: String,
) -> StdResult<HandleResponse> {
    RevokedPermits::revoke_permit(
        &mut deps.storage,
        PREFIX_REVOKED_PERMITS,
        &env.message.sender,
        &permit_name,
    );

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::RevokePermit { status: Success })?),
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