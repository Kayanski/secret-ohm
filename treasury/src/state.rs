use std::any::type_name;
use std::convert::TryFrom;

use cosmwasm_std::{CanonicalAddr, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage, Querier};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage, bucket, bucket_read};

use secret_toolkit::storage::{TypedStore, TypedStoreMut};
use secret_toolkit::snip20;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{status_level_to_u8, u8_to_status_level, ContractStatusLevel};
use crate::viewing_key::ViewingKey;
use serde::de::DeserializeOwned;

pub static CONFIG_KEY: &[u8] = b"config";
pub const PREFIX_TXS: &[u8] = b"transfers";

pub const KEY_CONSTANTS: &[u8] = b"constants";
pub const KEY_TOTAL_RESERVES: &[u8] = b"total_reserves";
pub const KEY_EXCESS_RESERVES: &[u8] = b"excess_reserves";
pub const KEY_TOTAL_DEBT: &[u8] = b"total_debt";
pub const KEY_CONTRACT_STATUS: &[u8] = b"contract_status";
pub const KEY_MINTERS: &[u8] = b"minters";
pub const KEY_TX_COUNT: &[u8] = b"tx-count";
pub const KEY_RESERVE_TOKENS: &[u8] = b"reserve_tokens";
pub const KEY_LIQUIDITY_TOKENS: &[u8] = b"liquidity_tokens";
pub const KEY_BOND_CALCULATOR: &[u8] = b"bond_calculator";

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_DEBTORS: &[u8] = b"debtors";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";
pub const PREFIX_VIEW_KEY: &[u8] = b"viewingkey";
pub const PREFIX_RECEIVERS: &[u8] = b"receivers";

pub const QUEUE_POSTFIX : &str = "_queue";
pub const POSITION_POSTFIX : &str = "_position";
pub const MANAGING_ROLE_POSTFIX : &str = "_managing"; 

pub const RESPONSE_BLOCK_SIZE: usize = 256;

//Bond Valuation

pub fn get_bond_valuation(
    _token : Contract,
    amount : u128
) -> u128 {
    amount
}
//Maybe try to not have duplicate methods BEG


// Config

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Contract{
    pub address : HumanAddr,
    pub code_hash : String
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct Constants {
    pub name: String,
    pub admin: HumanAddr,
    pub prng_seed: Vec<u8>,
    pub ohm : Contract,
    pub sohm : Contract,
    // the address of this contract, used to validate query permits
    pub contract_address: HumanAddr,
    // Blocks needed to accept a new managing role
    pub blocks_needed_for_queue : u64,
}
#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, JsonSchema,strum_macros::Display)]
pub enum ManagingRole{
    ReserveDepositor,
    ReserveSpender,
    ReserveToken,
    ReserveManager,
    LiquidityDepositor,
    LiquidityToken,
    LiquidityManager,
    Debtor,
    RewardManager,
    SOHM
}


pub struct ReadonlyConfig<'a, S: ReadonlyStorage> {
    storage: ReadonlyPrefixedStorage<'a, S>,
}

impl<'a, S: ReadonlyStorage> ReadonlyConfig<'a, S> {
    pub fn from_storage(storage: &'a S) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(PREFIX_CONFIG, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyConfigImpl<ReadonlyPrefixedStorage<S>> {
        ReadonlyConfigImpl(&self.storage)
    }

    pub fn constants(&self) -> StdResult<Constants> {
        self.as_readonly().constants()
    }


    pub fn managing_addresses(&self, role : ManagingRole) -> Vec<CanonicalAddr> {
        self.as_readonly().managing_addresses(role)
    }

    pub fn managing_position(&self,address: &CanonicalAddr,role : ManagingRole) -> Option<bool> {
        self.as_readonly().managing_position(address,role)
    }

    pub fn has_managing_position(&self,address: &CanonicalAddr,role : ManagingRole) -> bool {
        self.as_readonly().has_managing_position(address,role)
    }

    pub fn managing_queue(&self,address: &CanonicalAddr,role : ManagingRole) -> StdResult<u64> {
        self.as_readonly().managing_queue(address,role)
    }

    pub fn total_reserves(&self) -> u128 {
        self.as_readonly().total_reserves()
    }

    pub fn excess_reserves<Q: Querier> (&self, querier : &Q) -> StdResult<u128> {
        self.as_readonly().excess_reserves(querier)
    }

    pub fn total_debt(&self) -> u128 {
        self.as_readonly().total_debt()
    }

    pub fn contract_status(&self) -> ContractStatusLevel {
        self.as_readonly().contract_status()
    }

    pub fn minters(&self) -> Vec<HumanAddr> {
        self.as_readonly().minters()
    }

    pub fn tx_count(&self) -> u64 {
        self.as_readonly().tx_count()
    }

    pub fn reserve_tokens(&self) -> Vec<Contract> {
        self.as_readonly().reserve_tokens()
    }

    pub fn get_reserve_token_info(&self, token : HumanAddr) -> StdResult<Contract>{
        self.as_readonly().get_reserve_token_info(token)
    }

    pub fn is_reserve_token(&self,token : HumanAddr) -> bool {
        self.as_readonly().is_reserve_token(token)
    }

    pub fn liquidity_tokens(&self) -> Vec<Contract> {
        self.as_readonly().liquidity_tokens()
    }

    pub fn is_liquidity_token(&self,token : HumanAddr) -> bool {
        self.as_readonly().is_liquidity_token(token)
    }

    pub fn bond_calculator(&self, token : HumanAddr) -> Contract{
        self.as_readonly().bond_calculator(token)
    }

    pub fn value_of<Q: Querier> (&self, querier: &Q,token : Contract,amount:u128) -> StdResult<u128>{
        self.as_readonly().value_of(querier,token,amount)
    }
}

fn ser_bin_data<T: Serialize>(obj: &T) -> StdResult<Vec<u8>> {
    bincode2::serialize(&obj).map_err(|e| StdError::serialize_err(type_name::<T>(), e))
}

fn deser_bin_data<T: DeserializeOwned>(data: &[u8]) -> StdResult<T> {
    bincode2::deserialize::<T>(&data).map_err(|e| StdError::serialize_err(type_name::<T>(), e))
}

fn set_bin_data<T: Serialize, S: Storage>(storage: &mut S, key: &[u8], data: &T) -> StdResult<()> {
    let bin_data = ser_bin_data(data)?;

    storage.set(key, &bin_data);
    Ok(())
}

fn get_bin_data<T: DeserializeOwned, S: ReadonlyStorage>(storage: &S, key: &[u8]) -> StdResult<T> {
    let bin_data = storage.get(key);

    match bin_data {
        None => Err(StdError::not_found("Key not found in storage")),
        Some(bin_data) => Ok(deser_bin_data(&bin_data)?),
    }
}

pub struct Config<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> Config<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(PREFIX_CONFIG, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyConfigImpl<PrefixedStorage<S>> {
        ReadonlyConfigImpl(&self.storage)
    }

    pub fn constants(&self) -> StdResult<Constants> {
        self.as_readonly().constants()
    }
    
    fn set_managing_addresses(&mut self,addresses : Vec<CanonicalAddr>, role : ManagingRole) -> StdResult<()> {
        let role_string : String = role.to_string() + MANAGING_ROLE_POSTFIX;
        set_bin_data(&mut self.storage, &role_string.into_bytes(), &addresses)
    }
    
    fn add_managing_addresses(&mut self,addresses_to_add: Vec<CanonicalAddr>,role : ManagingRole) -> StdResult<()> {
        let mut addresses = self.managing_addresses(role.clone());
        addresses.extend(addresses_to_add);
        self.set_managing_addresses(addresses,role)
    }

    pub fn remove_managing_addresses(&mut self, addresses_to_remove: Vec<CanonicalAddr>, role : ManagingRole) -> StdResult<()> {
        let mut addresses = self.managing_addresses(role.clone());

        for address in addresses_to_remove {
            addresses.retain(|x| x != &address);
        }

        self.set_managing_addresses(addresses,role)
    }

    pub fn managing_addresses(&mut self,role : ManagingRole) -> Vec<CanonicalAddr>{
        self.as_readonly().managing_addresses(role)
    }

    pub fn managing_position(&self,address: &CanonicalAddr,role : ManagingRole) -> Option<bool> {
        self.as_readonly().managing_position(address,role)
    }

    pub fn has_managing_position(&self,address: &CanonicalAddr,role : ManagingRole) -> bool {
        self.as_readonly().has_managing_position(address,role)
    }

    pub fn managing_queue(&self,address: &CanonicalAddr,role : ManagingRole) -> StdResult<u64> {
        self.as_readonly().managing_queue(address,role)
    }

    pub fn set_managing_position(&mut self,address: &CanonicalAddr,role : ManagingRole,value : bool) -> StdResult<()> {
        let role_string : String = role.to_string() + POSITION_POSTFIX;
        bucket(&role_string.into_bytes(), &mut self.storage).save(address.as_slice(),&value)?;
        self.remove_managing_addresses(vec![address.clone()],role.clone())?;
        if value{
            self.add_managing_addresses(vec![address.clone()],role)
        }else{
            Ok(())
        }
    }

    pub fn set_managing_queue(&mut self,address: &CanonicalAddr,role : ManagingRole,value : u64) -> StdResult<()> {
        let role_string : String = role.to_string() + QUEUE_POSTFIX;
        bucket(&role_string.into_bytes(), &mut self.storage).save(address.as_slice(),&value)
    }

    pub fn set_constants(&mut self, constants: &Constants) -> StdResult<()> {
        set_bin_data(&mut self.storage, KEY_CONSTANTS, constants)
    }

    pub fn total_reserves(&self) -> u128 {
        self.as_readonly().total_reserves()
    }

    pub fn excess_reserves<Q: Querier> (&self, querier : &Q) -> StdResult<u128> {
        self.as_readonly().excess_reserves(querier)
    }

    pub fn total_debt(&self) -> u128 {
        self.as_readonly().total_debt()
    }

    pub fn set_total_reserves(&mut self, reserve: u128) {
        self.storage.set(KEY_TOTAL_RESERVES, &reserve.to_be_bytes());
    }

    pub fn set_total_debt(&mut self, debt: u128) {
        self.storage.set(KEY_TOTAL_DEBT, &debt.to_be_bytes());
    }

    pub fn contract_status(&self) -> ContractStatusLevel {
        self.as_readonly().contract_status()
    }

    pub fn set_contract_status(&mut self, status: ContractStatusLevel) {
        let status_u8 = status_level_to_u8(status);
        self.storage
            .set(KEY_CONTRACT_STATUS, &status_u8.to_be_bytes());
    }

    pub fn set_minters(&mut self, minters_to_set: Vec<HumanAddr>) -> StdResult<()> {
        set_bin_data(&mut self.storage, KEY_MINTERS, &minters_to_set)
    }

    pub fn add_minters(&mut self, minters_to_add: Vec<HumanAddr>) -> StdResult<()> {
        let mut minters = self.minters();
        minters.extend(minters_to_add);

        self.set_minters(minters)
    }

    pub fn remove_minters(&mut self, minters_to_remove: Vec<HumanAddr>) -> StdResult<()> {
        let mut minters = self.minters();

        for minter in minters_to_remove {
            minters.retain(|x| x != &minter);
        }

        self.set_minters(minters)
    }

    pub fn minters(&mut self) -> Vec<HumanAddr> {
        self.as_readonly().minters()
    }

    pub fn tx_count(&self) -> u64 {
        self.as_readonly().tx_count()
    }

    pub fn set_tx_count(&mut self, count: u64) -> StdResult<()> {
        set_bin_data(&mut self.storage, KEY_TX_COUNT, &count)
    }

    pub fn reserve_tokens(&mut self) -> Vec<Contract> {
        self.as_readonly().reserve_tokens()
    }
    pub fn get_reserve_token_info(&self, token : HumanAddr) -> StdResult<Contract>{
        self.as_readonly().get_reserve_token_info(token)
    }
    pub fn is_reserve_token(&mut self,token : HumanAddr) -> bool {
        self.as_readonly().is_reserve_token(token)
    }

    pub fn set_reserve_tokens(&mut self, reserve_tokens_to_set : Vec<Contract>) -> StdResult<()>{
        set_bin_data(&mut self.storage, KEY_RESERVE_TOKENS, &reserve_tokens_to_set)
    }

    pub fn add_reserve_tokens(&mut self, reserve_tokens_to_add : Vec<Contract>) -> StdResult<()>{
        let mut reserve_tokens = self.reserve_tokens();
        reserve_tokens.extend(reserve_tokens_to_add);

        self.set_reserve_tokens(reserve_tokens)
    }

    pub fn remove_reserve_token(&mut self, reserve_token_to_remove : Contract){
        let mut reserve_tokens = self.reserve_tokens();
        reserve_tokens.retain(|x| *x != reserve_token_to_remove)
    }

    pub fn liquidity_tokens(&mut self) -> Vec<Contract> {
        self.as_readonly().liquidity_tokens()
    }

    pub fn is_liquidity_token(&mut self,token : HumanAddr) -> bool {
        self.as_readonly().is_liquidity_token(token)
    }

    pub fn set_liquidity_tokens(&mut self, tokens_to_set : Vec<Contract>) -> StdResult<()>{
        set_bin_data(&mut self.storage, KEY_LIQUIDITY_TOKENS, &tokens_to_set)
    }

    pub fn add_liquidity_tokens(&mut self, tokens_to_add : Vec<Contract>) -> StdResult<()>{
        let mut liquidity_tokens = self.liquidity_tokens();
        liquidity_tokens.extend(tokens_to_add);

        self.set_liquidity_tokens(liquidity_tokens)
    }

    pub fn remove_liquidity_token(&mut self, liquidity_token_to_remove : Contract){
        let mut liquidity_tokens = self.liquidity_tokens();
        liquidity_tokens.retain(|x| *x != liquidity_token_to_remove)
    }

    pub fn bond_calculator(&self, token : HumanAddr) -> Contract{
        self.as_readonly().bond_calculator(token)
    }

    pub fn set_bond_calculator(&mut self, token : HumanAddr, calculator: Contract) -> StdResult<()>{
        bucket(&KEY_BOND_CALCULATOR, &mut self.storage).save(token.as_str().as_bytes(),&calculator)
    }

    pub fn value_of<Q: Querier> (&self, querier: &Q, token : Contract,amount:u128) -> StdResult<u128>{
        self.as_readonly().value_of(querier,token,amount)
    }
}

/// This struct refactors out the readonly methods that we need for `Config` and `ReadonlyConfig`
/// in a way that is generic over their mutability.
///
/// This was the only way to prevent code duplication of these methods because of the way
/// that `ReadonlyPrefixedStorage` and `PrefixedStorage` are implemented in `cosmwasm-std`
struct ReadonlyConfigImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyConfigImpl<'a, S> {
    fn constants(&self) -> StdResult<Constants> {
        let consts_bytes = self
            .0
            .get(KEY_CONSTANTS)
            .ok_or_else(|| StdError::generic_err("no constants stored in configuration"))?;
        bincode2::deserialize::<Constants>(&consts_bytes)
            .map_err(|e| StdError::serialize_err(type_name::<Constants>(), e))
    }

    pub fn managing_addresses(&self, role : ManagingRole) -> Vec<CanonicalAddr> { 
        let role_string : String = role.to_string() + MANAGING_ROLE_POSTFIX;
        get_bin_data(self.0,&role_string.into_bytes()).unwrap_or_default()
    }

    pub fn managing_position(&self,address: &CanonicalAddr,role : ManagingRole) -> Option<bool> {
        let role_string : String = role.to_string() + POSITION_POSTFIX;
        bucket_read(&role_string.into_bytes(), self.0).may_load(address.as_slice()).ok()?
    }

    pub fn has_managing_position(&self,address: &CanonicalAddr,role : ManagingRole) -> bool {
        if let Some(has_managing_position) = self.managing_position(address,role){
            if has_managing_position{
                return true;
            }
        }
        false
    }
    
    pub fn managing_queue(&self,address: &CanonicalAddr,role : ManagingRole) -> StdResult<u64> {
        let role_string : String = role.to_string() + QUEUE_POSTFIX;
        let managing_queue = bucket_read(&role_string.into_bytes(), self.0).may_load(address.as_slice());
        managing_queue.map(Option::unwrap_or_default)
    }

    fn total_reserves(&self) -> u128 {
        let reserves_bytes = self
            .0
            .get(KEY_TOTAL_RESERVES)
            .expect("no total reserves stored in config");
        // This unwrap is ok because we know we stored things correctly
        slice_to_u128(&reserves_bytes).unwrap_or_default()
    }

    fn excess_reserves<Q: Querier> (&self, querier : &Q)-> StdResult<u128> {

        let ohm_token_info = snip20::token_info_query(
            querier,
            RESPONSE_BLOCK_SIZE,
            self.constants()?.ohm.code_hash,
            self.constants()?.ohm.address
        )?; 
        let total_ohm_supply = ohm_token_info.total_supply.unwrap_or_default().u128();
        Ok(self.total_reserves() - (total_ohm_supply - self.total_debt()))
    }

    fn total_debt(&self) -> u128 {
        let debt_bytes = self
            .0
            .get(KEY_TOTAL_DEBT)
            .expect("no total debt stored in config");
        // This unwrap is ok because we know we stored things correctly
        slice_to_u128(&debt_bytes).unwrap_or_default()
    }

    fn contract_status(&self) -> ContractStatusLevel {
        let status_bytes = self
            .0
            .get(KEY_CONTRACT_STATUS)
            .expect("no contract status stored in config");

        // These unwraps are ok because we know we stored things correctly
        let status = slice_to_u8(&status_bytes).unwrap();
        u8_to_status_level(status).unwrap()
    }

    fn minters(&self) -> Vec<HumanAddr> {
        get_bin_data(self.0, KEY_MINTERS).unwrap_or_default()
    }

    pub fn tx_count(&self) -> u64 {
        get_bin_data(self.0, KEY_TX_COUNT).unwrap_or_default()
    }

    fn reserve_tokens(&self) -> Vec<Contract> {
        get_bin_data(self.0, KEY_RESERVE_TOKENS).unwrap_or_default()
    }

    pub fn get_reserve_token_info(&self, token : HumanAddr) -> StdResult<Contract>{
        self.reserve_tokens()
        .into_iter()
        .filter(|voc| voc.address == token.clone())
        .collect::<Vec<Contract>>().get(0).ok_or_else(
            || StdError::generic_err("No reserve_tokens with this name")
        ).map(|x| x.clone())
    }
    pub fn is_reserve_token(&self, token : HumanAddr) -> bool {
        self.get_reserve_token_info(token).is_ok()
    }

    fn liquidity_tokens(&self) -> Vec<Contract> {
        get_bin_data(self.0, KEY_LIQUIDITY_TOKENS).unwrap_or_default()
    }

    pub fn is_liquidity_token(&self, token : HumanAddr) -> bool {
        let liquidity_tokens_filtered : Vec<Contract> = 
        self.liquidity_tokens()
        .into_iter()
        .filter(|voc| voc.address == token.clone())
        .collect();
        !liquidity_tokens_filtered.is_empty()
    }

    pub fn bond_calculator(&self, token : HumanAddr) -> Contract{
        bucket_read(KEY_BOND_CALCULATOR, self.0).load(token.as_str().as_bytes()).unwrap()
    }

    pub fn value_of<Q: Querier> (&self, querier: &Q,token : Contract,amount:u128) -> StdResult<u128>{
        if self.is_reserve_token(token.address.clone()){
            let ohm_decimals = snip20::token_info_query(
                querier,
                RESPONSE_BLOCK_SIZE,
                self.constants()?.ohm.code_hash,
                self.constants()?.ohm.address,
            )?.decimals;
            let token_decimals = snip20::token_info_query(
                querier,
                RESPONSE_BLOCK_SIZE,
                token.code_hash,
                token.address,
            )?.decimals;
            Ok(amount*10_u128.pow(ohm_decimals.into())/10_u128.pow(token_decimals.into()))
        }else if self.is_liquidity_token(token.address.clone()){
            Ok(get_bond_valuation(token,amount))
        }else if token.address == self.constants()?.ohm.address{
            Ok(amount)
        }else{
            Err(StdError::generic_err("The token was not registered"))           
        }
    }
}

// Balances

pub struct ReadonlyBalances<'a, S: ReadonlyStorage> {
    storage: ReadonlyPrefixedStorage<'a, S>,
}

impl<'a, S: ReadonlyStorage> ReadonlyBalances<'a, S> {
    pub fn from_storage(storage: &'a S) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(PREFIX_BALANCES, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyBalancesImpl<ReadonlyPrefixedStorage<S>> {
        ReadonlyBalancesImpl(&self.storage)
    }

    pub fn account_amount(&self, account: &CanonicalAddr) -> u128 {
        self.as_readonly().account_amount(account)
    }
}

pub struct Balances<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> Balances<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(PREFIX_BALANCES, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyBalancesImpl<PrefixedStorage<S>> {
        ReadonlyBalancesImpl(&self.storage)
    }

    pub fn balance(&self, account: &CanonicalAddr) -> u128 {
        self.as_readonly().account_amount(account)
    }

    pub fn set_account_balance(&mut self, account: &CanonicalAddr, amount: u128) {
        self.storage.set(account.as_slice(), &amount.to_be_bytes())
    }
}

/// This struct refactors out the readonly methods that we need for `Balances` and `ReadonlyBalances`
/// in a way that is generic over their mutability.
///
/// This was the only way to prevent code duplication of these methods because of the way
/// that `ReadonlyPrefixedStorage` and `PrefixedStorage` are implemented in `cosmwasm-std`
struct ReadonlyBalancesImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyBalancesImpl<'a, S> {
    pub fn account_amount(&self, account: &CanonicalAddr) -> u128 {
        let account_bytes = account.as_slice();
        let result = self.0.get(account_bytes);
        match result {
            // This unwrap is ok because we know we stored things correctly
            Some(balance_bytes) => slice_to_u128(&balance_bytes).unwrap(),
            None => 0,
        }
    }
}

//Debtors
pub struct ReadonlyDebtors<'a, S: ReadonlyStorage> {
    storage: ReadonlyPrefixedStorage<'a, S>,
}

impl<'a, S: ReadonlyStorage> ReadonlyDebtors<'a, S> {
    pub fn from_storage(storage: &'a S) -> Self {
        Self {
            storage: ReadonlyPrefixedStorage::new(PREFIX_DEBTORS, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyDebtorsImpl<ReadonlyPrefixedStorage<S>> {
        ReadonlyDebtorsImpl(&self.storage)
    }

    pub fn debt(&self, account: &CanonicalAddr) -> u128 {
        self.as_readonly().debt_amount(account)
    }
}

pub struct Debtors<'a, S: Storage> {
    storage: PrefixedStorage<'a, S>,
}

impl<'a, S: Storage> Debtors<'a, S> {
    pub fn from_storage(storage: &'a mut S) -> Self {
        Self {
            storage: PrefixedStorage::new(PREFIX_DEBTORS, storage),
        }
    }

    fn as_readonly(&self) -> ReadonlyDebtorsImpl<PrefixedStorage<S>> {
        ReadonlyDebtorsImpl(&self.storage)
    }

    pub fn debt(&self, account: &CanonicalAddr) -> u128 {
        self.as_readonly().debt_amount(account)
    }

    pub fn set_account_debt(&mut self, account: &CanonicalAddr, amount: u128) {
        self.storage.set(account.as_slice(), &amount.to_be_bytes())
    }
}

/// This struct refactors out the readonly methods that we need for `Debtors` and `ReadonlyDebtors`
/// in a way that is generic over their mutability.
///
/// This was the only way to prevent code duplication of these methods because of the way
/// that `ReadonlyPrefixedStorage` and `PrefixedStorage` are implemented in `cosmwasm-std`
struct ReadonlyDebtorsImpl<'a, S: ReadonlyStorage>(&'a S);

impl<'a, S: ReadonlyStorage> ReadonlyDebtorsImpl<'a, S> {
    pub fn debt_amount(&self, account: &CanonicalAddr) -> u128 {
        let account_bytes = account.as_slice();
        let result = self.0.get(account_bytes);
        match result {
            // This unwrap is ok because we know we stored things correctly
            Some(debtor_bytes) => slice_to_u128(&debtor_bytes).unwrap(),
            None => 0,
        }
    }
}


// Allowances

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, Default, JsonSchema)]
pub struct Allowance {
    pub amount: u128,
    pub expiration: Option<u64>,
}

impl Allowance {
    pub fn is_expired_at(&self, block: &cosmwasm_std::BlockInfo) -> bool {
        match self.expiration {
            Some(time) => block.time >= time,
            None => false, // allowance has no expiration
        }
    }
}

pub fn read_allowance<S: Storage>(
    store: &S,
    owner: &CanonicalAddr,
    spender: &CanonicalAddr,
) -> StdResult<Allowance> {
    let owner_store =
        ReadonlyPrefixedStorage::multilevel(&[PREFIX_ALLOWANCES, owner.as_slice()], store);
    let owner_store = TypedStore::attach(&owner_store);
    let allowance = owner_store.may_load(spender.as_slice());
    allowance.map(Option::unwrap_or_default)
}

pub fn write_allowance<S: Storage>(
    store: &mut S,
    owner: &CanonicalAddr,
    spender: &CanonicalAddr,
    allowance: Allowance,
) -> StdResult<()> {
    let mut owner_store =
        PrefixedStorage::multilevel(&[PREFIX_ALLOWANCES, owner.as_slice()], store);
    let mut owner_store = TypedStoreMut::attach(&mut owner_store);

    owner_store.store(spender.as_slice(), &allowance)
}

// Viewing Keys

pub fn write_viewing_key<S: Storage>(store: &mut S, owner: &CanonicalAddr, key: &ViewingKey) {
    let mut balance_store = PrefixedStorage::new(PREFIX_VIEW_KEY, store);
    balance_store.set(owner.as_slice(), &key.to_hashed());
}

pub fn read_viewing_key<S: Storage>(store: &S, owner: &CanonicalAddr) -> Option<Vec<u8>> {
    let balance_store = ReadonlyPrefixedStorage::new(PREFIX_VIEW_KEY, store);
    balance_store.get(owner.as_slice())
}

// Receiver Interface

pub fn get_receiver_hash<S: ReadonlyStorage>(
    store: &S,
    account: &HumanAddr,
) -> Option<StdResult<String>> {
    let store = ReadonlyPrefixedStorage::new(PREFIX_RECEIVERS, store);
    store.get(account.as_str().as_bytes()).map(|data| {
        String::from_utf8(data)
            .map_err(|_err| StdError::invalid_utf8("stored code hash was not a valid String"))
    })
}

pub fn set_receiver_hash<S: Storage>(store: &mut S, account: &HumanAddr, code_hash: String) {
    let mut store = PrefixedStorage::new(PREFIX_RECEIVERS, store);
    store.set(account.as_str().as_bytes(), code_hash.as_bytes());
}

// Helpers

/// Converts 16 bytes value into u128
/// Errors if data found that is not 16 bytes
fn slice_to_u128(data: &[u8]) -> StdResult<u128> {
    match <[u8; 16]>::try_from(data) {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => Err(StdError::generic_err(
            "Corrupted data found. 16 byte expected.",
        )),
    }
}

/// Converts 1 byte value into u8
/// Errors if data found that is not 1 byte
fn slice_to_u8(data: &[u8]) -> StdResult<u8> {
    if data.len() == 1 {
        Ok(data[0])
    } else {
        Err(StdError::generic_err(
            "Corrupted data found. 1 byte expected.",
        ))
    }
}
