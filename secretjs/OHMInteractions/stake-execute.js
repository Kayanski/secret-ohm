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
      "amount":"1000000000",
      "msg" : Buffer.from(JSON.stringify({stake: {recipient: accAddress}})).toString('base64')
    }
  }
  response = await client.execute(OHMcontract.contractAddress,handleMsg);
  console.log("Staked some OHM");
  return response
  
}

async function unstake(client,amount) {
  let staking = getContractFromName("staking");
  let stakingCodeHash = getCodeHashFromName("staking");
  let sOHMContract = getContractFromName("sOHM");
  // Stake
  let handleMsg = {
    send: {
      recipient: staking.contractAddress,
      recipient_code_hash: stakingCodeHash,
      amount: amount,
      msg: Buffer.from(
        JSON.stringify({
          unstake: {
            trigger: false,
          },
        })
      ).toString("base64"),
    },
  };
  let response = await client.execute(
    sOHMContract.contractAddress,
    handleMsg, 
  );
  return response;
}

async function setWarmupPeriod(client,period){
  let stakingContract = getContractFromName("staking");

  let handleMsg = {
    set_warmup_period: {
      warmup_period: period,
    },
  };

  let response = await client.execute(
    stakingContract.contractAddress,
    handleMsg
  );
  return response;
}

const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await getUserAddress();
  let response;

  response = await unstake(client,"501800000000000");
  //response = await stake_helper(client);
  //response = await setWarmupPeriod(client, 0);
  //console.log(response);
  /*
//Register Receive messages
        HandleMsg::Receive {
            from, amount, msg, ..
        } => receive(deps, env, from, amount.u128(), msg),

        // Other
        HandleMsg::ChangeAdmin { address, .. } => change_admin(deps, env, address),
        HandleMsg::SetContractStatus { level, .. } => set_contract_status(deps, env, level),

        //Staking
        HandleMsg::Rebase { .. } => rebase(deps, env),
        HandleMsg::Claim { recipient, .. } => claim(deps, recipient),
        HandleMsg::Forfeit { .. } => forfeit(deps, env),
        HandleMsg::ToggleDepositLock { .. } => toggle_deposit_lock(deps, env),
        HandleMsg::GiveLockBonus { amount, .. } => give_lock_bonus(deps, env, amount),
        HandleMsg::SetContract {
            contract_type,
            contract,
            ..
        } => set_contract(deps, env, contract_type, contract),
        HandleMsg::SetWarmupPeriod { warmup_period, .. } => {
            set_warmup_period(deps, env, warmup_period)
        }
        */
}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
