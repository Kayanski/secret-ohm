const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, CosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const fs = require("fs");

require('dotenv').config();

const COMMON_VIEWING_KEY = "ALL_ORGANISATION_INFO_SHOULD_BE_PUBLIC";

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
async function getEpoch(client,contracts) {
  let response = await client.queryContractSmart(contracts["staking"][1].contractAddress, { "epoch": {}});
  return response;
}

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
async function getBondPriceInUsd(client, bondAddress, principleAddress){
  let token_info = await getTokenInfo(client,principleAddress)
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
  return response.bond_price.price/Math.pow(10,sUSTdecimals);
}

async function getBondROI(client, bondAddress, principleAddress, contracts){
  let bondPrice = await getBondPriceInUsd(client, bondAddress,principleAddress);
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




  
const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await get_address();

  let rawdata = fs.readFileSync('contract_data.json');
  let contracts = JSON.parse(rawdata);
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
  
  //const [apiKey,balance] = await getBalance(client, contracts["OHM"][1].contractAddress, accAddress );
  //console.log(apiKey,balance);

  
  array = [123, 34, 99, 105, 114, 99, 117, 108, 97, 116, 105, 110, 103, 95, 115, 117, 112, 112, 108, 121, 34, 58, 123, 34, 99, 105, 114, 99, 117, 108, 97, 116, 105, 110, 103, 95, 115, 117, 112, 112, 108, 121, 34, 58, 34, 48, 34, 125, 125];
  console.log(new Buffer.from(array).toString());

  let msg = "eyJjaGFuZ2VzX2luX3JlYmFzZSI6eyJjaXJjdWxhdGluZ19zdXBwbHkiOiIwIiwidG90YWxfc3VwcGx5X2JlZm9yZSI6IjUwMDAwMDAwMDAwMDAwMDAiLCJ0b3RhbF9zdXBwbHlfYWZ0ZXIiOiI1MDAxODAwMDAwMDAwMDAwIn19";
  console.log(base64ToJson(msg));
 
  response = await getBalance(client, OHMcontract.contractAddress, Stakingcontract.contractAddress,COMMON_VIEWING_KEY);
  console.log(response);

  let ohm_amount = "100000000000";
                    100000000000
  let query_msg = {
    gons_for_balance:{amount:ohm_amount}
  };
  response = await client.queryContractSmart(sOHMcontract.contractAddress,query_msg);
  console.log(response);

  let gons = "157898303505431176295316359127443402960442520577888964419453600000000000";
  query_msg = {
    balance_for_gons:{gons:gons}
  };
  response = await client.queryContractSmart(sOHMcontract.contractAddress,query_msg);
  console.log(response);


  query_msg = {
    token_info:{}
  };
  let token_info = await client.queryContractSmart(OHMcontract.contractAddress,query_msg);
  console.log("Total supply :",token_info.token_info.total_supply);

  query_msg = {
    circulating_supply:{}
  };
  token_info = await client.queryContractSmart(sOHMcontract.contractAddress,query_msg);
  console.log("Circulating supply :",token_info.circulating_supply.circulating_supply);

  
  response = await client.queryContractSmart(contracts["sOHM"][1].contractAddress, { "circulating_supply": {}});
  console.log("After Rebase : ", response);
  

  let stakingBalance = await getBalance(client, 
    contracts["OHM"][1].contractAddress,
    contracts["staking"][1].contractAddress,
    COMMON_VIEWING_KEY);  
  console.log("Staking balance", stakingBalance[1].balance.amount);

  let rebaseROI = await getRebaseAmount(client,contracts);
  console.log("Rebase amount : ", rebaseROI);

  let epoch = await getEpoch(client,contracts);
  console.log("Epoch : ", epoch);

  let AddressBalance = await getBalance(client, 
    contracts["sOHM"][1].contractAddress,
    accAddress);  
  console.log("Warmup balance : ", AddressBalance);

}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
