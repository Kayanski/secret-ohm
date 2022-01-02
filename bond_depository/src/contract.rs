/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use std::convert::TryInto;
use cosmwasm_std::{
    to_binary, from_binary, Api, Binary, Env, Extern,
    HandleResponse, HumanAddr, InitResponse, Querier, QueryResult, ReadonlyStorage, StdError,
    StdResult, Storage, Uint128,
};

use crate::msg::QueryWithPermit;
use crate::msg::{
    space_pad, HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg, ReceiveMsg,
    ResponseStatus::Success, TreasuryHandleMsg, StakingHandleMsg,
    TreasuryQueryMsg, BondCalculatorQueryMsg,
    ValueOfResponse, MarkdownResponse,
    BondParameter,
    RESPONSE_BLOCK_SIZE
};
use crate::rand::sha_256;
use crate::state::{
    read_viewing_key, 
    write_viewing_key, Config, Constants, ReadonlyConfig,
    Adjust, Terms, Contract, Bond, BondInfo, ReadonlyBondInfo
};
use secret_toolkit::snip20;
use crate::viewing_key::{ViewingKey, VIEWING_KEY_SIZE};
use secret_toolkit::permit::{validate, Permission, Permit, RevokedPermits};
use secret_toolkit::utils::{Query};

pub const PREFIX_REVOKED_PERMITS: &str = "revoked_permits";
pub const COMMON_VIEWING_KEY : &str = "ALL_ORGANISATION_INFO_SHOULD_BE_PUBLIC";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {

    let admin = msg.admin.unwrap_or(env.message.sender);
    let prng_seed_hashed = sha_256(&msg.prng_seed.0);

    let mut config = Config::from_storage(&mut deps.storage);
    config.set_constants(&Constants {
        ohm: msg.ohm.clone(),
        principle: msg.principle.clone(),
        treasury: msg.treasury.clone(),
        dao: msg.dao.clone(),
        bond_calculator: msg.bond_calculator.clone(),
        staking: None,
        terms: None,
        adjustment: None,

        admin: admin,
        prng_seed: prng_seed_hashed.to_vec(),
        contract_address: env.contract.address,
    })?;

    Ok(InitResponse::default())
}


pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {

    let response = match msg {

        //Register Receive messages
        HandleMsg::Receive {
                    from, amount, msg,..
                } => receive(deps, env, from, amount.u128(), msg),


        HandleMsg::InitializeBondTerms{
            control_variable,vesting_term,minimum_price,
            max_payout, fee, max_debt, initial_debt
        } => initialize_bond_terms(
            deps,env,
            control_variable.u128(),vesting_term,minimum_price.u128(),
            max_payout.u128(), fee.u128(), max_debt.u128(), initial_debt.u128()
        ),

        HandleMsg::SetBondTerm{parameter,input} => set_bond_terms(deps,env, parameter, input.u128()),

        HandleMsg::SetAdjustment{addition,increment, target, buffer} =>
            set_adjustment(deps,env,addition,increment.u128(), target.u128(), buffer),
        HandleMsg::SetStaking{staking} => set_staking(deps,env,staking),
        HandleMsg::Redeem{recipient,stake} => redeem(deps,env,recipient,stake),
        HandleMsg::RecoverLostToken{token} => recover_lost_token(deps,env,token),

        // Other
        HandleMsg::ChangeAdmin { address, .. } => change_admin(deps, env, address),
        HandleMsg::RevokePermit { permit_name, .. } => revoke_permit(deps, env, permit_name),
    };

    pad_response(response)
}


pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::ContractInfo {} => query_contract_info(&deps.storage),
        QueryMsg::MaxPayout{} => query_max_payout(deps),
        QueryMsg::PayoutFor{block_height, value} => query_payout_for(deps,block_height, value.u128()),
        QueryMsg::BondPrice{block_height} => query_bond_price(deps,block_height),
        QueryMsg::BondPriceInUsd{block_height} => query_bond_price_in_usd(deps,block_height),
        QueryMsg::DebtRatio{block_height} => query_debt_ratio(deps,block_height),
        QueryMsg::StandardizedDebtRatio{block_height} => query_standardized_debt_ratio(deps,block_height),
        QueryMsg::CurrentDebt{block_height} => query_current_debt(deps,block_height),
        QueryMsg::DebtDecay{block_height} => query_debt_decay(deps,block_height),

        QueryMsg::WithPermit { permit, query } => permit_queries(deps, permit, query),
        _ => viewing_keys_queries(deps, msg),
    }
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

pub fn initialize_bond_terms<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    control_variable: u128, 
    vesting_term: u64,
    minimum_price: u128,
    max_payout: u128,
    fee: u128,
    max_debt: u128, 
    initial_debt: u128
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    let mut consts = config.constants()?;
    if consts.terms != None {
        return Err(StdError::generic_err("Bonds must be initialized from 0"));
    }
    consts.terms = Some(Terms{
        control_variable:Uint128(control_variable),
        vesting_term,
        minimum_price: Uint128(minimum_price),
        max_payout:Uint128(max_payout),
        fee:Uint128(fee),
        max_debt:Uint128(max_debt),
    });
    config.set_total_debt(initial_debt);
    config.set_last_decay(env.block.height);
    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::InitializeBondTerms{ status: Success })?),
    })
}
  
   
pub fn set_bond_terms<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    parameter: BondParameter,
    input: u128,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    let mut consts = config.constants()?;
    let mut terms = consts.terms.unwrap();
    match parameter{
        BondParameter::Vesting => {
            if input < 10000 {
                return Err(StdError::generic_err("Vesting must be longer than 36 hours" ));
            }
            terms.vesting_term = input.try_into().unwrap();
        },
        BondParameter::Payout => {
            if input > 1000 {
                return Err(StdError::generic_err("Vesting must be longer than 36 hours" ));
            }
            terms.max_payout = Uint128(input);
        },
        BondParameter::Fee => {
            if input > 10000 {
                return Err(StdError::generic_err("Vesting must be longer than 36 hours" ));
            }
            terms.fee = Uint128(input);
        },
        BondParameter::Debt => {
            terms.max_debt = Uint128(input);
            
        },
    }
    consts.terms = Some(terms);
    config.set_constants(&consts)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetBondTerms{ status: Success })?),
    })
}

pub fn set_adjustment<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    addition: bool,
    increment: u128,
    target: u128,
    buffer: u64,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    let mut consts = config.constants()?;
    if increment > consts.terms.clone().unwrap().control_variable.u128()*25/1000{
        return Err(StdError::generic_err("Increment too large"));
    }
    consts.adjustment = Some(Adjust{
        add: addition,
        rate: Uint128(increment),
        target: Uint128(target),
        buffer: buffer,
        last_block: env.block.height,
    });
    config.set_constants(&consts)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetAdjustment{ status: Success })?),
    })
}

pub fn set_staking<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    staking: Contract,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);
    check_if_admin(&config,&env.message.sender)?;
    let mut consts = config.constants()?;
    consts.staking = Some(staking);

    config.set_constants(&consts)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetStaking{ status: Success })?),
    })
}

/* register receive after receiving principle from someone */
/*IERC20( principle ).safeTransferFrom( msg.sender, address(this), _amount );*/
pub fn deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    token: HumanAddr,
    amount: u128,
    max_price: u128,
    depositor: HumanAddr,
) -> StdResult<HandleResponse> {
    decay_debt(deps,env.block.height)?;
    let config = ReadonlyConfig::from_storage(&deps.storage);
    let consts = config.constants()?;
    let terms = consts.terms.unwrap();
    let block_height = env.block.height;
    if token != consts.principle.address{
        return Err(StdError::generic_err(format!("This bond is only for the token: {}",token)));
    }
    if config.total_debt() > terms.max_debt.u128(){
        return Err(StdError::generic_err("Max capacity reached"));
    }
    let prince_in_usd = bond_price_in_usd(deps,env.block.height)?;
    let native_price = _bond_price(deps,&env)?;

    if max_price < native_price{
         return Err(StdError::generic_err("Slippage limit: more than max price"));
    }

    let value_of_query_msg = TreasuryQueryMsg::ValueOf{token: consts.principle.address.clone(), amount:Uint128(amount)};
    let value_of_response: ValueOfResponse = value_of_query_msg.query(
        &deps.querier,
        consts.treasury.code_hash.clone(),
        consts.treasury.address.clone(),
    ).unwrap();
    let value = value_of_response.value_of.value.u128();

    let payout = payout_for(deps,env.block.height,value)?;

    if payout < 10_000_000{
         return Err(StdError::generic_err("Bond too small"));
    }

    if payout > max_payout(deps)?{
         return Err(StdError::generic_err("Bond too large"));
    }

    // profits are calculated
    let fee = payout.checked_mul( terms.fee.u128() ).ok_or_else(||{
        StdError::generic_err("The fee is too high, sorry, check your privilege")
    })?/10_000;

    let profit = value.checked_sub( payout ).ok_or_else(||{
        StdError::generic_err("No profit, too much payout")
    })?.checked_sub(fee).ok_or_else(||{
        StdError::generic_err("No profit, too much fee")
    })?;

    let mut messages = vec![];
    /*
        principle is already transferred, so we deposit it in the treasury
        We return payout OHM to the Dao
    */
    messages.push(
        snip20::send_msg(
            consts.treasury.address,
            Uint128(amount),
            Some(to_binary(
                &TreasuryHandleMsg::Deposit{
                    profit:Uint128(profit)
                }
            )?),
            None,
            RESPONSE_BLOCK_SIZE,
            consts.principle.code_hash,
            consts.principle.address
        )?
    );

    if  fee != 0  { // fee is transferred to dao 
        messages.push(snip20::transfer_msg(
            consts.dao,
            Uint128(fee),
            None,
            RESPONSE_BLOCK_SIZE,
            consts.ohm.code_hash,
            consts.ohm.address
        )?);
    }
    

    let mut config = Config::from_storage(&mut deps.storage);
    // total debt is increased
    config.set_total_debt(config.total_debt().checked_add(value).ok_or_else(||{
        StdError::generic_err("Too much bond debt")
    })?); 

    let canon_depositor = deps.api.canonical_address(&depositor)?;
    let mut bonds = BondInfo::from_storage(&mut deps.storage);
    bonds.set_bond(&canon_depositor,Bond{
        payout : Uint128(bonds.bond(&canon_depositor).payout.u128()
        .checked_add(value).ok_or_else(||{
            StdError::generic_err("Too much bond debt")
        })?), 
        vesting: terms.vesting_term,
        last_block: block_height,
        price_paid: Uint128(prince_in_usd)
    })?; 
        
    adjust(deps,env)?; // control variable is adjusted

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Deposit{ status: Success, payout: Uint128(payout) })?),
    })
}

pub fn redeem<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient:  HumanAddr,
    stake: bool,
) -> StdResult<HandleResponse> {
    let bonds = ReadonlyBondInfo::from_storage(& deps.storage);
    let canon_recipient = deps.api.canonical_address(&recipient)?;
    let info = bonds.bond(&canon_recipient);
    let block_height = env.block.height;
    let percent_vested = percent_vested_for(deps,block_height,recipient.clone())?;
    let stake_or_send_return;
    let payout;
    let bond_to_set;
    let mut messages = vec![];
    if  percent_vested >= 10_000  { // if fully vested
        payout = info.payout;
        bond_to_set = Bond::default();
        stake_or_send_return = stake_or_send(deps, recipient, stake, info.payout.u128() ); // pay user everything due
        messages.extend(stake_or_send_return?.messages);
    } else { // if unfinished
        // calculate payout vested
        payout = Uint128(info.payout.u128() *  percent_vested / 10_000);
        bond_to_set = Bond{
            payout: (info.payout - payout)?,
            vesting: info.vesting - (block_height - info.last_block),
            last_block: block_height,
            price_paid: info.price_paid
        };
        stake_or_send_return = stake_or_send(deps,recipient, stake, payout.u128() );
        messages.extend(stake_or_send_return?.messages);
    }
    //Update user info
    let mut bonds = BondInfo::from_storage(&mut deps.storage);
    bonds.set_bond(&canon_recipient, bond_to_set)?; 

    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Redeem{ status: Success, payout: payout})?),
    })
}
pub fn stake_or_send<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    recipient:  HumanAddr,
    stake: bool,
    amount: u128
) -> StdResult<HandleResponse> {
    let mut messages = vec![];
    let consts = ReadonlyConfig::from_storage(&mut deps.storage).constants()?;
    if !stake{ // if user does not want to stake
        messages.push(snip20::transfer_msg(
            recipient,
            Uint128(amount),
            None,
            RESPONSE_BLOCK_SIZE,
            consts.ohm.code_hash,
            consts.ohm.address
            )?
        );
    } else { // if user wants to stake 
        messages.push(snip20::send_msg(
            consts.staking.unwrap().address,
            Uint128(amount),
            Some(to_binary(
                &StakingHandleMsg::Stake{
                    recipient
                }
            )?),
            None,
            RESPONSE_BLOCK_SIZE,
            consts.ohm.code_hash,
            consts.ohm.address
        )?);
    }

    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::StakeOrSend{ status: Success })?),
    })
}

pub fn adjust<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<()> {
    let mut consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let mut adjustment = consts.adjustment.unwrap();
    let mut terms = consts.terms.unwrap();

    let block_can_adjust = adjustment.last_block + adjustment.buffer;
    if adjustment.rate.u128() != 0 && env.block.height >= block_can_adjust{
        if  adjustment.add {
            terms.control_variable = terms.control_variable + adjustment.rate;
            if terms.control_variable >= adjustment.target {
                adjustment.rate = Uint128(0);
            }
        } else {
            terms.control_variable = (terms.control_variable - adjustment.rate)?;
            if terms.control_variable <= adjustment.target {
                adjustment.rate = Uint128(0);
            }
        }
        adjustment.last_block = env.block.height;
        consts.terms = Some(terms);
        consts.adjustment = Some(adjustment);
        let mut config = Config::from_storage(&mut deps.storage);
        config.set_constants(&consts)?;
    }
    Ok(())
}
pub fn decay_debt<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    block_height: u64
) -> StdResult<()> {
    let config = ReadonlyConfig::from_storage(&deps.storage);
    let new_debt = config.total_debt().checked_sub(debt_decay(deps,block_height)?).ok_or_else(||{
            StdError::generic_err("not enough debt to decay")
        }
    )?;
    let mut config = Config::from_storage(&mut deps.storage);
    config.set_total_debt(new_debt);
    config.set_last_decay(block_height);
    Ok(())
}

pub fn max_payout<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<u128> {
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    ohm_total_supply(&deps.querier,&consts.ohm)?
    .checked_mul(consts.terms.unwrap().max_payout.u128())
    .ok_or_else(||{
            StdError::generic_err("too much payout")
        }
    )?
    .checked_div(100_000 )
    .ok_or_else(||{
            StdError::generic_err("Unaccessible Error")
        }
    )
}

fn query_max_payout<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> QueryResult {
    to_binary(&QueryAnswer::MaxPayout {
        payout: Uint128(max_payout(deps)?),
    })
}

pub fn payout_for<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
    value: u128
) -> StdResult<u128> {
    value
    .checked_div(bond_price(deps,block_height)?)
    .ok_or_else(||{
            StdError::generic_err("too much payout")
        }
    )?
    .checked_div(10_u128.pow(16))
    .ok_or_else(||{
            StdError::generic_err("Unaccessible Error")
        }
    )
}

fn query_payout_for<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
    value: u128
) -> QueryResult {
    to_binary(&QueryAnswer::PayoutFor {
        payout: Uint128(payout_for(deps,block_height,value)?),
    })
}

pub fn bond_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64
) -> StdResult<u128> {
    let terms = ReadonlyConfig::from_storage(&deps.storage).constants()?.terms.unwrap();
    let mut price = terms.control_variable.u128()
    .checked_mul(debt_ratio(deps, block_height)?)
    .ok_or_else(||{
            StdError::generic_err("too much payout")
        }
    )?
    .checked_add(1_000_000_000)
    .ok_or_else(||{
            StdError::generic_err("BondPrice too high")
        }
    )?
    .checked_div(10_000_000)
    .ok_or_else(||{
            StdError::generic_err("Unaccessible Error")
        }
    )?;
    if price < terms.minimum_price.u128(){
        price = terms.minimum_price.u128()
    }
    Ok(price)
}

fn query_bond_price<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
) -> QueryResult {
    to_binary(&QueryAnswer::BondPrice {
        price: Uint128(bond_price(deps,block_height)?),
    })
}
/**
 *  @notice calculate current bond price and remove floor if above
 *  @return price_ uint
 */
 pub fn _bond_price<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
) -> StdResult<u128> {
    let config = ReadonlyConfig::from_storage(&deps.storage);
    let mut consts = config.constants()?;
    let mut terms = consts.terms.unwrap();
    let mut price = terms.control_variable.u128()
    .checked_mul(debt_ratio(deps, env.block.height)?)
    .ok_or_else(||{
            StdError::generic_err("too much payout")
        }
    )?
    .checked_add(1_000_000_000)
    .ok_or_else(||{
            StdError::generic_err("BondPrice too high")
        }
    )?
    .checked_div(10_000_000)
    .ok_or_else(||{
            StdError::generic_err("Unaccessible Error")
        }
    )?;
    if price < terms.minimum_price.u128(){
        price = terms.minimum_price.u128()
    }else if terms.minimum_price.u128() != 0{
        terms.minimum_price = Uint128(0);
    }

    let mut config = Config::from_storage(&mut deps.storage);
    consts.terms = Some(terms);
    config.set_constants(&consts)?;
    Ok(price)
}

pub fn bond_price_in_usd<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64
) -> StdResult<u128> {

    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let bond_calculator = consts.bond_calculator;
    let price;
    if let Some(bond_calculator) = bond_calculator{
        // Query the bondcalculator for the bound price
        let markdown_query_msg = BondCalculatorQueryMsg::Markdown{principle:consts.principle};
        let markdown_response: MarkdownResponse = markdown_query_msg.query(
            &deps.querier,
            bond_calculator.code_hash.clone(),
            bond_calculator.address.clone(),
        )?;

        price = bond_price(deps,block_height)?
        .checked_mul(markdown_response.markdown.price.u128())
        .ok_or_else(||{
                StdError::generic_err("BondPrice too high")
            }
        )?
        .checked_div(100_u128)
        .ok_or_else(||{
                StdError::generic_err("BondPrice too high")
            }
        )?;
    }else{
        let decimals = snip20::token_info_query(
            &deps.querier,
            RESPONSE_BLOCK_SIZE,
            consts.principle.code_hash,
            consts.principle.address,
        )?.decimals;
        price = bond_price(deps,block_height)?
        .checked_mul(10_u128.pow(decimals.into()))
        .ok_or_else(||{
                StdError::generic_err("BondPrice too high")
            }
        )?
        .checked_div(100_u128)
        .ok_or_else(||{
                StdError::generic_err("BondPrice too high")
            }
        )?;
    }
    Ok(price)
}

fn query_bond_price_in_usd<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
) -> QueryResult {
    to_binary(&QueryAnswer::BondPriceInUsd {
        price: Uint128(bond_price_in_usd(deps,block_height)?),
    })
}

pub fn debt_ratio<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64
) -> StdResult<u128> {
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let supply = ohm_total_supply(&deps.querier,&consts.ohm)?;
    current_debt(deps,block_height)?
    .checked_mul(10_u128.pow(9_u32))
    .ok_or_else(||{
            StdError::generic_err("BondPrice too high")
        }
    )?
    .checked_div(supply)
    .ok_or_else(||{
            StdError::generic_err("BondPrice too high")
        }
    )?
    .checked_div(10_u128.pow(18_u32))
    .ok_or_else(||{
            StdError::generic_err("BondPrice too high")
        }
    )
}

fn query_debt_ratio<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
) -> QueryResult {
    to_binary(&QueryAnswer::DebtRatio {
        ratio: Uint128(debt_ratio(deps,block_height)?),
    })
}

pub fn standardized_debt_ratio<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64
) -> StdResult<u128> {
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let bond_calculator = consts.bond_calculator;
    let ratio;
    if let Some(bond_calculator) = bond_calculator{
        // Query the bondcalculator for the bound price
        let markdown_query_msg = BondCalculatorQueryMsg::Markdown{principle:consts.principle};
        let markdown_response: MarkdownResponse = markdown_query_msg.query(
            &deps.querier,
            bond_calculator.code_hash.clone(),
            bond_calculator.address.clone(),
        )?;

        ratio = debt_ratio(deps, block_height)?
        .checked_mul(markdown_response.markdown.price.u128())
        .ok_or_else(||{
                StdError::generic_err("ratio too high")
            }
        )?
        .checked_div(10_u128.pow(9_u32))
        .ok_or_else(||{
                StdError::generic_err("Error not reachable")
            }
        )?;
    }else{
        ratio = debt_ratio(deps, block_height)?;
    }
    Ok(ratio)
}


fn query_standardized_debt_ratio<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
) -> QueryResult {
    to_binary(&QueryAnswer::StandardizedDebtRatio {
        ratio: Uint128(standardized_debt_ratio(deps,block_height)?),
    })
}

pub fn current_debt<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64
) -> StdResult<u128> {
    let config = ReadonlyConfig::from_storage(&deps.storage);
    config.total_debt()
    .checked_sub(debt_decay(deps, block_height)?)
    .ok_or_else(||{
            StdError::generic_err("Debt less than zero")
        }
    )
}

fn query_current_debt<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
) -> QueryResult {
    to_binary(&QueryAnswer::CurrentDebt {
        debt: Uint128(current_debt(deps,block_height)?),
    })
}

pub fn debt_decay<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64
) -> StdResult<u128> {
    let config = ReadonlyConfig::from_storage(&deps.storage);
    let blocks_since_last = block_height - config.last_decay();
    let total_debt = config.total_debt();
    let mut decay = total_debt
    .checked_mul(blocks_since_last.into())
    .ok_or_else(||{
            StdError::generic_err("too much blocks since last decay. The contract is down...")
        }
    )?
    .checked_div(config.constants()?.terms.unwrap().vesting_term.into())
    .ok_or_else(||{
            StdError::generic_err("Vesting term is zero. The contract is not initialized correctly")
        }
    )?;
    if decay > total_debt{
        decay = total_debt
    }
    Ok(decay)
}

fn query_debt_decay<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
) -> QueryResult {
    to_binary(&QueryAnswer::DebtDecay {
        decay: Uint128(debt_decay(deps,block_height)?),
    })
}

pub fn percent_vested_for<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
    depositor: HumanAddr
) -> StdResult<u128> {
    let canon_depositor = deps.api.canonical_address(&depositor)?;
    let bond = ReadonlyBondInfo::from_storage(&deps.storage).bond(&canon_depositor);
    let blocks_since_last = block_height - bond.last_block;
    let vesting = bond.vesting;
    let percent_vested: u128;

    if vesting > 0 {
        percent_vested = blocks_since_last
        .checked_mul(10_000)
        .ok_or_else(||{
                StdError::generic_err("too much blocks since last decay. The contract is down...")
            }
        )?
        .checked_div(vesting.into())
        .ok_or_else(||{
                StdError::generic_err("Vesting bond is zero. The contract is not initialized correctly")
            }
        )?.into();
    } else {
            percent_vested = 0;
    }
    Ok(percent_vested)
}

fn query_percent_vested_for<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
    depositor: HumanAddr
) -> QueryResult {
    to_binary(&QueryAnswer::PercentVestedFor {
        percent: Uint128(percent_vested_for(deps,block_height,depositor)?),
    })
}


pub fn query_pending_payout_for<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    block_height: u64,
    depositor: HumanAddr
) -> QueryResult {
    let percent_vested = percent_vested_for(deps,block_height,depositor.clone())?;
    let canon_depositor = deps.api.canonical_address(&depositor)?;
    let payout = ReadonlyBondInfo::from_storage(&deps.storage).bond(&canon_depositor).payout;
    let pending_payout;
    if percent_vested >= 10000{
        pending_payout = payout;
    } else {
        pending_payout = Uint128(payout.u128() * percent_vested / 10_000);
    }
    to_binary(&QueryAnswer::PendingPayoutFor {
        payout: pending_payout,
    })
}

pub fn recover_lost_token<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    token: Contract
) -> StdResult<HandleResponse>{
    let consts = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    if token == consts.ohm || token == consts.ohm{
        return Err(StdError::generic_err("You can only recover token that are not the treasury token or the principle token",
        ));
    }
    let contract_balance = snip20::balance_query(
        &deps.querier,
        env.contract.address,
        COMMON_VIEWING_KEY.to_string(),
        RESPONSE_BLOCK_SIZE,
        token.code_hash.clone(),
        token.address.clone()
    )?.amount;
    let messages = vec![
        snip20::transfer_msg(
            consts.dao,
            contract_balance,
            None,
            RESPONSE_BLOCK_SIZE,
            token.code_hash,
            token.address
        )?
    ];

    Ok(HandleResponse {
        messages: messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::RecoverLostToken { status: Success })?),
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
    match msg {
        ReceiveMsg::Deposit {max_price, depositor } => deposit(deps, env, token, amount, max_price.u128(), depositor.unwrap_or(from)),
    }
}

fn ohm_total_supply<Q: Querier>(
    querier: &Q,
    ohm: &Contract, 
    ) -> StdResult<u128>{

    let token_info = snip20::token_info_query(
        querier,
        RESPONSE_BLOCK_SIZE,
        ohm.code_hash.clone(),
        ohm.address.clone(),
    )?;
    Ok(token_info.total_supply.unwrap_or_default().u128())
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
       
        QueryWithPermit::BondInfo{} => {
            if !permit.check_permission(&Permission::Balance) {
                return Err(StdError::generic_err(format!(
                    "No permission to query rate, got permissions {:?}",
                    permit.params.permissions
                )));
            }

            query_bond_info(deps, account)
        }
        QueryWithPermit::PercentVestedFor{
            block_height
        } => {
            if !permit.check_permission(&Permission::Balance) {
                return Err(StdError::generic_err(format!(
                    "No permission to query rate, got permissions {:?}",
                    permit.params.permissions
                )));
            }

            query_percent_vested_for(deps, block_height, account)
        }
        QueryWithPermit::PendingPayoutFor{
            block_height
        } => {
            if !permit.check_permission(&Permission::Balance) {
                return Err(StdError::generic_err(format!(
                    "No permission to query rate, got permissions {:?}",
                    permit.params.permissions
                )));
            }

            query_pending_payout_for(deps, block_height, account)
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
                QueryMsg::BondInfo { address, .. } => query_bond_info(deps, address),
                QueryMsg::PercentVestedFor { address, block_height, .. } 
                    => query_percent_vested_for(deps, block_height, address),
                QueryMsg::PendingPayoutFor { address, block_height, .. } 
                    => query_pending_payout_for(deps, block_height, address),
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
        ohm: constants.ohm,
        principle: constants.principle,
        treasury: constants.treasury,
        dao: constants.dao,
        bond_calculator: constants.bond_calculator,
        staking: constants.staking,
        terms: constants.terms,
        adjustment: constants.adjustment,

        admin: constants.admin,
        total_debt: Uint128(config.total_debt()),
        last_decay: config.last_decay(),
    })
}

fn query_bond_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    recipient: HumanAddr
) -> QueryResult {
    let canon_recipient = deps.api.canonical_address(&recipient)?;
    let bond = ReadonlyBondInfo::from_storage(&deps.storage).bond(&canon_recipient);

    to_binary(&QueryAnswer::Bond {
        payout: bond.payout, 
        vesting: bond.vesting,
        last_block: bond.last_block,
        price_paid: bond.price_paid,
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