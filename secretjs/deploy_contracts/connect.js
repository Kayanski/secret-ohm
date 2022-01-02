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


  process.argv.forEach((val, index) => {
    console.log(`${index}: ${val}`)
  })
  
  // Create connection to DataHub Secret Network node
  const client = await get_client();

  const accAddress = await get_address();

  let contracts = {};

  // We start by deploying the token
  const OhmInitMsg = {
      "config": {
        "enable_burn": true,
        "enable_mint": true,
        "public_total_supply": true
      },
      "decimals": 9,
      "initial_balances": [],
      "name": "FondCommun",
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
      "symbol": "FCT"
  }
  
  const [OHMcontractCodeHash, OHMcontract] = await upload_contract(snip_contract, client, OhmInitMsg);
  
  contracts["OHM"] = [OHMcontractCodeHash, OHMcontract];
  //Then we deploy sUST and sSCRT to to interact with the treasury
   const sUSTInitMsg = {
      "config": {
        "enable_burn": true,
        "enable_mint": true,
        "public_total_supply": true
      },
      "decimals": 6,
      "initial_balances": [
      {
        "address": accAddress,
        "amount": initialMint
      }
      ],
      "name": "FondCommun",
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
      "symbol": "SUST"
  }
  
  const [sUSTcontractCodeHash, sUSTcontract] = await upload_contract(snip_contract, client, sUSTInitMsg);

  console.log("Deployed sUST");

  contracts["sUST"] = [sUSTcontractCodeHash, sUSTcontract];

  let data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

   const sSCRTInitMsg = {
      "config": {
        "enable_burn": true,
        "enable_mint": true,
        "public_total_supply": true,
        "enable_deposit": true,
        "enable_redeem": true,
      },
      "decimals": 6,
      "initial_balances": [
      {
        "address": accAddress,
        "amount": initialMint
      }],
      "name": "FondCommun",
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
      "symbol": "SSCRT"
  }
  
  const [sSCRTcontractCodeHash, sSCRTcontract] = await upload_contract(snip_contract, client, sSCRTInitMsg)

  console.log("Deployed sSCRT");

  contracts["sSCRT"] = [sSCRTcontractCodeHash, sSCRTcontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);


  let handleMsg;
  let response;

  funds =  [{amount:"100",
  denom:"uscrt",
            },];

  handleMsg = {
    deposit:{}
  };
  const reponse = await client.execute(sSCRTcontract.contractAddress,handleMsg,"",funds);

   // We upload the sOHM contract

  const sohm_folder = "s-ohm"
  const sohm_contract = "../" + sohm_folder + "/contract.wasm"

  const sOHMInitMsg = {
      "decimals" : 9,
      "index": initialIndex,
      "name" : "Staked Fund",
      "symbol" : "SOHM",
      "config": {
        "public_total_supply": true,
      },
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  
  const [sOHMcontractCodeHash, sOHMcontract] = await upload_contract(sohm_contract, client, sOHMInitMsg);

  console.log("Deployed sOHM");

  contracts["sOHM"] = [sOHMcontractCodeHash, sOHMcontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);
  
  //END


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
  
  const [treasurycontractCodeHash, treasurycontract] = await upload_contract(treasury_contract, client, treasuryInitMsg);

  console.log("Deployed Treasury");

  contracts["treasury"] = [treasurycontractCodeHash, treasurycontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  // We upload the bond calculator contract

  let folder = "bond_calculator"
  let contract = "../" + folder + "/contract.wasm"

  let InitMsg = {
      "ohm": {"address":OHMcontract.contractAddress,"code_hash":OHMcontractCodeHash},
  }
  
  const [CalculatorcontractCodeHash, Calculatorcontract] = await upload_contract(contract, client, InitMsg);

  console.log("Deployed Bond Calculator");

  contracts["bond_calculator"] = [CalculatorcontractCodeHash, Calculatorcontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END
// We upload the distributor contract

  folder = "staking-distributor"
  contract = "../" + folder + "/contract.wasm"

  InitMsg = {
      "treasury": {"address":treasurycontract.contractAddress,"code_hash":treasurycontractCodeHash},
      "ohm": {"address":OHMcontract.contractAddress,"code_hash":OHMcontractCodeHash},
      "epoch_length": epochLengthInBlocks, 
      "next_epoch_block": firstEpochBlock,
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  console.log(InitMsg);
  const [DistributorcontractCodeHash, Distributorcontract] = await upload_contract(contract, client, InitMsg);

  console.log("Deployed Staking Distributor");

  contracts["staking_distributor"] = [DistributorcontractCodeHash, Distributorcontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  // We upload the staking contract

  folder = "staking"
  contract = "../" + folder + "/contract.wasm"

  InitMsg = {
      "epoch_length":epochLengthInBlocks,
      "first_epoch_block": firstEpochBlock,
      "first_epoch_number": firstEpochNumber,
      "ohm": {"address":OHMcontract.contractAddress,"code_hash":OHMcontractCodeHash},
      "sohm": {"address":sOHMcontract.contractAddress,"code_hash":sOHMcontractCodeHash},
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  
  const [StakingcontractCodeHash, Stakingcontract] = await upload_contract(contract, client, InitMsg);

  console.log("Deployed Staking");

  contracts["staking"] = [StakingcontractCodeHash, Stakingcontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  // We upload the staking warmup contract

  folder = "staking-warmup"
  contract = "../" + folder + "/contract.wasm"

  InitMsg = {
      "staking": {"address":Stakingcontract.contractAddress,"code_hash":StakingcontractCodeHash},
      "sohm": {"address":sOHMcontract.contractAddress,"code_hash":sOHMcontractCodeHash},
  }
  
  const [StakingWarmupContractCodeHash, StakingWarmupContract] = await upload_contract(contract, client, InitMsg);

  console.log("Deployed StakingWarmup");
  contracts["staking-warmup"] = [StakingWarmupContractCodeHash, StakingWarmupContract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  // We upload the sUST bond

  folder = "bond_depository"
  contract = "../" + folder + "/contract.wasm"

  InitMsg = {
    "dao" : DAOAddress,
    "ohm" :  {"address":OHMcontract.contractAddress,"code_hash":OHMcontractCodeHash},
    "principle" :  {"address":sUSTcontract.contractAddress,"code_hash":sUSTcontractCodeHash},
    "treasury" : {"address":treasurycontract.contractAddress,"code_hash":treasurycontractCodeHash},
    "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  
  const [sUSTBondContractCodeHash, sUSTBondContract] = await upload_contract(contract, client, InitMsg);

  console.log("Deployed Bond depository (sUST)");
  contracts["sUST-bond"] = [sUSTBondContractCodeHash, sUSTBondContract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END


 // We upload the sSCRT bond

  var prng_seed= uuid.v4();
  folder = "bond_depository"
  contract = "../" + folder + "/contract.wasm"

  InitMsg = {
    "dao" : DAOAddress,
    "ohm" :  {"address":OHMcontract.contractAddress,"code_hash":OHMcontractCodeHash},
    "principle" :  {"address":sSCRTcontract.contractAddress,"code_hash":sSCRTcontractCodeHash},
    "treasury" : {"address":treasurycontract.contractAddress,"code_hash":treasurycontractCodeHash},
    "prng_seed" : Buffer.from(prng_seed).toString('base64'),
  }
  
  const [sSCRTBondContractCodeHash, sSCRTBondContract] = await upload_contract(contract, client, InitMsg);

  console.log("Deployed sSCRT bond (sSCRT)");
  contracts["sSCRT-bond"] = [sSCRTBondContractCodeHash, sSCRTBondContract] ;
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  //Now we can interact with the deployed contracts

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

  //We set the bond terms
  handleMsg = {
        initialize_bond_terms : {
          "control_variable": daiBondBCV,
          "fee" : bondFee,
          "initial_debt" : initialBondDebt,
          "max_debt" : maxBondDebt,
          "max_payout" : maxBondPayout,
          "minimum_price" : minBondPrice,
          "vesting_term" : bondVestingLength,
      }
    }
    response = await client.execute(sUSTBondContract.contractAddress,handleMsg);
    console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

     handleMsg = {
        initialize_bond_terms : {
          "control_variable": daiBondBCV,
          "fee" : bondFee,
          "initial_debt" : initialBondDebt,
          "max_debt" : maxBondDebt,
          "max_payout" : maxBondPayout,
          "minimum_price" : minBondPrice,
          "vesting_term" : bondVestingLength,
      }
    }
    response = await client.execute(sSCRTBondContract.contractAddress,handleMsg);
    console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  // Set staking for sUST and sSCRT bonds
  handleMsg = {
      set_staking : {
        "staking": {"address":Stakingcontract.contractAddress,"code_hash":StakingcontractCodeHash},
    }
  }
  response = await client.execute(sUSTBondContract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      set_staking : {
         "staking": {"address":Stakingcontract.contractAddress,"code_hash":StakingcontractCodeHash},
    }
  }
  response = await client.execute(sSCRTBondContract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

   // Initialize sOHM

  handleMsg = {
      initialize : {
         "staking_contract": Stakingcontract.contractAddress,
    }
  }
  response = await client.execute(sOHMcontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

// set distributor contract and warmup contract
    handleMsg = {
      set_contract : {
        "contract_type" : "distributor",
        "contract" : {"address":Distributorcontract.contractAddress,"code_hash":DistributorcontractCodeHash},
    }
  }
  response = await client.execute(Stakingcontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

  handleMsg = {
      set_contract : {
        "contract_type" : "warmup_contract",
        "contract" : {"address":StakingWarmupContract.contractAddress,"code_hash":StakingWarmupContractCodeHash},
    }
  }
  response = await client.execute(Stakingcontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", JSON.parse(fromUtf8(response.data)));

// Add staking contract as distributor recipient
    handleMsg = {
      add_recipient : {
        "recipient" : Stakingcontract.contractAddress,
        "reward_rate" : initialRewardRate,
    }
  }
  console.log(handleMsg);
  response = await client.execute(Distributorcontract.contractAddress,handleMsg);
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

  /* Allow the treasury to mint new OHM */
  handleMsg = {
    add_minters:{
      "minters" : [treasurycontract.contractAddress],
    }
  };
  response = await client.execute(OHMcontract.contractAddress,handleMsg); 
  console.log("Treasury set as an OHM minter");

  /* Deposit sUST into the treasury */
  handleMsg = {
    send:{
      "amount" : "9000000000000",
      "recipient" : treasurycontract.contractAddress,
      "recipient_code_hash" : treasurycontractCodeHash,
      "msg": Buffer.from(JSON.stringify({deposit: {profit: "8400000000000000"}})).toString('base64'),
    }
  };
  console.log(handleMsg);
  reponse = await client.execute(sUSTcontract.contractAddress,handleMsg); 
  console.log("Deposited UST");

  /* Deposit sSCRT into the treasury */
  handleMsg = {
    send:{
      "amount" : "5000000000000",
      "recipient" : treasurycontract.contractAddress,
      "recipient_code_hash" : treasurycontractCodeHash,
      "msg": Buffer.from(JSON.stringify({deposit: {profit: "5000000000000000"}})).toString('base64'),
    }
  };
  console.log(handleMsg);

  response = await client.execute(sUSTcontract.contractAddress,handleMsg); 
  console.log("Deposited sSCRT");

  // Query chain ID
  const chainId = await client.getChainId()

  // Query chain height
  const height = await client.getHeight()

  console.log("ChainId:", chainId);
  console.log("Block height:", height);

  console.log('Successfully connected to Secret Network');
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
