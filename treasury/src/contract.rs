/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use cosmwasm_std::{
    to_binary, Api, Binary, CosmosMsg, Env, Extern,
    HandleResponse, HumanAddr, InitResponse, Querier, QueryResult, ReadonlyStorage, StdError,
    StdResult, Storage, Uint128, from_binary
};

use crate::msg::{
    space_pad, ContractStatusLevel, HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg,
    ResponseStatus::Success, ReceiveAnswer, ReceiveMsg
};
use crate::rand::sha_256;
use crate::state::{
     Config, Constants, Debtors, ReadonlyConfig, ReadonlyDebtors,
    ManagingRole, Contract, Deposited, ReadonlyDeposited, RESPONSE_BLOCK_SIZE
};

use secret_toolkit::snip20;

pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";
pub const COMMON_VIEWING_KEY : &str = "ALL_ORGANISATION_INFO_SHOULD_BE_PUBLIC";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    
    // Check name, symbol, decimals
    if !is_valid_name(&msg.name) {
        return Err(StdError::generic_err(
            "Name is not in the expected format (3-30 UTF-8 bytes)",
        ));
    }

    let admin = msg.admin.unwrap_or(env.message.sender);

    let prng_seed_hashed = sha_256(&msg.prng_seed.0);

    let mut config = Config::from_storage(&mut deps.storage);

    config.set_constants(&Constants {
        ohm: msg.ohm.clone(),
        sohm: msg.sohm.clone(),
        name: msg.name,
        admin: admin.clone(),
        prng_seed: prng_seed_hashed.to_vec(),
        contract_address: env.contract.address,
        blocks_needed_for_queue : msg.blocks_needed_for_queue
    })?;

    config.set_contract_status(ContractStatusLevel::NormalRun);
    
    config.set_reserve_tokens(msg.reserve_tokens.unwrap_or_default())?;
    config.set_liquidity_tokens(msg.liquidity_tokens.unwrap_or_default())?;
    config.set_total_reserves(0);

    let mut messages : Vec<CosmosMsg> = vec![];

    //We need to register a receive message from the treasury tokens
    //When depositing a token, we need to make sure the token is received before updating the treasury
    //Reserve
    for token in config.reserve_tokens(){
        messages.push(
        snip20::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            RESPONSE_BLOCK_SIZE,
            token.code_hash.clone(),
            token.address.clone(),
        )?);
        messages.push(
        snip20::set_viewing_key_msg(
            COMMON_VIEWING_KEY.to_string(),
            None,
            RESPONSE_BLOCK_SIZE,
            token.code_hash.clone(),
            token.address.clone(),
        )?);
    }
    //Liquidity
    for token in config.liquidity_tokens(){
        messages.push(
        snip20::register_receive_msg(
            env.contract_code_hash.clone(),
            None,
            RESPONSE_BLOCK_SIZE,
            token.code_hash.clone(),
            token.address.clone(),
        )?);
        messages.push(
        snip20::set_viewing_key_msg(
            COMMON_VIEWING_KEY.to_string(),
            None,
            RESPONSE_BLOCK_SIZE,
            token.code_hash.clone(),
            token.address.clone(),
        )?);
    }

    //Add viewing keys for the ohm and sohm contracts
    messages.push(
        snip20::set_viewing_key_msg(
            COMMON_VIEWING_KEY.to_string(),
            None,
            RESPONSE_BLOCK_SIZE,
            msg.ohm.code_hash,
            msg.ohm.address
        )?
    );
    messages.push(
        snip20::set_viewing_key_msg(
            COMMON_VIEWING_KEY.to_string(),
            None,
            RESPONSE_BLOCK_SIZE,
            msg.sohm.code_hash,
            msg.sohm.address
        )?
    );

    Ok(InitResponse {
        messages,
        log: vec![],
    })
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

        //Register Receive messages
        HandleMsg::Receive {
                    from, amount, msg,..
                } => receive(deps, env, from, amount.u128(), msg),

        //Normal messages
        HandleMsg::IncurDebt { token, amount, .. } => incur_debt(deps, env, token, amount.u128()),
        HandleMsg::Manage { token, amount, .. } => manage(deps, env, token, amount.u128()),
        HandleMsg::MintRewards { recipient, amount, .. } => mint_rewards(deps, env, recipient, amount.u128()),
        HandleMsg::AuditReserves { .. } => audit_reserves(deps, env),
        HandleMsg::Queue { address, role } => queue(deps, env, address, role),
        HandleMsg::ToggleQueue { address, role } => toggle_queue(deps, env, address, role),
        HandleMsg::ToggleTokenQueue { token, role, calculator } => toggle_token_queue(deps, env, token, role, calculator),

        // Other
        HandleMsg::ChangeAdmin { address, .. } => change_admin(deps, env, address),
        HandleMsg::SetContractStatus { level, .. } => set_contract_status(deps, env, level),
    };

    pad_response(response)
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::ContractInfo {} => query_contract_info(deps),
        QueryMsg::Contracts {role} => query_tokens(&deps.storage, role),
        QueryMsg::ManagingAddresses {role} => query_managing_addresses(deps, role),
        QueryMsg::ContractStatus {} => query_contract_status(&deps.storage),
        QueryMsg::ValueOf{ token, amount } => query_value_of(deps, token, amount),
        QueryMsg::TotalBondDeposited{ token } => query_total_bond_deposited(deps, token),
    }
}

pub fn deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    from : HumanAddr,
    token : HumanAddr,
    amount: u128,
    profit: u128

) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    if !(config.is_reserve_token(&token) || config.is_liquidity_token(&token)){
        return Err(StdError::generic_err(
                    "Token not accepted",
                ));
    }
    let from_canonical = deps.api.canonical_address(&from)?;
    if (config.is_reserve_token(&token) && !config.has_managing_position(&from_canonical,ManagingRole::ReserveDepositor)) |
        (config.is_liquidity_token(&token) && !config.has_managing_position(&from_canonical,ManagingRole::LiquidityDepositor)){
        return Err(StdError::generic_err(
                    "Depositor not approved",
                ));
    }

    let value = config.value_of(&deps.querier,&token,amount)?;

    let send = if let Some(send_) = value.checked_sub(profit) {
        send_
    } else {
        return Err(StdError::generic_err(format!(
            "insufficient funds to mint amount={}, profit={}",
            value, profit
        )));
    };
    // mint OHM needed and store amount of rewards for distribution
    let messages = vec![snip20::mint_msg(
        from,
        Uint128(send),
        None,
        RESPONSE_BLOCK_SIZE,
        config.constants()?.ohm.code_hash,
        config.constants()?.ohm.address,
        )?];

    //Update the total reserves
    if let Some(new_total_reserves) = config.total_reserves().checked_add(value) {
        config.set_total_reserves(new_total_reserves);
    } else {
        return Err(StdError::generic_err(
            "This mint attempt would increase the total reserves above the supported maximum",
        ));
    }
    let mut deposited = Deposited::from_storage(&mut deps.storage);   
    let token_address = deps.api.canonical_address(&token)?;

    //Save a bond deposit (to have statistics)
    deposited.add_new_bond(&token_address, amount)?;

    Ok(HandleResponse {
        messages : messages,
        log: vec![],
        data: Some(to_binary(&ReceiveAnswer::Deposit { status: Success })?),
    })
}

pub fn withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sent_token: HumanAddr,
    sent_amount: u128,
    token_address : HumanAddr,
    withdraw_amount: u128
) -> StdResult<HandleResponse> {
    let sender = env.message.sender.clone();
    let mut config = Config::from_storage(&mut deps.storage);
    if !config.is_reserve_token(&token_address){
        return Err(StdError::generic_err(
                    "Token not accepted",
                ));
    }

    if sent_token != config.constants()?.ohm.address{
        return Err(StdError::generic_err(
                    "Can only withdraw by burning OHM",
                ));
    }
    let canonical_sender = deps.api.canonical_address(&sender)?;
    if !config.has_managing_position(&canonical_sender,ManagingRole::ReserveDepositor){
        return Err(StdError::generic_err(
                    "Not authorized",
                ));
    }
    let value = config.value_of(&deps.querier,&token_address,withdraw_amount)?;
    if value != sent_amount{
        return Err(StdError::generic_err(
            "Sent token amount and specified reserve amount don't match",
        ));
    }
    let mut messages = vec![];

    //We burn some OHM
    messages.push(snip20::burn_msg(
        Uint128(sent_amount),
        None,
        RESPONSE_BLOCK_SIZE,
        config.constants()?.ohm.code_hash,
        config.constants()?.ohm.address
        )?);

    if let Some(new_total_reserves) = config.total_reserves().checked_sub(value) {
        config.set_total_reserves(new_total_reserves);
    } else {
        return Err(StdError::generic_err(
            "This withdraw attemp is not possible, not enough reserves",
        ));
    }
    let token = config.get_reserve_token_info(&token_address)?;
    //We transfer the withdrawn amount
    messages.push(snip20::send_msg(
        sender,
        Uint128(withdraw_amount),
        None,
        None,
        RESPONSE_BLOCK_SIZE,
        token.code_hash,
        token.address
        )?);

    Ok(HandleResponse {
        messages : messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Withdraw { status: Success })?),
    })

}

pub fn receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    from: HumanAddr,
    sent_amount: u128,
    msg: Binary,
) -> StdResult<HandleResponse> {

    let msg: ReceiveMsg = from_binary(&msg)?;
    let sent_token = env.message.sender.clone();
    match msg {
        ReceiveMsg::Deposit {profit,..} => deposit(deps, env, from, sent_token, sent_amount, profit.u128()),

        ReceiveMsg::Withdraw { token, amount,.. } => withdraw(deps, env, sent_token, sent_amount, token, amount.u128()),

        ReceiveMsg::RepayDebt {..} => repay_debt(deps, env, from, sent_token, sent_amount),
    }
}
pub fn incur_debt<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    token : HumanAddr,
    amount: u128,
) -> StdResult<HandleResponse> {
    let sender = env.message.sender.clone();

    let config = ReadonlyConfig::from_storage(& deps.storage);
    if !config.is_reserve_token(&token){
        return Err(StdError::generic_err(
                    "Token not accepted",
                ));
    }
    let canonical_sender = deps.api.canonical_address(&sender)?;
    if !config.has_managing_position(&canonical_sender,ManagingRole::Debtor){
        return Err(StdError::generic_err(
                    "Not authorized",
                ));
    }
    let value = config.value_of(&deps.querier,&token,amount)?;
    let from_balance = snip20::balance_query(
        &deps.querier,
        sender.clone(),
        COMMON_VIEWING_KEY.to_string(),
        RESPONSE_BLOCK_SIZE,
        config.constants()?.sohm.code_hash,
        config.constants()?.sohm.address
    )?;
    let maximum_debt = from_balance.amount.u128();// Can only borrow against sOHM held

    let debtors = ReadonlyDebtors::from_storage(& deps.storage);

    let current_debt = debtors.debt(&canonical_sender);
    //Check if the debtor can occur this amount of debt
    if let Some(available_debt) = maximum_debt.checked_sub(current_debt) {
        if value > available_debt{
             return Err(StdError::generic_err(
                "Not possible, too much debt already",
            ));
        }
    } else {
        return Err(StdError::generic_err(
            "Not possible, too much debt already",
        ));
    }
    //Update the debtors debt
    let mut debtors = Debtors::from_storage(&mut deps.storage);
    if let Some(new_debt) = current_debt.checked_add(value){
        debtors.set_account_debt(&canonical_sender,new_debt);
    } else {
        return Err(StdError::generic_err(
            "Not possible, the debt of this account is above the u128 capacity",
        ));
    };
    // Update the total debt 
    let mut config = Config::from_storage(&mut deps.storage);
    if let Some(new_total_debt) = config.total_debt().checked_add(value){
        config.set_total_debt(new_total_debt);
    } else {
        return Err(StdError::generic_err(
            "Not possible, the total debt is above the u128 capacity",
        ));
    }
     // Update the total reserves
    if let Some(new_total_reserves) = config.total_reserves().checked_sub(value){
        config.set_total_reserves(new_total_reserves);
    } else {
        return Err(StdError::generic_err(
            "Not enough reserves to incur debt",
        ));
    }
    let token_info = config.get_reserve_token_info(&token)?;
    

    let messages = vec![
        snip20::send_msg(
        sender,
        Uint128(amount),
        None,
        None,
        RESPONSE_BLOCK_SIZE,
        token_info.code_hash.clone(),
        token_info.address.clone()
    )?];

    Ok(HandleResponse {
        messages : messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::IncurDebt { status: Success })?),
    })
       
}
pub fn repay_debt<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    from : HumanAddr,
    token : HumanAddr,
    amount: u128,

) -> StdResult<HandleResponse> {

    let mut config = Config::from_storage(&mut deps.storage);

    let ohm = config.constants()?.ohm.clone();
    let token_is_ohm = token == ohm.address;
    if ! (config.is_reserve_token(&token) || token_is_ohm){
        return Err(StdError::generic_err(
                    "Token not accepted",
                ));
    }

    let canonical_from = deps.api.canonical_address(&from)?;
    if !config.has_managing_position(&canonical_from,ManagingRole::Debtor){
        return Err(StdError::generic_err(
                    "Not authorized",
                ));
    }

    let value = config.value_of(&deps.querier,&token,amount)?;

    // Update the total debt 
    if let Some(new_total_debt) = config.total_debt().checked_sub(value){
        config.set_total_debt(new_total_debt);
    } else {
        return Err(StdError::generic_err(
            "Not possible, total debt would be negative",
        ));
    }
     // Update the total reserves (if the token is a reserve token)
    let mut messages = vec![];
    if !token_is_ohm 
    {
        if let Some(new_total_reserves) = config.total_reserves().checked_add(value){
            config.set_total_reserves(new_total_reserves);
        } else {
            return Err(StdError::generic_err(
                "THe total reserves are through the roof",
            ));
        }
    }else{
        //We burn the OHM token received
        messages.push(snip20::burn_msg(
            Uint128(value),
            None,
            RESPONSE_BLOCK_SIZE,
            ohm.code_hash,
            ohm.address,
        )?);

    }
    
    //Update the debtors debt
    let mut debtors = Debtors::from_storage(&mut deps.storage);
    if let Some(new_debt) = debtors.debt(&canonical_from).checked_sub(value){
        debtors.set_account_debt(&canonical_from,new_debt);
    } else {
        return Err(StdError::generic_err(
            "Not possible, the debt of this account would be negative",
        ));
    };

    Ok(HandleResponse {
        messages : messages,
        log: vec![],
        data: Some(to_binary(&ReceiveAnswer::RepayDebt { status: Success })?),
    })
}

//Let me talk to management
pub fn manage<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    token : HumanAddr,
    amount: u128,
) -> StdResult<HandleResponse> {

    let sender = env.message.sender.clone();
    let mut config = Config::from_storage(&mut deps.storage);
    let canonical_sender = deps.api.canonical_address(&sender)?;
    if !((config.is_reserve_token(&token) & config.has_managing_position(&canonical_sender,ManagingRole::ReserveManager)) | 
        (config.is_liquidity_token(&token) & config.has_managing_position(&canonical_sender,ManagingRole::LiquidityManager)))
    {
        return Err(StdError::generic_err(
                    "Token not accepted",
                ));
    }

    let value = config.value_of(&deps.querier,&token,amount)?;
    if value > config.excess_reserves(&deps.querier)?{
        return Err(StdError::generic_err(
                    "Insufficient reserves" ,
                ));
    }
    if let Some(new_total_reserves) = config.total_reserves().checked_sub(value){
        config.set_total_reserves(new_total_reserves);
    } else {
        return Err(StdError::generic_err(
            "Insufficient reserves",
        ));
    }
    let token_info = config.get_reserve_token_info(&token)?;
    let messages = vec![snip20::send_msg(
         sender,
         Uint128(amount),
         None, //TODO Maybe change that, so that the recipient knows what to do with the funds
         None,
         RESPONSE_BLOCK_SIZE,
         token_info.code_hash.clone(),
         token_info.address.clone(),
        )?];

    Ok(HandleResponse {
        messages : messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Manage { status: Success })?),
    })
}


//Let me talk to management
pub fn mint_rewards<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: HumanAddr,
    mut amount: u128,
) -> StdResult<HandleResponse> {

    let sender = env.message.sender.clone();
    let config = Config::from_storage(&mut deps.storage);
    let canonical_sender = deps.api.canonical_address(&sender)?;
    if !config.has_managing_position(&canonical_sender,ManagingRole::RewardManager)
    {
        return Err(StdError::generic_err(
                    "Address not approved",
                ));
    }
    if amount > config.excess_reserves(&deps.querier)?
    {
        amount = config.excess_reserves(&deps.querier)?;
    }

    Ok(HandleResponse {
        messages : vec![snip20::mint_msg(
            recipient,
            Uint128(amount),
            None,
            RESPONSE_BLOCK_SIZE,
            config.constants()?.ohm.code_hash,
            config.constants()?.ohm.address,
            )?],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::MintRewards { status: Success })?),
    })
}
pub fn audit_reserves<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {

    let sender = env.message.sender.clone();
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &sender)?;

    let contract_address = env.contract.address.clone();
    let mut reserves :u128 = 0;
    for token in config.reserve_tokens(){

        let balance = snip20::balance_query(
            &deps.querier,
            contract_address.clone(),
            COMMON_VIEWING_KEY.to_string(),
            RESPONSE_BLOCK_SIZE,
            token.code_hash.clone(),
            token.address.clone()
        )?.amount.u128();  

        reserves = reserves.checked_add(config.value_of(&deps.querier,&token.address,balance)?)
        .ok_or_else(|| StdError::generic_err("Too much reserves"))?;
    }
    for token in config.liquidity_tokens(){

        let balance = snip20::balance_query(
            &deps.querier,
            contract_address.clone(),
            COMMON_VIEWING_KEY.to_string(),
            RESPONSE_BLOCK_SIZE,
            token.code_hash.clone(),
            token.address.clone()
        )?.amount.u128();   

        reserves = reserves.checked_add(config.value_of(&deps.querier,&token.address,balance)?)
        .ok_or_else(|| StdError::generic_err("Too much reserves"))?;
    }
    config.set_total_reserves(reserves);

    Ok(HandleResponse {
        messages : vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::AuditReserves { status: Success })?),
    })
}

pub fn require_different(a : HumanAddr,b: HumanAddr) -> StdResult<HandleResponse>{
    if a == b
    {
        return Err(StdError::generic_err(format!(
                    "{} is the same as {}", a, b,
                )));
    }
    Ok(HandleResponse::default())

}

pub fn queue<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address : HumanAddr,
    new_role : ManagingRole
) -> StdResult<HandleResponse> {

    let sender = env.message.sender.clone();
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&sender)?;
    require_different(address.clone(),HumanAddr::default())?;
    let canonical_addr = deps.api.canonical_address(&address)?;
    let mut blocks_needed_for_this_queue = config.constants()?.blocks_needed_for_queue;
    match new_role{
        ManagingRole::ReserveManager{} | ManagingRole::LiquidityManager{}

            => blocks_needed_for_this_queue = blocks_needed_for_this_queue.checked_mul(2).ok_or_else(|| {
                    StdError::generic_err("This is the end of the blockchain, no more blocks")
                })?,

        _ => (),
    }
    config.set_managing_queue(
        &canonical_addr,
        new_role,
        env.block.height.checked_add(blocks_needed_for_this_queue).ok_or_else(|| {
            StdError::generic_err("This is the end of the blockchain, no more blocks")
        })?
    )?;

    Ok(HandleResponse {
        messages : vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::QueueAddress { status: Success })?),
    })
}

pub fn toggle_queue<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address : HumanAddr,
    role : ManagingRole
) -> StdResult<HandleResponse> {

    let sender = env.message.sender.clone();
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&sender)?;
    require_different(address.clone(),HumanAddr::default())?; 
    let canonical_addr = deps.api.canonical_address(&address)?;

    let has_managing_position = config.has_managing_position(&canonical_addr,role.clone());
    let queue = config.managing_queue(&canonical_addr,role.clone())?;
    let mut new_has_managing_position = false;
    if !has_managing_position{
        if queue == 0{
            return Err(StdError::generic_err("You need to queue first"));
        }else if queue > env.block.height{
            return Err(StdError::generic_err("Queue is not over yet, wait a bit more"));
        }
        config.set_managing_queue(&canonical_addr,role.clone(),0)?;
        config.set_managing_position(&canonical_addr,role.clone(),true)?;
        new_has_managing_position = true;
    }
    config.set_managing_position(&canonical_addr,role.clone(),new_has_managing_position)?;

    Ok(HandleResponse {
        messages : vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ToggleQueue { status: Success })?),
    })
}

pub fn toggle_token_queue<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    token : Contract,
    role : ManagingRole,
    liquidity_calculator : Option<Contract>
) -> StdResult<HandleResponse> {
    let sender = env.message.sender.clone();

    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&sender)?;
    let canonical_addr = deps.api.canonical_address(&token.address)?;

    let has_managing_position = match role.clone(){
        ManagingRole::ReserveToken{} => config.is_reserve_token(&token.address),
        ManagingRole::LiquidityToken{} => config.is_liquidity_token(&token.address),
        ManagingRole::SOHM{} => true,
        _ => return Err(StdError::generic_err("toggle token with a non token role"))
    };
    let queue = config.managing_queue(&canonical_addr,role.clone())?;
    let mut messages = vec![];
    if !has_managing_position{
        if queue == 0{
            return Err(StdError::generic_err("You need to queue first"));
        }else if queue > env.block.height{
            return Err(StdError::generic_err("Queue is not over yet, wait a bit more"));
        }
        config.set_managing_queue(&canonical_addr,role.clone(),0)?;
        messages.push(
            snip20::register_receive_msg(
                env.contract_code_hash.clone(),
                None,
                RESPONSE_BLOCK_SIZE,
                token.code_hash.clone(),
                token.address.clone(),
            )?
        );
        messages.push(
            snip20::set_viewing_key_msg(
                COMMON_VIEWING_KEY.to_string(),
                None,
                RESPONSE_BLOCK_SIZE,
                token.code_hash.clone(),
                token.address.clone(),
            )?
        );
        match role.clone(){
            ManagingRole::ReserveToken{} => config.add_reserve_tokens(vec![token]),
            ManagingRole::LiquidityToken{} => {
                config.add_liquidity_tokens(vec![token.clone()])?;
                if let Some(calculator) = liquidity_calculator{
                    config.set_bond_calculator(token.address.clone(),calculator)
                }else{
                    return Err(StdError::generic_err("you need a calculator to add a liquidity token"));
                }                
            },
            _ => return Err(StdError::generic_err("toggle token with a non token role"))
        }?;
    }else{
        match role.clone(){
            ManagingRole::ReserveToken{} => config.remove_reserve_token(token),
            ManagingRole::LiquidityToken{} => config.remove_liquidity_token(token),
            ManagingRole::SOHM{} => {

                config.set_managing_queue(&canonical_addr,role,0)?;
                let mut consts = config.constants()?;
                consts.sohm = token;
                config.set_constants(&consts)
            },


            _ => return Err(StdError::generic_err("toggle token with a non token role"))
        }?;
    }
    Ok(HandleResponse {
        messages : messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ToggleQueue { status: Success })?),
    })
}
fn query_contract_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    ) -> QueryResult {
    let config = ReadonlyConfig::from_storage(&deps.storage);
    let constants = config.constants()?;
    to_binary(&
         QueryAnswer::ContractInfo{
            name : constants.name.clone(),
            admin : constants.admin.clone(),
            prng_seed : constants.prng_seed.clone(),
            ohm : constants.ohm.clone(),
            sohm : constants.sohm.clone(),
            blocks_needed_for_queue : constants.blocks_needed_for_queue.clone(),
            total_reserves: Uint128(config.total_reserves()),
            total_debt: Uint128(config.total_debt()),
            excess_reserves: Uint128(config.excess_reserves(&deps.querier)?)
        }
    )
}
fn query_tokens<S: ReadonlyStorage>(storage: &S,role : ManagingRole) -> QueryResult {
    let config = ReadonlyConfig::from_storage(storage);
    let reserve_tokens = match role{
        ManagingRole::ReserveToken{} => config.reserve_tokens(),
        ManagingRole::LiquidityToken{} => config.liquidity_tokens(),
        ManagingRole::SOHM{} => vec![config.constants()?.sohm],
        _ => vec![]
    }; 
    to_binary(&QueryAnswer::TokensInfo{
        tokens : reserve_tokens
    })
}
fn query_managing_addresses<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    role : ManagingRole
    ) -> QueryResult {
    let config = ReadonlyConfig::from_storage(&deps.storage);

    let addresses : Vec<HumanAddr> = config.managing_addresses(role).iter().map(|x| deps.api.human_address(&x).unwrap()).collect();
    to_binary(&QueryAnswer::ManagersInfo{
        addresses,
    })
}

fn query_contract_status<S: ReadonlyStorage>(storage: &S) -> QueryResult {
    let config = ReadonlyConfig::from_storage(storage);

    to_binary(&QueryAnswer::ContractStatus {
        status: config.contract_status(),
    })
}

fn query_value_of<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    token: HumanAddr,
    amount: Uint128
    ) -> QueryResult {
    let config = ReadonlyConfig::from_storage(&deps.storage);

    to_binary(&QueryAnswer::ValueOf {
        value: Uint128(
            config.value_of(
                &deps.querier,
                &token,
                amount.u128()
            )?
        ),
    })
}

fn query_total_bond_deposited<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    token: HumanAddr
    ) -> QueryResult {
    
    let deposited = ReadonlyDeposited::from_storage(&deps.storage);   
    let token_address = deps.api.canonical_address(&token)?;

    to_binary(&QueryAnswer::TotalBondDeposited {
            amount: Uint128(deposited.deposited(&token_address)),
        
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

pub fn pad_response(response: StdResult<HandleResponse>) -> StdResult<HandleResponse> {
    response.map(|mut response| {
        response.data = response.data.map(|mut data| {
            space_pad(RESPONSE_BLOCK_SIZE, &mut data.0);
            data
        });
        response
    })
}

fn is_valid_name(name: &str) -> bool {
    let len = name.len();
    (3..=30).contains(&len)
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
            name: "treasury".to_string(),
            admin: None,
            prng_seed: Binary::from("lolz fun yay".as_bytes()),
            ohm : Contract{address:HumanAddr("ohm".to_string()),code_hash:"Complicated_hash".to_string()},
            sohm : Contract{address:HumanAddr("sohm".to_string()),code_hash:"Complicated_hash".to_string()},
            reserve_tokens : Some(vec![
                Contract{address:HumanAddr("sUST".to_string()),code_hash:"Complicated_hash".to_string()},
                Contract{address:HumanAddr("SSCRT".to_string()),code_hash:"Complicated_hash".to_string()},
                ]),
            liquidity_tokens : Some(vec![
                Contract{address:HumanAddr("sust-LP".to_string()),code_hash:"Complicated_hash".to_string()},
                ]),
            blocks_needed_for_queue : 0,
        };

        (init(&mut deps, env, init_msg), deps)
    }
    /// Will return a ViewingKey only for the first account in `initial_balances`
    /*
    fn _auth_query_helper(
        
    ) -> (ViewingKey, Extern<MockStorage, MockApi, MockQuerier>) {
        let (init_result, mut deps) = init_helper();
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let account = initial_balances[0].address.clone();
        let create_vk_msg = HandleMsg::CreateViewingKey {
            entropy: "42".to_string(),
            padding: None,
        };
        let handle_response = handle(&mut deps, mock_env(account.0, &[]), create_vk_msg).unwrap();
        let vk = match from_binary(&handle_response.data.unwrap()).unwrap() {
            HandleAnswer::CreateViewingKey { key } => key,
            _ => panic!("Unexpected result from handle"),
        };

        (vk, deps)
    }
    */

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
            HandleAnswer::Deposit { status }
            | HandleAnswer::Withdraw { status }
            | HandleAnswer::IncurDebt { status }
            | HandleAnswer::Manage { status }
            | HandleAnswer::MintRewards {status }
            | HandleAnswer::AuditReserves { status }
            | HandleAnswer::QueueAddress {status}
            | HandleAnswer::ToggleQueue {status}
            | HandleAnswer::ChangeAdmin { status }
            | HandleAnswer::SetContractStatus { status } => {
                matches!(status, ResponseStatus::Success { .. })
            }
        }
    }

    fn ensure_failure(handle_result: HandleResponse) -> bool {
        let handle_result: HandleAnswer = from_binary(&handle_result.data.unwrap()).unwrap();

        match handle_result {
            HandleAnswer::Deposit { status }
            | HandleAnswer::Withdraw { status }
            | HandleAnswer::IncurDebt { status }
            | HandleAnswer::Manage { status }
            | HandleAnswer::MintRewards {status }
            | HandleAnswer::AuditReserves { status }
            | HandleAnswer::QueueAddress {status}
            | HandleAnswer::ToggleQueue {status}
            | HandleAnswer::ChangeAdmin { status }
            | HandleAnswer::SetContractStatus { status } => {
                matches!(status, ResponseStatus::Failure { .. })
            }
        }
    }

    // Init tests

    #[test]
    fn test_init_sanity() {
        let (_init_result, deps) = init_helper();
        //assert_eq!(init_result.unwrap(), InitResponse::default());

        let config = ReadonlyConfig::from_storage(&deps.storage);
        let constants = config.constants().unwrap();
        
       
        assert_eq!(constants.name, "treasury".to_string());
        assert_eq!(constants.admin, HumanAddr("admin".to_string()));
        assert_eq!(
            constants.prng_seed,
            sha_256("lolz fun yay".to_owned().as_bytes())
        );
    }


    fn handle_queue_toggle_helper(init_result: StdResult<InitResponse>,
        deps: &mut Extern<MockStorage, MockApi, MockQuerier>){

        let handle_failure_msg = HandleMsg::Queue {
            address: HumanAddr("sUST".to_string()),
            role: ManagingRole::ReserveDepositor
        };

        let handle_failure_result = handle(deps, mock_env("bob", &[]), handle_failure_msg);
        assert!(!handle_failure_result.is_ok());

        // We queue the token
        let handle_msg = HandleMsg::Queue {
            address: HumanAddr("sUST".to_string()),
            role: ManagingRole::ReserveDepositor
        };
        let handle_result = handle(deps, mock_env("admin", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));

        //Now we toggle the token
        let toggle_handle_msg = HandleMsg::ToggleQueue {
            address: HumanAddr("sUST".to_string()),
            role: ManagingRole::ReserveDepositor
        };
        let handle_result = handle(deps, mock_env("admin", &[]), toggle_handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));
    }

    #[test]
    fn test_handle_queue_toggle(){
        let (init_result, mut deps) = init_helper();
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );
        handle_queue_toggle_helper(init_result,&mut deps);
        
    }


    #[test]
    fn test_deposit(){
        let (init_result, mut deps) = init_helper();
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );
        let deposit_msg = ReceiveMsg::Deposit{
            profit: Uint128(86945),
        };
        let handle_receive_msg = HandleMsg::Receive {
            sender : HumanAddr("sUST".to_string()),
            from : HumanAddr("admin".to_string()),
            amount: Uint128(7000000),
            msg: to_binary(&deposit_msg).unwrap()
        };
        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_receive_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));


    }

}
