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
      "amount":"1000000000000",
      "msg" : Buffer.from(JSON.stringify({stake: {recipient: accAddress}})).toString('base64')
    }
  }
  response = await client.execute(OHMcontract.contractAddress,handleMsg);
  console.log("Staked some OHM");
  
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


async function stake_helper(client){

  // This is supposed to be the same as : 
  /* We stake some OHM */
  let accAddress = await getUserAddress();
const [StakingcontractCodeHash, Stakingcontract] = await contracts["staking"];
  const [OHMcontractCodeHash, OHMcontract] = await contracts["OHM"];
  const [StakingHelperContractCodeHash, StakingHelperContract] = contracts["staking-helper"];
  handleMsg = {
      send : {
      "recipient":StakingHelperContract.contractAddress,
      "recipient_code_hash":StakingHelperContractCodeHash,
      "amount":"1000000000000",
      "msg" : Buffer.from(JSON.stringify({stake: {recipient: accAddress}})).toString('base64')
    }
  }
  response = await client.execute(OHMcontract.contractAddress,handleMsg);
  console.log("Staked some OHM");
  return response
  
}

async function bond(client,bondName,amount){
  let accAddress = await getUserAddress();
  let bondContract = getContractFromName(bondName);
  let bondCodeHash = getCodeHashFromName(bondName);
  let principleContract = getContractFromName(bondContract.principle);

  let send_amount = parseFloat(amount)
  let handleMsg = {
    send: {
      recipient: bondContract.contractAddress,
      recipient_code_hash: bondCodeHash,
      amount: String(send_amount),
      msg: Buffer.from(
        JSON.stringify({
          deposit: {
            max_price: "60000000",
            depositor: accAddress
          },
        })
      ).toString("base64"),
    },
  };

  let response = await client.execute(
    principleContract.contractAddress,
    handleMsg
  );
  return response;
}


async function redeem(bondName, stake,client) {
  let accAddress = await getUserAddress();
  let bondContract = getContractFromName(bondName);

  let handleMsg = {
    redeem: {
      recipient: accAddress,
      stake: stake
    },
  };

  let response = await client.execute(
    bondContract.contractAddress,
    handleMsg
  );
  return response;
}

async function setAdjustment(bondName, client) {
  let bondContract = getContractFromName(bondName);

  let handleMsg = {
    set_adjustment: {
      addition:true,
      increment : "4",
      target: "500",
      buffer: 1,
    },
  };

  let response = await client.execute(
    bondContract.contractAddress,
    handleMsg
  );
  return response;
}

const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await getUserAddress();

/*

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
  const [StakingHelperContractCodeHash, StakingHelperContract] = contracts["staking-helper"];
*/

  //let response = await stake(client, contracts, accAddress);
  //let response = await stake_helper(client, contracts, accAddress);
  //let response = await bond(client,contracts,accAddress,"sUST-bond","100000000");
  let response = await redeem("sUST-bond", false,client);
  
  response = await setAdjustment("sUST-bond",client)
  console.log(new Buffer.from(response.data).toString());


}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
