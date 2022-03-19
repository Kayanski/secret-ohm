const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, CosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const fs = require("fs");

require('dotenv').config();

const COMMON_VIEWING_KEY = "ALL_ORGANISATION_INFO_SHOULD_BE_PUBLIC";

let contracts = require("../contract_data.json");
//Fees
const customFees = {
  upload: {
      amount: [{ amount: "4000000", denom: "uscrt" }],
      gas: "4000000",
  },
  init: {
      amount: [{ amount: "500000", denom: "uscrt" }],
      gas: "500000",
  },
  exec: {
      amount: [{ amount: "500000", denom: "uscrt" }],
      gas: "500000",
  },
  send: {
      amount: [{ amount: "80000", denom: "uscrt" }],
      gas: "80000",
  },
}

function jsonToBase64(json){
  return Buffer.from(JSON.stringify(json)).toString('base64')
}
function base64ToJson(base64String){
  return JSON.parse(Buffer.from(base64String,'base64').toString('utf8'));
}
function base64ToText(base64String){
  return Buffer.from(base64String,'base64').toString('utf8');
}


async function get_address(){
  // Use key created in tutorial #2
  const mnemonic = process.env.MNEMONIC;

  // A pen is the most basic tool you can think of for signing.
  // This wraps a single keypair and allows for signing.
  const signingPen = await Secp256k1Pen.fromMnemonic(mnemonic);

  // Get the public key
  const pubkey = encodeSecp256k1Pubkey(signingPen.pubkey);

  // get the wallet address
  const accAddress = pubkeyToAddress(pubkey, 'secret');
  return accAddress;

}

async function get_client(){
  const httpUrl = process.env.SECRET_REST_URL;

  // Use key created in tutorial #2
  const mnemonic = process.env.MNEMONIC;

  // A pen is the most basic tool you can think of for signing.
  // This wraps a single keypair and allows for signing.
  const signingPen = await Secp256k1Pen.fromMnemonic(mnemonic);

  // Get the public key
  const pubkey = encodeSecp256k1Pubkey(signingPen.pubkey);

  // get the wallet address
  const accAddress = pubkeyToAddress(pubkey, 'secret');

  const txEncryptionSeed = EnigmaUtils.GenerateNewSeed();
  
  const client = new SigningCosmWasmClient(
      httpUrl,
      accAddress,
      (signBytes) => signingPen.sign(signBytes),
      txEncryptionSeed, customFees
  );
  return client
}

// General Queries

async function getTokenInfo(client,address){
  let response = await client.queryContractSmart(address, { "token_info": {}});
  return response.token_info;
}

async function getTotalSupply(client,address){
  return (await getTokenInfo(client,address)).total_supply;
}

const getTotalValueDeposited = async(client,tokenContract) => {
  let response = await client.queryContractSmart(tokenContract.contractAddress, { "contract_balance": {}});
  return response.contract_balance.amount;
}

async function getContractInfo(client, address){
  let response = await client.queryContractSmart(address, { "contract_info": {}});
  return response.contract_info;
}

async function getTreasuryExcessReserves(client, contracts){
  let contract_info = await getContractInfo(client,contracts["treasury"][1].contractAddress)
  return contract_info.total_reserves;
}

async function getTotalTreasuryReserves(client, contracts){
  let contract_info = await getContractInfo(client,contracts["treasury"][1].contractAddress)
  return contract_info.total_reserves;
}

async function getTotalTreasuryDebt(client, contracts){
  let contract_info = await getContractInfo(client,contracts["treasury"][1].contractAddress)
  return contract_info.total_debt;
}

async function getTotalTreasuryDebt(client, contracts){
  let total_reserves = await getTotalTreasuryReserves(client, contracts);
  let OHM_total_supply = (await getTokenInfo(client,contracts["OHM"][1].contractAddress)).total_supply;
  return (total_reserves)/OHM_total_supply;
}

async function getTokenPriceInUsd(client, contractAddress, contracts){
  return await getTokenPrice(client, contractAddress, contracts["sUST"][1].contractAddress, contracts);
}

async function getTokenPrice(client, contractAddress, baseContractAddress,contracts){
  //let response = await client.queryContractSmart(contracts["secret-swap"], { "price": {contractAddress, baseContractAddress}});
  //return response.price;
  return 583;
}

//Stake
async function getStakedCirculatingSupply(client,contracts) {
  let response = await client.queryContractSmart(contracts["sOHM"][1].contractAddress, { "circulating_supply": {}});
  return response;
}

async function getRebaseAmount(client,contracts) {
  let epoch = await getEpoch(client,contracts);
  let circulating_supply = await getStakedCirculatingSupply(client,contracts);
  return epoch.distribute/circulating_supply.circulating_supply.circulating_supply;
}

const getRebaseHistory = async(client,tokenContract, page_size, page=null) => {
  queryMsg = {
    rebase_history:{page_size:page_size, page:page},
  };
  let response = await client.queryContractSmart(tokenContract.contractAddress,queryMsg);
  return response;
}

const getCurrentIndex = async(client,tokenContract) => {
  const index = await getIndex(client, tokenContract);
  const rebase_history = await getRebaseHistory(client, tokenContract, 0);
  last_rebase_id = rebase_history.rebase_history.total;
  let response = (await getRebaseHistory(client, tokenContract, 1, last_rebase_id-1))
  console.log(response);
  let first_rebase = response.rebase_history.rebases[0];
  console.log(first_rebase);
  if (first_rebase.id == 1){
    return parseInt(index)/parseInt(first_rebase.index);
  }
  return null;
}


const getIndex = async(client,tokenContract) => {
  let response = await client.queryContractSmart(tokenContract.contractAddress, { "index": {}});
  return response.index.index;
}

const nextRewardAmount = async(client, contracts, address, apiKey = undefined) => {
  let stakedBalance = await getStakedAmount(client, contracts, address, apiKey);
  let rebaseAmount = await getRebaseAmount(client,contracts);
  return stakedBalance*rebaseAmount;
}

const getStakedAmount = async(client, contracts, address, apiKey = undefined) => {
  let stakedBalance = await getBalance(client, contracts["sOHM"][1].contractAddress, address, apiKey);
  stakedBalance = stakedBalance[1].balance.amount
  let token_info = await getTokenInfo(client,contracts["sOHM"][1].contractAddress)
  return stakedBalance/Math.pow(10,token_info.decimals);
}


// User Queries
async function createViewingKey(client, contractAddress, address){
  const entropy = "Another really random thing";
  let handleMsg = { create_viewing_key: {entropy: entropy} };
  response = await client.execute(contractAddress, handleMsg);

  // Convert the UTF8 bytes to String, before parsing the JSON for the api key.
  const apiKey = JSON.parse(new TextDecoder().decode(response.data)).create_viewing_key.key;
  return apiKey;
}

async function getBalance(client, contractAddress, address, apiKey = undefined){
  // Query balance with the api key
  if(apiKey == undefined){
    apiKey = await createViewingKey(client, contractAddress, address)
  }
  const balanceQuery = { 
      balance: {
          key: apiKey, 
          address: address
      }
  };
  let balance = await client.queryContractSmart(contractAddress, balanceQuery);
  return [apiKey,balance];
}

async function getCurrentBlockHeight(client){
  const blocksLatest = await client.restClient.blocksLatest();
  return parseInt(blocksLatest.block.last_commit.height);
}

async function getBondStatus(client, bondAddress, address, apiKey = undefined){
  if(apiKey == undefined){
    apiKey = await createViewingKey(client, bondAddress, address)
  }
  const bondQuery = { 
      bond_info: {
          key: apiKey, 
          address: address
      }
  };
  let bond_info = await client.queryContractSmart(bondAddress, bondQuery);
  return [apiKey,bond_info.bond];
}

// Bond queries
async function getBondPriceInUsd(client, bondAddress){
  let ust = getContractFromName("sUST");
  let token_info = await getTokenInfo(client,ust.contractAddress)
  let decimals = token_info.decimals;

  block_height = await getCurrentBlockHeight(client);
  const query = { 
      bond_price_in_usd: {
          block_height:block_height+1
      }
  };
  let response = await client.queryContractSmart(bondAddress, query);
  return response.bond_price_in_usd.price/Math.pow(10,decimals);
}

async function getBondPrice(client, bondAddress){
  block_height = await getCurrentBlockHeight(client);
  const query = { 
      bond_price: {
          block_height:block_height+1
      }
  };
  let response = await client.queryContractSmart(bondAddress, query);
  return response.bond_price.price/100;
}

async function getBondROI(client, bondAddress, principleAddress, contracts){
  let bondPrice = await getBondPriceInUsd(client, bondAddress);
  let tokenPrice = await getTokenPriceInUsd(client, contracts["OHM"][1].contractAddress, contracts);
  return tokenPrice/bondPrice - 1;
}

async function getBondPurchased(client, bondAddress, contracts){
  const query = { 
    total_bond_deposited:{
      token: bondAddress,
    }
  };
  let response = await client.queryContractSmart(contracts["treasury"][1].contractAddress, query);
  return response.total_bond_deposited.amount;
}

async function getDebtRatio(client, bondAddress, contracts){
  block_height = await getCurrentBlockHeight(client);
  const query = { 
      standardized_debt_ratio: { 
        block_height: block_height+1
      }
  };
  let response = await client.queryContractSmart(bondAddress, query);
  return response.standardized_debt_ratio.ratio;
}

async function getBondTerms(client, bondAddress){
  const query = { 
      bond_terms: { }
  };
  let response = await client.queryContractSmart(bondAddress, query);
  return response.terms;
}

async function maxYouCanBuy(client, bondAddress){
  const query = { 
      max_payout: { }
  };
  let response = await client.queryContractSmart(bondAddress, query);
  return response.terms;
}

function getContractFromName(contractName) {
  if (contractName in contracts) {
    return contracts[contractName][1];
  } else {
    console.log("Contract not found : ", contractName, contracts);
  }
}



  
const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await get_address();

  let response;
  
  const [sSCRTcontractCodeHash, sSCRTcontract] = await contracts["sSCRT"];
  const [sUSTcontractCodeHash, sUSTcontract] = await contracts["sUST"];
  const [OHMcontractCodeHash, OHMcontract] = await contracts["OHM"];
  const [sOHMcontractCodeHash, sOHMcontract] = await contracts["sOHM"];
  const [treasurycontractCodeHash, treasurycontract]  = await contracts["treasury"];
  const [CalculatorcontractCodeHash, Calculatorcontract] = await contracts["bond_calculator"];
  const [DistributorcontractCodeHash, Distributorcontract] = await contracts["staking_distributor"];
  const [StakingcontractCodeHash, Stakingcontract] = await contracts["staking"];
  const [StakingWarmupContractCodeHash, StakingWarmupContract] = await contracts["staking-warmup"];
  const [sUSTBondContractCodeHash, sUSTBondContract] = contracts["sUST-bond"];
  const [sSCRTBondContractCodeHash, sSCRTBondContract] = contracts["sSCRT-bond"];
  const [LPBondContractCodeHash, LPBondContract] = contracts["OHM-UST-LP-bond"];
  const [LPContractCodeHash, LPContract] = contracts["OHM-UST-LP"];
  
  //const [apiKey,balance] = await getBalance(client, contracts["OHM"][1].contractAddress, accAddress );
  //console.log(apiKey,balance);

  
  array = [45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 107, 119, 122, 102, 109, 115, 51, 114, 104, 104, 112, 109, 52, 118, 121, 97, 122, 118, 102, 112, 121, 108, 113, 110, 118, 119, 100, 113, 108, 52, 106, 116, 108, 53, 54, 102, 55, 115, 64, 0, 0, 0, 0, 0, 0, 0, 51, 99, 55, 53, 49, 56, 102, 100, 98, 100, 49, 55, 55, 53, 101, 100, 48, 101, 50, 48, 101, 100, 100, 102, 51, 50, 48, 57, 55, 55, 50, 100, 99, 97, 52, 53, 54, 102, 56, 52, 100, 51, 98, 100, 53, 102, 97, 53, 100, 97, 102, 99, 53, 51, 49, 102, 53, 99, 56, 56, 102, 55, 50, 48, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 110, 52, 48, 116, 116, 100, 100, 103, 107, 54, 115, 108, 120, 56, 54, 117, 112, 112, 121, 122, 53, 57, 119, 100, 110, 112, 52, 118, 48, 50, 54, 121, 101, 51, 120, 107, 48, 53, 64, 0, 0, 0, 0, 0, 0, 0, 53, 100, 99, 54, 98, 48, 53, 57, 97, 100, 55, 98, 48, 55, 52, 55, 98, 97, 102, 100, 51, 53, 55, 50, 55, 99, 50, 48, 98, 100, 50, 53, 56, 54, 101, 52, 49, 53, 102, 54, 97, 53, 100, 48, 98, 99, 51, 51, 54, 56, 48, 99, 102, 102, 48, 56, 57, 55, 51, 54, 101, 101, 57, 54];
  console.log(new Buffer.from(array).toString());
  
  let msg = "eyJ0cmFuc2ZlciI6eyJzdGF0dXMiOiJzdWNjZXNzIn19ICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIA==";
  console.log(base64ToText(msg));
  
  response = await getContractInfo(client, getContractFromName("treasury").contractAddress);
  console.log(response);
  response = await getBondPrice(client, LPBondContract.contractAddress);
  console.log("Bond Price", response);
  response = await getBondPriceInUsd(client, LPBondContract.contractAddress);
  console.log("Bond Price in USD", response);
  response = await getContractInfo(client, sUSTBondContract.contractAddress);
  console.log("General Bond Info", response);
  response = await getBondStatus(client, LPBondContract.contractAddress, accAddress);
  console.log("Bond Info for the address", response);

  response = await getBalance(client, LPBondContract.contractAddress, accAddress,response[0]);
  console.log("Bond Balance for the address", response);
  
  response = await getBalance(client, LPContract.contractAddress, accAddress);
  console.log("LiquidityToken", response);

  

  //response = await getBalance(client, OHMcontract.contractAddress, accAddress);
  //console.log("OHM Balance", response);

  response = await getBalance(client, sUSTcontract.contractAddress, accAddress)
  console.log("QUIIIID", response);

  response = await getTokenInfo(client, OHMcontract.contractAddress );
  console.log("OHM token info", response)
}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
