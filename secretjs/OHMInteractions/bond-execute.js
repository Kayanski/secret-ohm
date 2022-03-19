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
  console.log("Bonded some")
  return response;
}


async function redeem(client, bondName, stake) {
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

async function getBondTerms(client, bondName){
  let bondAddress = getContractFromName(bondName).contractAddress;
  const query = { 
      bond_terms: { }
  };
  let response = await client.queryContractSmart(bondAddress, query);
  return response
}

async function SetBondTerm(client, bondName, parameter, value){
  let bondAddress = getContractFromName(bondName).contractAddress;
  let handleMsg = {
    set_bond_term: {
      parameter: parameter,
      input: value
    },
  };
  let response = await client.execute(bondAddress, handleMsg);
  return response
}

async function changeAdmin(bondName,client,address){
  let bondAddress = getContractFromName(bondName).contractAddress;
  let handleMsg = {
    change_admin: {
      address: address,
    },
  };
  let response = await client.execute(bondAddress, handleMsg);
  return response
}

async function getContractInfo(client, contractName){
  let address = getContractFromName(contractName).contractAddress;
  let response = await client.queryContractSmart(address, { "contract_info": {}});
  return response.contract_info;
}
async function giveRole(client, contract, role, calculator=undefined){

  let treasurycontract = getContractFromName("treasury");
   // queue and toggle reward manager
  handleMsg = {
      queue: {
        "address":contract.address,
        "role" : role
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

  handleMsg = {
      toggle_token_queue: {
        "token":contract,
        "role" : role,
        calculator:calculator
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

}

async function toggle(client, contract, role, calculator=undefined){

  let treasurycontract = getContractFromName("treasury");
  let handleMsg = {
      toggle_token_queue: {
        "token":contract,
        "role" : role,
        calculator:calculator
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));
}
const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await getUserAddress();
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
  const [StakingHelperContractCodeHash, StakingHelperContract] = contracts["staking-helper"];


  //We check the treasury has the liquidity token registered

  response = await bond(client,"OHM-UST-LP-bond","1000000");
  //response = await bond(client,"sUST-bond","1000000");
  //response = await bond(client,"sUST-bond","1000000");
  //response = await bond(client,"sUST-bond","1000000");
  //response = await bond(client,"sUST-bond","1000000");
  //response = await redeem(client, "sUST-bond", false);
  console.log(Buffer.from(response.data).toString("utf8"));
  //response = await setAdjustment("sUST-bond",client)
  //console.log(new Buffer.from(response.data).toString());
  /*
  let response = await SetBondTerm(client, "sUST-bond","vesting", "10001");
  response = await SetBondTerm(client, "sUST-bond","payout", "1");
  response = await SetBondTerm(client, "sUST-bond","fee", "1");
  response = await SetBondTerm(client, "sUST-bond","debt", "1");

  response = await getBondTerms(client,"sUST-bond");
  console.log(response);

  response = await changeAdmin("sUST-bond",client,"secretbloob");
  */

  response = await getContractInfo(client, "sUST-bond");
  console.log(response);
  /*
  HandleMsg::SetBondTerm{parameter,input} => set_bond_terms(deps,env, parameter, input.u128()),

  HandleMsg::SetAdjustment{addition,increment, target, buffer} =>
      set_adjustment(deps,env,addition,increment.u128(), target.u128(), buffer),
  HandleMsg::SetStaking{staking} => set_staking(deps,env,staking),
  HandleMsg::Redeem{recipient,stake} => redeem(deps,env,recipient,stake),
  HandleMsg::RecoverLostToken{token} => recover_lost_token(deps,env,token),

  // Other
  HandleMsg::ChangeAdmin { address, .. } => change_admin(deps, env, address),
  HandleMsg::RevokePermit { permit_name, .. } => revoke_permit(deps, env, permit_name),
  */

}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
