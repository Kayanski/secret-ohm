/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use cosmwasm_std::{
    log, to_binary, from_binary, Api, Binary, Env, Extern,
    HandleResponse, HumanAddr, InitResponse, Querier, QueryResult, ReadonlyStorage, StdError,
    StdResult, Storage, Uint128,
};

use crate::msg::{
    space_pad, ContractStatusLevel, HandleAnswer, HandleMsg, ReceiveMsg, InitMsg, QueryAnswer, QueryMsg,
    ResponseStatus::Success, 
    SOhmHandleMsg,  WarmupContractHandleMsg, DistributorHandleMsg, 
    SOhmQueryMsg, CirculatingSupplyResponse, GonsForBalanceResponse, BalanceForGonsResponse,
    IndexResponse
};
use crate::rand::sha_256;
use secret_toolkit::snip20;
use crate::state::{
    read_viewing_key, set_receiver_hash,
    write_viewing_key, Config, Constants, ReadonlyConfig, Epoch, Claim,
    ConfigContracts, ContractType, Contract
};
use crate::viewing_key::{ViewingKey, VIEWING_KEY_SIZE};
use secret_toolkit::utils::{HandleCallback, Query};

use primitive_types::U256;
/// We make sure that responses from `handle` are padded to a multiple of this size.
pub const RESPONSE_BLOCK_SIZE: usize = 256;
pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";

pub const COMMON_VIEWING_KEY : &str = "ALL_ORGANISATION_INFO_SHOULD_BE_PUBLIC";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    // Check name, symbol, decimals
   
    let admin = msg.admin.unwrap_or(env.message.sender);

    let prng_seed_hashed = sha_256(&msg.prng_seed.0);

    let mut config = Config::from_storage(&mut deps.storage);

    config.set_constants(&Constants {
        admin: admin.clone(),
        prng_seed: prng_seed_hashed.to_vec(),
        ohm : msg.ohm.clone(),
        sohm : msg.sohm.clone(),
        epoch: Epoch{
            length: msg.epoch_length.clone(),
            number: msg.first_epoch_number.clone(),
            end_block: msg.first_epoch_block.clone(),
            distribute: Uint128(0)
        },
        total_bonus:Uint128(0),
        warmup_period:0,
        contract_address:env.contract.address,
    })?;

    let messages = vec![  
        //Set register receive msgs  
        snip20::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            RESPONSE_BLOCK_SIZE,
            msg.ohm.code_hash.clone(),
            msg.ohm.address.clone()
        )?, 
        snip20::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            RESPONSE_BLOCK_SIZE,
            msg.sohm.code_hash.clone(),
            msg.sohm.address.clone()
        )?,
        //Add viewing keys for the ohm and sohm contracts
        snip20::set_viewing_key_msg(
            COMMON_VIEWING_KEY.to_string(),
            None,
            RESPONSE_BLOCK_SIZE,
            msg.ohm.code_hash,
            msg.ohm.address
        )?,
        snip20::set_viewing_key_msg(
            COMMON_VIEWING_KEY.to_string(),
            None,
            RESPONSE_BLOCK_SIZE,
            msg.sohm.code_hash,
            msg.sohm.address
        )?,
    ];



    config.set_contracts(&ConfigContracts::default())?;
    config.set_contract_status(ContractStatusLevel::NormalRun);
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

//After a transfer of OHM to this address (receive Msg)
pub fn stake<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: HumanAddr,
    amount: u128,
) -> StdResult<HandleResponse> {
    let rebase_response = rebase(deps,env)?;

    let mut config = Config::from_storage(&mut deps.storage);
    let canon_recipient = deps.api.canonical_address(&recipient)?;

    let claim_info: Claim = config.warmup_info(&canon_recipient);
    if claim_info.lock{
        return Err(StdError::generic_err(
            "Deposits for account are locked",
        ));
    }
    let consts = config.constants()?.clone();
   
    let gons_for_balance_query_msg = SOhmQueryMsg::GonsForBalance{amount:Uint128(amount)};
    let gons_for_balance_response: GonsForBalanceResponse = gons_for_balance_query_msg.query(
        &deps.querier,
        consts.sohm.code_hash.clone(),
        consts.sohm.address.clone(),
    )?;
    let gons = U256::from_dec_str(&gons_for_balance_response.gons_for_balance.gons).unwrap();


    config.set_warmup_info(&canon_recipient,Claim{
        deposit: Uint128(claim_info.deposit.u128().checked_add(amount).ok_or_else(|| {
            StdError::generic_err("Sorry, can't deposit, the contract already contains too much sOHM")
        })?),
        gons: U256::to_string(&U256::from_dec_str(&claim_info.gons).unwrap().checked_add(gons).ok_or_else(|| {
            StdError::generic_err("Sorry, can't deposit, the contract already contains too much sOHM")
        })?),
        expiry: consts.epoch.number.checked_add(consts.warmup_period).ok_or_else(|| {
            StdError::generic_err("Sorry, can't deposit, the maximum epoch has been reached")
        })?,
        lock: false
    })?;
    let mut messages = rebase_response.messages;
    messages.push(snip20::transfer_msg(
        config.contracts()?.warmup.address,
        Uint128(amount),
        None,
        RESPONSE_BLOCK_SIZE,
        consts.sohm.code_hash.clone(),
        consts.sohm.address.clone(),
    )?);
    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Stake { status: Success })?),
    })
} 

pub fn rebase<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let config = ReadonlyConfig::from_storage(&deps.storage);
    let mut consts = config.constants()?.clone();
    let distributor = config.contracts()?.distributor;
    let mut messages = vec![];
    if consts.epoch.end_block <= env.block.height{
        //We rebase the sOHM contract
        //We need to ask the contract for a valuation
        let rebase_msg = SOhmHandleMsg::Rebase{
            profit: consts.epoch.distribute,
            epoch: consts.epoch.number,
        };
        messages.push(
            rebase_msg.to_cosmos_msg( 
                consts.sohm.code_hash.clone(),
                consts.sohm.address.clone(),
                None
            )?
        );

        consts.epoch.end_block = consts.epoch.end_block + consts.epoch.length;
        consts.epoch.number += 1;

        if distributor.address != HumanAddr::default(){
            let distribute_msg = DistributorHandleMsg::Distribute{};
            messages.push(
                distribute_msg.to_cosmos_msg( 
                    distributor.code_hash,
                    distributor.address,
                    None
                )?
            );
        }

        let balance = contract_balance(deps)?;
        let staked = {
            //We need to ask the contract for the circulating_supply
            let circulating_supply_query_msg = SOhmQueryMsg::CirculatingSupply{};
            let circulating_supply_response: CirculatingSupplyResponse = circulating_supply_query_msg.query(
                &deps.querier,
                consts.sohm.code_hash.clone(),
                consts.sohm.address.clone(),
            )?;
            circulating_supply_response.circulating_supply.circulating_supply.u128()
        };

        if balance <= staked{
            consts.epoch.distribute = Uint128(0);
        }else{
            consts.epoch.distribute = Uint128(balance - staked);
        }
        
        let mut config = Config::from_storage(&mut deps.storage); 
        config.set_constants(&consts)?;

    }
    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Rebase { status: Success })?),
    })
} 
pub fn contract_balance<S: Storage, A: Api, Q: Querier>(
    deps: & Extern<S, A, Q>,
)-> StdResult<u128>{

    let constants = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    snip20::balance_query(
        &deps.querier,
        constants.contract_address.clone(),
        COMMON_VIEWING_KEY.to_string(),
        RESPONSE_BLOCK_SIZE,
        constants.ohm.code_hash,
        constants.ohm.address
    )?.amount.u128().checked_add(constants.total_bonus.u128()).ok_or_else(|| {
        StdError::generic_err("The contract is too rich for you, sorry")
    })
}

fn query_contract_balance<S: Storage, A: Api, Q: Querier>(
    deps: & Extern<S, A, Q>
) -> QueryResult {
    let contract_balance = contract_balance(deps)?;
    to_binary(&QueryAnswer::ContractBalance {
        amount: Uint128(contract_balance)
    })
}

pub fn claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    recipient: HumanAddr
)-> StdResult<HandleResponse> {

    let mut config = Config::from_storage(&mut deps.storage);
    let canon_recipient = deps.api.canonical_address(&recipient)?;
    let claim_info = config.warmup_info(&canon_recipient).clone();
    let mut messages = vec![];
    if config.constants()?.epoch.number >= claim_info.expiry && claim_info.expiry != 0{
        config.set_warmup_info(&canon_recipient,Claim::default())?;
        //We get the balance for gons equivalent
        let balance_for_gons_query_msg = SOhmQueryMsg::BalanceForGons{
            gons: claim_info.gons
        };
        let balance_for_gons_response: BalanceForGonsResponse = balance_for_gons_query_msg.query(
            &deps.querier,
            config.constants()?.sohm.code_hash.clone(),
            config.constants()?.sohm.address.clone(),
        )?;
        //We send the retrieve message from the warmup contract
        let retrieve_msg = WarmupContractHandleMsg::Retrieve{
            staker: recipient,
            amount: balance_for_gons_response.balance_for_gons.amount
        };
        messages.push(
            retrieve_msg.to_cosmos_msg( 
                config.contracts()?.warmup.code_hash,
                config.contracts()?.warmup.address,
                None
            )?
        );
    }
    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Claim { status: Success })?),
    })
}

pub fn forfeit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env
)-> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    let canon_sender = deps.api.canonical_address(&env.message.sender)?;
    let claim_info = config.warmup_info(&canon_sender).clone();
    //delete the claim in memory for the sender
    config.set_warmup_info(&canon_sender,Claim::default())?;
    let mut messages = vec![];

    //We get the balance for gons equivalent
    let balance_for_gons_query_msg = SOhmQueryMsg::BalanceForGons{
        gons: claim_info.gons
    };
    let balance_for_gons_response: BalanceForGonsResponse = balance_for_gons_query_msg.query(
        &deps.querier,
        config.constants()?.sohm.code_hash.clone(),
        config.constants()?.sohm.address.clone(),
    ).unwrap();
//We send the retrieve message from the warmup contract
    let retrieve_msg = WarmupContractHandleMsg::Retrieve{
        staker: env.contract.address.clone(),
        amount: balance_for_gons_response.balance_for_gons.amount
    };
    messages.push(
        retrieve_msg.to_cosmos_msg( 
            config.contracts()?.warmup.code_hash,
            config.contracts()?.warmup.address,
            None
        )?
    );

    //Send funds back to the address
    messages.push(snip20::transfer_msg(
        env.message.sender.clone(),
        claim_info.deposit,
        None,
        RESPONSE_BLOCK_SIZE,
        config.constants()?.ohm.code_hash.clone(),
        config.constants()?.ohm.address.clone(),
    )?);
    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Forfeit { status: Success })?),
    })
}
pub fn toggle_deposit_lock<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env
)-> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    let canon_sender = deps.api.canonical_address(&env.message.sender)?;
    let mut claim_info = config.warmup_info(&canon_sender).clone();
    claim_info.lock = !claim_info.lock;
    config.set_warmup_info(&canon_sender, claim_info)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ToggleDepositLock { status: Success })?),
    })
}
//TODO this is a receive message, after an sOHM deposit from the sender to this address 
pub fn unstake<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: HumanAddr,
    amount: u128,
    trigger: bool
)-> StdResult<HandleResponse> {
    let mut messages = vec![];
    if trigger{
        let rebase_response = rebase(deps,env)?;
        messages.extend(rebase_response.messages);
    }
    let config = ReadonlyConfig::from_storage(&deps.storage);

    messages.push(
        snip20::transfer_msg(
            sender.clone(),
            Uint128(amount),
            None,
            RESPONSE_BLOCK_SIZE,
            config.constants()?.ohm.code_hash.clone(),
            config.constants()?.ohm.address.clone(),
        )?
    );
    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Unstake { status: Success })?),
    })
}
//TODO public view (needs a query function)
pub fn index<S: Storage, A: Api, Q: Querier>(
    deps: & Extern<S, A, Q>,
)-> StdResult<String> {
    let config = ReadonlyConfig::from_storage(&deps.storage);
    //We get the balance for gons equivalent
    let index_query_msg = SOhmQueryMsg::Index{};
    let index_response: IndexResponse = index_query_msg.query(
        &deps.querier,
        config.constants()?.sohm.code_hash.clone(),
        config.constants()?.sohm.address.clone(),
    )?;
    Ok(index_response.index.index)
}

fn query_index<S: Storage, A: Api, Q: Querier>(
    deps: & Extern<S, A, Q>
) -> QueryResult {
    let index = index(deps)?;
    to_binary(&QueryAnswer::Index {
        index: index
    })
}
//TODO create a hanlde for this one
pub fn give_lock_bonus<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    let mut constants = config.constants()?;
    check_equal(&env.message.sender, &config.contracts()?.locker.address)?;
    constants.total_bonus = Uint128(constants.total_bonus.u128().checked_add(amount.u128()).ok_or_else(|| {
        StdError::generic_err("Sorry, can't give bonus to the locker contract, too much bonus already")
    })?);
    config.set_constants(&constants)?;
    let messages = vec![
        snip20::transfer_msg(
            config.contracts()?.locker.address,
            amount,
            None,
            RESPONSE_BLOCK_SIZE,
            config.constants()?.sohm.code_hash.clone(),
            config.constants()?.sohm.address.clone(),
        )?
    ];
    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::GiveLockBonus{ status: Success })?),
    })
}
//TODO, this is called after a received transfer from the locker
pub fn return_lock_bonus<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    sender: HumanAddr,
    amount: u128
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    let mut constants = config.constants()?;
    check_equal(&sender, &config.contracts()?.locker.address)?;

    constants.total_bonus = Uint128(constants.total_bonus.u128().checked_sub(amount).ok_or_else(|| {
        StdError::generic_err("Sorry, can't return bonus to the locker contract, nothing to give back :/")
    })?);
    config.set_constants(&constants)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ReturnLockBonus{ status: Success })?),
    })
}
//TODO, a hadle function for this one
pub fn set_contract<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    contract_type: ContractType,
    contract: Contract
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    let mut contracts = config.contracts()?;
    check_if_admin(&config,&env.message.sender)?;
    match contract_type{
        ContractType::Distributor => contracts.distributor = contract,
        ContractType::WarmupContract => if contracts.warmup == Contract::default() {contracts.warmup = contract},
        ContractType::Locker => if contracts.locker == Contract::default() {contracts.locker = contract},
    };
    config.set_contracts(&contracts)?;


    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetContract{ status: Success })?),
    })
}
pub fn set_warmup_period<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    warmup_period: u64
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    let mut constants = config.constants()?;
    constants.warmup_period = warmup_period;
    config.set_constants(&constants)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetWarmupPeriod{ status: Success })?),
    })
}
pub fn receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    from: HumanAddr,
    amount: u128,
    msg: Binary,
) -> StdResult<HandleResponse> {

    let msg: ReceiveMsg = from_binary(&msg)?;
    let token = env.message.sender.clone();
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    match msg {
        ReceiveMsg::Stake {recipient, ..} => {
            if token == consts.ohm.address{
                stake(deps, env, recipient.unwrap_or(from), amount)
            }else{
                Err(StdError::generic_err(
                    "You can't stake anything else than the treasury token",
                ))
            }
        },
        ReceiveMsg::Unstake {trigger,..} => {
            if token == consts.sohm.address{
                unstake(deps, env, from, amount, trigger)
            }else{
                Err(StdError::generic_err(
                    "You can't unstake with anything else than the staked treasury token",
                ))
            }
        },
        ReceiveMsg::ReturnLockBonus {..} => {
            if token == consts.sohm.address{
                return_lock_bonus(deps, from, amount)
            }else{
                Err(StdError::generic_err(
                    "Bonuses should be the staked treasury token",
                ))
            }
        },
    }
}


pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    let contract_status = ReadonlyConfig::from_storage(&deps.storage).contract_status();

    match contract_status {
        ContractStatusLevel::StopAll | ContractStatusLevel::StopAllButRedeems => {
            let response = match msg {
                HandleMsg::SetContractStatus { level, .. } => set_contract_status(deps, env, level),
                _ => Err(StdError::generic_err(
                    "This contract is stopped and this action is not allowed",
                )),
            };
            return pad_response(response);
        }
        ContractStatusLevel::NormalRun => {} // If it's a normal run just continue
    }

    let response = match msg {
        // Native
       
        //Register Receive messages
        HandleMsg::Receive {
                    from, amount, msg,..
                } => receive(deps, env, from, amount.u128(), msg),

        HandleMsg::RegisterReceive { code_hash, .. } => try_register_receive(deps, env, code_hash),
        HandleMsg::CreateViewingKey { entropy, .. } => try_create_key(deps, env, entropy),
        HandleMsg::SetViewingKey { key, .. } => try_set_key(deps, env, key),

        // Other
        HandleMsg::ChangeAdmin { address, .. } => change_admin(deps, env, address),
        HandleMsg::SetContractStatus { level, .. } => set_contract_status(deps, env, level),

        //Staking
        HandleMsg::Rebase {..} => rebase(deps, env,),
        HandleMsg::Claim {recipient, .. } => claim(deps, recipient),
        HandleMsg::Forfeit{..} => forfeit(deps, env,),
        HandleMsg::ToggleDepositLock {..} => toggle_deposit_lock(deps, env),
        HandleMsg::GiveLockBonus { amount, ..  } => give_lock_bonus(deps, env, amount),
        HandleMsg::SetContract { contract_type,contract, .. } => set_contract(deps, env, contract_type, contract),
        HandleMsg::SetWarmupPeriod { warmup_period , .. } => set_warmup_period(deps, env, warmup_period),


        _ => Err(StdError::generic_err(
                    "TODO not handled right now",
                )),
    };

    pad_response(response)
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {

        QueryMsg::ContractBalance {} => query_contract_balance(deps),
        QueryMsg::Index {} => query_index(deps),
        QueryMsg::ContractStatus {} => query_contract_status(&deps.storage),
        /*
        QueryMsg::WithPermit { permit, query } => permit_queries(deps, permit, query),
        */
        _ => viewing_keys_queries(deps, msg),
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
            panic!("This query type does not require authentication")
            /*
            return match msg {
                // Base
                _ => ,
            };
            */
        }
    }

    to_binary(&QueryAnswer::ViewingKeyError {
        msg: "Wrong viewing key for this address or viewing key not set".to_string(),
    })
}

fn query_contract_status<S: ReadonlyStorage>(storage: &S) -> QueryResult {
    let config = ReadonlyConfig::from_storage(storage);

    to_binary(&QueryAnswer::ContractStatus {
        status: config.contract_status(),
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

fn set_contract_status<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    status_level: ContractStatusLevel,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    config.set_contract_status(status_level);

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetContractStatus {
            status: Success,
        })?),
    })
}


fn try_register_receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    code_hash: String,
) -> StdResult<HandleResponse> {
    set_receiver_hash(&mut deps.storage, &env.message.sender, code_hash);
    let res = HandleResponse {
        messages: vec![],
        log: vec![log("register_status", "success")],
        data: Some(to_binary(&HandleAnswer::RegisterReceive {
            status: Success,
        })?),
    };
    Ok(res)
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

fn check_equal(account1: &HumanAddr, account2: &HumanAddr) -> StdResult<()> {
    if account1 != account2{
        return Err(StdError::generic_err(
            "This address can't call this function",
        ));
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::ResponseStatus;
    use cosmwasm_std::testing::*;
    use cosmwasm_std::{from_binary};
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
            admin: None,
            ohm : Contract{address:HumanAddr("ohm".to_string()),code_hash:"Complicated_hash".to_string()},
            sohm : Contract{address:HumanAddr("sohm".to_string()),code_hash:"Complicated_hash".to_string()},
            epoch_length: 2200,
            first_epoch_number: 338,
            first_epoch_block: 8961000,
            prng_seed: Binary::from("lolz fun yay".as_bytes()),
            
        };

        (init(&mut deps, env, init_msg), deps)
    }

    fn extract_error_msg<T: Any>(error: StdResult<T>) -> String {
        match error {
            Err(err) => match err {
                StdError::GenericErr { msg, .. } => msg,
                _ => panic!("Unexpected result from init"),
            },
            Ok(_) => "Very nice".to_string()
        }
    }

    fn ensure_success(handle_result: HandleResponse) -> bool {
        let handle_result: HandleAnswer = from_binary(&handle_result.data.unwrap()).unwrap();

        match handle_result {
        | HandleAnswer::RegisterReceive { status }
        | HandleAnswer::ChangeAdmin { status }
        | HandleAnswer::SetContractStatus { status }
        | HandleAnswer::RevokePermit { status }
        | HandleAnswer::Rebase { status }
        | HandleAnswer::Claim { status }
        | HandleAnswer::Forfeit { status }
        | HandleAnswer::ToggleDepositLock { status }
        | HandleAnswer::GiveLockBonus { status }
        | HandleAnswer::SetContract { status }
        | HandleAnswer::SetWarmupPeriod { status } => {
                matches!(status, ResponseStatus::Success { .. })
            },
            _ => panic!("Answer not handled in test right now")
        }
    }

    fn ensure_failure(handle_result: HandleResponse) -> bool {
        let handle_result: HandleAnswer = from_binary(&handle_result.data.unwrap()).unwrap();

        match handle_result {
            | HandleAnswer::RegisterReceive { status }
            | HandleAnswer::ChangeAdmin { status }
            | HandleAnswer::SetContractStatus { status }
            | HandleAnswer::RevokePermit { status }
            | HandleAnswer::Rebase { status }
            | HandleAnswer::Claim { status }
            | HandleAnswer::Forfeit { status }
            | HandleAnswer::ToggleDepositLock { status }
            | HandleAnswer::GiveLockBonus { status }
            | HandleAnswer::SetContract { status }
            | HandleAnswer::SetWarmupPeriod { status } => {
                matches!(status, ResponseStatus::Failure { .. })
            },
            _ => panic!("Answer not handled in test right now")
        }
    }

    // Init tests

    #[test]
    fn test_init_sanity() {
        let (_init_result, deps) = init_helper();
        //assert_eq!(init_result.unwrap(), InitResponse::default());

        let config = ReadonlyConfig::from_storage(&deps.storage);
        let constants = config.constants().unwrap();
        
       
        assert_eq!(constants.epoch.end_block,8961000);
        assert_eq!(constants.admin, HumanAddr("admin".to_string()));
        assert_eq!(
            constants.prng_seed,
            sha_256("lolz fun yay".to_owned().as_bytes())
        );
    }

    #[test]
    fn test_stake(){
        let (init_result, mut deps) = init_helper();
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );
        let stake_msg = ReceiveMsg::Stake{
            recipient: None,
        };
        let handle_receive_msg = HandleMsg::Receive {
            sender : HumanAddr("ohm".to_string()),
            from : HumanAddr("admin".to_string()),
            amount: Uint128(40000000000000),
            msg: to_binary(&stake_msg).unwrap()
        };
        /*
        let handle_result = handle(&mut deps, mock_env("ohm", &[]), handle_receive_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));
        */
        
    }
}


