const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const fs = require("fs");

const { fromBase64 } = require("@iov/encoding");
require('dotenv').config();

//Fees

let contracts = require("../contract_data.json");

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

async function getUserAddress(){
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

async function stake(client){
  let accAddress = await getUserAddress();
  const [StakingcontractCodeHash, Stakingcontract] = await contracts["staking"];
  const [OHMcontractCodeHash, OHMcontract] = await contracts["OHM"];
  const [StakingHelperContractCodeHash, StakingHelperContract] = contracts["staking-helper"];
  
  // We stake some OHM 
  handleMsg = {
      send : {
      "recipient":Stakingcontract.contractAddress,
      "recipient_code_hash":StakingcontractCodeHash,
      "amount":"1000000000",
      "msg" : Buffer.from(JSON.stringify({stake: {recipient: accAddress}})).toString('base64')
    }
  }
  response = await client.execute(OHMcontract.contractAddress,handleMsg);
  console.log("Staked some OHM");
  
}
async function claim(client){
  let accAddress = await getUserAddress();
  const [StakingcontractCodeHash, Stakingcontract] = await contracts["staking"];  

  // And then claim it from the warmup contract 
  handleMsg = {
      claim : {
      "recipient":accAddress
    }
  }
  response = await client.execute(Stakingcontract.contractAddress,handleMsg);
  console.log("And Claimed from Warmup");
  return response;  
}

async function manage_ust(client){

  let sUST = await getContractFromName("sUST");
  let treasury = await getContractFromName("treasury");
    handleMsg = {
      manage : {
      "token":sUST.contractAddress,
      "amount":"1000",
    }
  }
  response = await client.execute(treasury.contractAddress,handleMsg);
  console.log("Managing Reserves");
  return response; 
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

async function audit_reserves(client){
  let treasury = await getContractFromName("treasury");
    handleMsg = {
      audit_reserves:{}
  }
  response = await client.execute(treasury.contractAddress,handleMsg);
  console.log("Audit Reserves");
  return response; 
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

async function getContractInfo(client, contractName){
  let contractAddress = getContractFromName(contractName).contractAddress;
  let response = await client.queryContractSmart(contractAddress, { "contract_info": {}});
  return response.contract_info;
}

const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await getUserAddress();
  let response;

  /*
  response = await giveRole(client,accAddress,"ReserveManager");
  console.log(response);

  response = await getManagingAddress(client, "ReserveManager");
  console.log(response);
  */

  response = await manage_ust(client);
  console.log(response);

  response = await audit_reserves(client);
  console.log(response);

  response = await getContractInfo(client, "treasury");
  console.log(response);
}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
