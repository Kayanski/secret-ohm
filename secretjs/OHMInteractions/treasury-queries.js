const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, CosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const fs = require("fs");

require('dotenv').config();
let contracts = require("../contract_data.json");

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

async function createViewingKey(client, contractAddress, address){
  const entropy = "Another really random thing";
  let handleMsg = { create_viewing_key: {entropy: entropy} };
  response = await client.execute(contractAddress, handleMsg);

  // Convert the UTF8 bytes to String, before parsing the JSON for the api key.
  const apiKey = JSON.parse(new TextDecoder().decode(response.data)).create_viewing_key.key;
  return apiKey;
}

async function getBalance(client, contractName, address, apiKey = undefined){
  contractAddress = getContractFromName(contractName).contractAddress;
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


function getContractFromName(contractName) {
  if (contractName in contracts) {
    return contracts[contractName][1];
  } else {
    console.log("Contract not found : ", contractName, contracts);
  }
}

async function getManagingAddress(client, role){
  contractAddress = getContractFromName("treasury").contractAddress;

  const query = { 
      managing_addresses: {
          role:role
      }
  };
  let response = await client.queryContractSmart(contractAddress, query);
  return response;
}

  
const main = async () => {
  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await get_address();

  let response;

  //response = await getManagingAddress(client, "ReserveManager");
  //console.log(response);

  response = await getBalance(client, "sUST",accAddress);
  console.log(response);
  
}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
