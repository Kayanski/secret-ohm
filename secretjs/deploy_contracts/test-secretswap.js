const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey,makeSignBytes
} = require("secretjs");

const uuid = require("uuid");

const fs = require("fs");

var arguments = process.argv;

if(arguments[2] == "staging"){
  require('dotenv').config({ path: '.env.testnet' });
}else{
  require('dotenv').config();;
}

function jsonToBase64(json){
  return Buffer.from(JSON.stringify(json)).toString('base64')
}
function base64ToJson(base64String){
  return JSON.parse(Buffer.from(base64String,'base64').toString('utf8'));
}

let contracts = require("../contract_data.json");
let liquidity_contracts = require("../liquidity_contracts.json");

//DAO Address
const DAOAddress = "secret1878ru0hfgdk0atdvj9kvcl0gfzkfjr25m4pd94";
const DAOMnemonicSecretKey = "quality isolate target melody flame adjust actress funny wear art sister capital banana orient duty settle until wire profit evidence violin side muscle obey";


// Initial staking index
const initialIndex = '7675210820';

// First block epoch occurs
const firstEpochBlock = 1;

// What epoch will be first epoch
const firstEpochNumber = 338;

// How many blocks are in each epoch
// 4800 ~ 8hs
//const epochLengthInBlocks = 4800;
const epochLengthInBlocks = 10;

// Initial reward rate for epoch
const initialRewardRate = '3000';

// Initial mint for Frax and DAI (10,000,000)
const initialMint = '10000000000000000000000000';

// DAI bond BCV
const daiBondBCV = '369';

// Frax bond BCV
const fraxBondBCV = '690';

// Bond vesting length in blocks. 33110 ~ 5 days
const bondVestingLength = 72000;

// Min bond price
const minBondPrice = '500';

// Max bond payout
const maxBondPayout = '50'

// DAO fee for bond
const bondFee = '10000';

// Max debt bond can take on
const maxBondDebt = '1000000000000000';

// Initial Bond debt
const initialBondDebt = '15000000000000000'

const initialUSTLiquidity = "1000"

const initialOHMLiquidity = "1000000"


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

async function get_testnet_default_address(){
  return get_address(process.env.MNEMONIC_TESTNET);
}

async function get_address(mnemonic = process.env.MNEMONIC){
  
  // A pen is the most basic tool you can think of for signing.
  // This wraps a single keypair and allows for signing.
  const signingPen = await Secp256k1Pen.fromMnemonic(mnemonic);

  // Get the public key
  const pubkey = encodeSecp256k1Pubkey(signingPen.pubkey);

  // get the wallet address
  const accAddress = pubkeyToAddress(pubkey, 'secret');

  return accAddress;
}

async function get_client(mnemonic = process.env.MNEMONIC){
  const httpUrl = process.env.SECRET_REST_URL;

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

async function upload_code(contract_file,client,contractName="Default Contract"){

  // Upload the wasm of a simple contract
  const wasm = fs.readFileSync(contract_file);
  console.log('Uploading contract')
  const uploadReceipt = await client.upload(wasm, {});

  // Get the code ID from the receipt
  const codeId = uploadReceipt.codeId;
  console.log('codeId: ', codeId);

  const contractCodeHash = await client.restClient.getCodeHashByCodeId(codeId);

  return [codeId,contractCodeHash];

}


async function instantiate_contract(client, codeId, initMsg, contractName){
  const contract = await client.instantiate(codeId, initMsg, contractName + Math.ceil(Math.random()*10000));
  return contract;
}


async function upload_contract(contract_file, client, initMsg, contractName = "Default Contract"){

  let [codeId,contractCodeHash] = await upload_code(contract_file,client,contractName);
  // contract hash, useful for contract composition
  contract = await instantiate_contract(client, codeId, initMsg, contractName);
  // Create an instance of the Counter contract, providing a starting count
  return [contractCodeHash,contract]

}

async function pool(client){

  let queryMsg = {
    pool:{}
  }

  response = await client.queryContractSmart(liquidity_contracts["pair"][1],queryMsg);
  return response;

}

async function pair(client){

  let queryMsg = {
    pair:{}
  }

  response = await client.queryContractSmart(liquidity_contracts["pair"][1],queryMsg);
  return response;

}


function getContractFromName(contractName) {
  if (contractName in contracts) {
    return contracts[contractName][1];
  } else {
    console.log("Contract not found : ", contractName, contracts);
  }
}

function getCodeHashFromName(contractName) {
  if (contractName in contracts) {
    return contracts[contractName][0];
  } else {
    console.log("Contract not found : ", contractName, contracts);
  }
}

async function getKValue(client, pair){
   let queryMsg = {
      get_k_value:{
        pair: pair
      }
    }
    let bond_calculator = getContractFromName("bond_calculator").contractAddress;
    response = await client.queryContractSmart(bond_calculator,queryMsg);
    return response;
}

async function getTotalValue(client, pair){
   let queryMsg = {
      get_total_value:{
        pair: pair
      }
    }
    let bond_calculator = getContractFromName("bond_calculator").contractAddress;
    response = await client.queryContractSmart(bond_calculator,queryMsg);
    return response;
}

async function valuation(client, pair, amount){
   let queryMsg = {
      valuation:{
        pair: pair,
        amount: amount
      }
    }
    let bond_calculator = getContractFromName("bond_calculator").contractAddress;
    response = await client.queryContractSmart(bond_calculator,queryMsg);
    return response;
}

async function markdown(client, pair){
   let queryMsg = {
      markdown:{
        pair: pair
      }
    }
    let bond_calculator = getContractFromName("bond_calculator").contractAddress;
    response = await client.queryContractSmart(bond_calculator,queryMsg);
    return response;
}

async function getTokenInfo(client,address){
  let response = await client.queryContractSmart(address, { "token_info": {}});
  return response.token_info;
}

async function getContractInfo(client,address){
  let response = await client.queryContractSmart(address, { "contract_info": {}});
  return response.contract_info;
}


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
  return response.bond_price.price/100;
}

async function getCurrentBlockHeight(client){
  const blocksLatest = await client.restClient.blocksLatest();
  return parseInt(blocksLatest.block.last_commit.height);
}


async function giveRole(client, address, role){

  let treasurycontract = getContractFromName("treasury");
   // queue and toggle reward manager
  handleMsg = {
      queue: {
        "address":address,
        "role" : role
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

  handleMsg = {
      toggle_queue: {
        "address":address,
        "role" : role
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

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

// User Queries
async function createViewingKey(client, contractAddress, address){
  const entropy = "Another really random thing";
  let handleMsg = { create_viewing_key: {entropy: entropy} };
  response = await client.execute(contractAddress, handleMsg);

  // Convert the UTF8 bytes to String, before parsing the JSON for the api key.
  const apiKey = JSON.parse(new TextDecoder().decode(response.data)).create_viewing_key.key;
  return apiKey;
}

async function increase_allowance(client, token, spender, amount){
  token = getContractFromName(token).contractAddress;
  let handleMsg = {
    increase_allowance:{
      spender: spender,
      amount: amount,
    }
  }
  let response = await client.execute(token,handleMsg);
  return response
}
const main = async () => {

  client = await get_client();

  const accAddress = await get_address();

  let test1 = "eyJkZXBvc2l0Ijp7Im1heF9wcmljZSI6IjYwMDAwIiwiZGVwb3NpdG9yIjoic2VjcmV0MXM1bDBkNzdlN2cwN21wZnM5cDJzeGQyZXgzMDlqaHZqdGVldm15In19";
  console.log(base64ToJson(test1));

  test1 = [15, 0, 0, 0, 0, 0, 0, 0, 115, 79, 72, 77, 45, 85, 83, 84, 76, 80, 32, 98, 111, 110, 100, 4, 0, 0, 0, 0, 0, 0, 0, 66, 83, 76, 80, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 115, 101, 100, 116, 55, 97, 110, 103, 97, 57, 53, 57, 102, 116, 122, 109, 121, 122, 118, 103, 50, 55, 122, 100, 101, 122, 101, 106, 99, 116, 117, 102, 53, 97, 109, 112, 112, 108, 64, 0, 0, 0, 0, 0, 0, 0, 51, 99, 55, 53, 49, 56, 102, 100, 98, 100, 49, 55, 55, 53, 101, 100, 48, 101, 50, 48, 101, 100, 100, 102, 51, 50, 48, 57, 55, 55, 50, 100, 99, 97, 52, 53, 54, 102, 56, 52, 100, 51, 98, 100, 53, 102, 97, 53, 100, 97, 102, 99, 53, 51, 49, 102, 53, 99, 56, 56, 102, 55, 50, 48, 9, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 55, 122, 108, 108, 51, 52, 121, 115, 51, 102, 51, 108, 52, 112, 102, 55, 108, 57, 117, 113, 109, 106, 99, 97, 100, 55, 116, 119, 51, 119, 99, 113, 106, 115, 115, 106, 117, 51, 64, 0, 0, 0, 0, 0, 0, 0, 99, 53, 54, 50, 98, 49, 102, 51, 48, 52, 53, 98, 98, 52, 53, 102, 52, 98, 101, 97, 55, 52, 98, 49, 56, 55, 53, 55, 54, 55, 57, 102, 97, 101, 52, 55, 53, 52, 98, 98, 57, 99, 49, 102, 98, 101, 55, 100, 51, 53, 97, 101, 52, 53, 52, 54, 100, 48, 55, 55, 48, 51, 99, 97, 1, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 99, 51, 108, 107, 107, 107, 121, 99, 53, 114, 121, 113, 104, 97, 52, 113, 50, 113, 120, 97, 57, 109, 48, 114, 102, 100, 52, 110, 119, 115, 104, 114, 56, 121, 116, 55, 103, 48, 64, 0, 0, 0, 0, 0, 0, 0, 98, 48, 102, 102, 48, 98, 100, 57, 52, 49, 101, 53, 101, 98, 99, 98, 56, 101, 48, 98, 102, 48, 99, 101, 55, 53, 53, 55, 49, 97, 49, 98, 57, 51, 99, 98, 100, 48, 50, 56, 100, 56, 101, 50, 57, 53, 53, 55, 51, 52, 53, 50, 51, 102, 51, 99, 52, 48, 57, 54, 102, 53, 52, 52, 6, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 50, 52, 103, 97, 104, 119, 118, 56, 53, 112, 55, 116, 56, 116, 114, 115, 114, 52, 110, 51, 100, 107, 113, 113, 108, 56, 109, 55, 57, 107, 52, 112, 52, 55, 110, 104, 118, 97, 64, 0, 0, 0, 0, 0, 0, 0, 50, 98, 102, 100, 50, 49, 55, 48, 56, 99, 54, 102, 54, 101, 53, 55, 56, 100, 50, 50, 102, 98, 49, 57, 98, 52, 49, 56, 50, 53, 51, 101, 55, 100, 102, 50, 97, 99, 100, 52, 56, 56, 49, 52, 54, 51, 51, 56, 51, 99, 51, 52, 54, 99, 49, 49, 53, 50, 55, 50, 54, 102, 100, 98, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 56, 55, 56, 114, 117, 48, 104, 102, 103, 100, 107, 48, 97, 116, 100, 118, 106, 57, 107, 118, 99, 108, 48, 103, 102, 122, 107, 102, 106, 114, 50, 53, 109, 52, 112, 100, 57, 52, 0, 0, 0, 0, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 115, 53, 108, 48, 100, 55, 55, 101, 55, 103, 48, 55, 109, 112, 102, 115, 57, 112, 50, 115, 120, 100, 50, 101, 120, 51, 48, 57, 106, 104, 118, 106, 116, 101, 101, 118, 109, 121, 32, 0, 0, 0, 0, 0, 0, 0, 240, 43, 140, 131, 106, 37, 185, 139, 114, 126, 180, 201, 158, 112, 138, 77, 248, 114, 55, 100, 158, 240, 166, 228, 211, 208, 66, 71, 92, 85, 238, 223, 45, 0, 0, 0, 0, 0, 0, 0, 115, 101, 99, 114, 101, 116, 49, 99, 104, 54, 107, 55, 106, 51, 107, 112, 117, 119, 118, 97, 55, 53, 51, 53, 107, 99, 52, 54, 118, 120, 54, 101, 108, 101, 106, 108, 113, 54, 99, 52, 104, 108, 56, 119, 50];
  console.log(Buffer.from(test1).toString("utf8"));

  let response;


  response = await getCurrentBlockHeight(client);
  console.log(response);
  console.log("Nb current epoch", response/4800);


  // We upload the LP bond
  let pairContractAddr = contracts["OHM-UST-LP"][1]["pair"][1].contractAddress;
  //await increase_allowance(client, "sUST", pairContractAddr, "4000");
  handleMsg = 
  {
    "provide_liquidity": {
      "assets": [
        {
          "info": {
            "token": {
            "contract_addr": getContractFromName("sUST").contractAddress,
            "token_code_hash": getCodeHashFromName("sUST"),
            "viewing_key": "" // ignored, can be whatever
          }
          },
          "amount": "4000"
        },
        {
          "info": {
            "token": {
            "contract_addr": getContractFromName("OHM").contractAddress,
            "token_code_hash": getCodeHashFromName("OHM"),
            "viewing_key": "" // ignored, can be whatever
          }
          },
          "amount": "0"
        }
      ]
    }
  }
  //await client.execute(pairContractAddr,handleMsg);




  //END

  // We test some bond queries

  response = await getBondPrice(client,getContractFromName("sOHM-USTLP bond").contractAddress);
  console.log("Bond Price", response);
  response = await getBondPriceInUsd(client, getContractFromName("sOHM-USTLP bond").contractAddress, liquidity_contracts["token"][1]);
  console.log("Bond Price in USD", response);
  response = await getContractInfo(client, getContractFromName("sOHM-USTLP bond").contractAddress);
  console.log("General Bond Info", response);
  response = await getBondStatus(client, getContractFromName("sOHM-USTLP bond").contractAddress, accAddress);
  console.log("Bond Info for the address", response);


  /*
  response = await getTokenInfo(client,liquidity_contracts["token"][1])
  console.log(response);

  response = await pool(client);
  console.log(response);
  console.log(response.assets[0])
  console.log(response.assets[1])

  response = await pair(client);
  console.log(response);
  console.log(response.asset_infos);


  */

  let pair_contract = {
    "address":liquidity_contracts["pair"][1],
    "code_hash": liquidity_contracts["pair"][0]
  };
  response = await getKValue(client, pair_contract);
  console.log(response);
  response = await getTotalValue(client, pair_contract);
  console.log(response);
  response = await valuation(client, pair_contract, "31622");
  console.log(response);
  response = await markdown(client, pair_contract);
  console.log("Markdown", response);
 
 // We need to test the bond calculator entry points

}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
