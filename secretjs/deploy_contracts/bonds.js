const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const uuid = require("uuid");

const fs = require("fs");

const { fromUtf8 } = require("@iov/encoding");
require('dotenv').config();


//DAO Address
const DAOAddress = "secretnothing189765"

// Initial staking index
const initialIndex = '7675210820';

// First block epoch occurs
const firstEpochBlock = 8961000;

// What epoch will be first epoch
const firstEpochNumber = 338;

// How many blocks are in each epoch
const epochLengthInBlocks = 2200;

// Initial reward rate for epoch
const initialRewardRate = '3000';

// Initial mint for Frax and DAI (10,000,000)
const initialMint = '10000000000000000000000000';

// DAI bond BCV
const daiBondBCV = '369';

// Frax bond BCV
const fraxBondBCV = '690';

// Bond vesting length in blocks. 33110 ~ 5 days
const bondVestingLength = 33110;

// Min bond price
const minBondPrice = '50000';

// Max bond payout
const maxBondPayout = '50'

// DAO fee for bond
const bondFee = '10000';

// Max debt bond can take on
const maxBondDebt = '1000000000000000';

// Initial Bond debt
const initialBondDebt = '0'

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

async function upload_contract(contract_file, client, initMsg){

  // Upload the wasm of a simple contract
  const wasm = fs.readFileSync(contract_file);
  console.log(wasm);
  console.log('Uploading contract')
  const uploadReceipt = await client.upload(wasm, {});

  // Get the code ID from the receipt
  const codeId = uploadReceipt.codeId;
  console.log('codeId: ', codeId);

  // contract hash, useful for contract composition
  const contractCodeHash = await client.restClient.getCodeHashByCodeId(codeId);
  console.log(`Contract hash: ${contractCodeHash}`);

  // Create an instance of the Counter contract, providing a starting count
  const contract = await client.instantiate(codeId, initMsg, "My Counter" + Math.ceil(Math.random()*10000));
  console.log('contract: ', contract);
  return [contractCodeHash,contract]

}

const snip_contract = "../snip20impl/contract.wasm"


const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();

  const accAddress = await get_address();

  let rawdata = fs.readFileSync('contract_data.json');
  let contracts = JSON.parse(rawdata);

  let [sSCRTcontractCodeHash, sSCRTcontract] = await contracts["sSCRT"];
  let [sUSTcontractCodeHash, sUSTcontract] = await contracts["sUST"];
  let [OHMcontractCodeHash, OHMcontract] = await contracts["OHM"];
  let [sOHMcontractCodeHash, sOHMcontract] = await contracts["sOHM"];
  let [treasurycontractCodeHash, treasurycontract]  = await contracts["treasury"];
  let [CalculatorcontractCodeHash, Calculatorcontract] = await contracts["bond_calculator"];
  let [DistributorcontractCodeHash, Distributorcontract] = await contracts["staking_distributor"];
  let  [StakingcontractCodeHash, Stakingcontract] = await contracts["staking"];
  let [StakingWarmupContractCodeHash, StakingWarmupContract] = await contracts["staking-warmup"];
  let [sUSTBondContractCodeHash, sUSTBondContract] = contracts["sUST-bond"];
  let [sSCRTBondContractCodeHash, sSCRTBondContract] = contracts["sSCRT-bond"];

  // We upload the treasury contract

  const treasury_folder = "treasury"
  const treasury_contract = "../" + treasury_folder + "/contract.wasm"

  const treasuryInitMsg = {
      "name":"Fund treasury",
      "ohm": {"address":OHMcontract.contractAddress,"code_hash":OHMcontractCodeHash},
      "sohm" : {"address":sOHMcontract.contractAddress,"code_hash":sOHMcontractCodeHash},
      "reserve_tokens": [
        {"address":sUSTcontract.contractAddress,"code_hash":sUSTcontractCodeHash},
        {"address":sSCRTcontract.contractAddress,"code_hash":sSCRTcontractCodeHash},
        ],
      "blocks_needed_for_queue" : 0,
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  console.log(treasuryInitMsg);
  
  [treasurycontractCodeHash, treasurycontract] = await upload_contract(treasury_contract, client, treasuryInitMsg);

  contracts["treasury"] = [treasurycontractCodeHash, treasurycontract];

  //END


  //We add the reserve depositor roles to our treasury tokens

  handleMsg = {
      queue : {
      "address":sUSTBondContract.contractAddress,
      "role" : "ReserveDepositor", 
    }
  }
  console.log(handleMsg);
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      toggle_queue : {
      "address":sUSTBondContract.contractAddress,
      "role" : "ReserveDepositor", 

    }
  }
  console.log(handleMsg);
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      queue : {
      "address":sSCRTBondContract.contractAddress,
      "role" : "ReserveDepositor", 
    }
  }
  console.log(handleMsg);
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      toggle_queue : {
      "address":sSCRTBondContract.contractAddress,
      "role" : "ReserveDepositor", 

    }
  }
  console.log(handleMsg);
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  // queue and toggle reward manager
  handleMsg = {
      queue: {
        "address":Distributorcontract.contractAddress,
        "role" : "RewardManager", 
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      toggle_queue: {
        "address":Distributorcontract.contractAddress,
        "role" : "RewardManager", 
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  // queue and toggle deployer reserve depositor
  handleMsg = {
      queue: {
        "address":accAddress,
        "role" : "ReserveDepositor", 
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      toggle_queue: {
        "address":accAddress,
        "role" : "ReserveDepositor", 
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  // queue and toggle liquidity depositor
  handleMsg = {
      queue: {
        "address":accAddress,
        "role" : "LiquidityDepositor", 
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      toggle_queue: {
        "address":accAddress,
        "role" : "LiquidityDepositor", 
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));


  
  let data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  console.log('Successful upload');
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
