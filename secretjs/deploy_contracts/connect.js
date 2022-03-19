const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey,makeSignBytes
} = require("secretjs");

const uuid = require("uuid");

const fs = require("fs");

// By default, we use the local testnet

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
let gasPrices = require("../gas_prices.json");
let gasFactor = 1.1;
let gasPrice = 0.3;
//DAO Address
const DAOAddress = "secret1878ru0hfgdk0atdvj9kvcl0gfzkfjr25m4pd94";
const DAOMnemonicSecretKey = "quality isolate target melody flame adjust actress funny wear art sister capital banana orient duty settle until wire profit evidence violin side muscle obey";


// Initial staking index
const initialIndex = '7675210820';

// First block epoch occurs
const firstEpochBlock = 1219200;

// What epoch will be first epoch
const firstEpochNumber = 254;

// How many blocks are in each epoch
// 4800 ~ 8hs
//const epochLengthInBlocks = 4800;
const epochLengthInBlocks = 4800;

// Initial reward rate for epoch
const initialRewardRate = '3000';

// Initial mint for Frax and DAI (10,000,000)
const initialMint = '10000000000000';

// DAI bond BCV
const daiBondBCV = '369';

// Frax bond BCV
const fraxBondBCV = '690';

// Bond vesting length in blocks. 33110 ~ 5 days
const bondVestingLength = 72000;

// Min bond price
const minBondPrice = '200';

// Max bond price
const maxBondPrice = '1000';

// Max bond payout
const maxBondPayout = '50'

// DAO fee for bond
const bondFee = '50';

// Max debt bond can take on
const maxBondDebt = '1000000000000000';

// Initial Bond debt
const initialBondDebt = '1000000000'

const initialUSTDeposit     = "3000000000";

const initialTreasuryProfit = "1000000000000";

const initialStake          = "1000000000000";

const initialUSTLiquidity = "500000000";

const initialOVLLiquidity = "100000000000";

//Fees


const customFees = {
  upload: {
      amount: [{ amount: "4000000", denom: "uscrt" }],
      gas: "4000000",
  },
  init: {
      amount: [{ amount: "500000", denom: "uscrt" }],
      gas: "200000",
  },
  exec: {
      amount: [{ amount: "500000", denom: "uscrt" }],
      gas: "200000",
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

async function queue(client, address, role){
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
}
async function giveRole(client, address, role){

  await queue(client, address, role);
  let treasurycontract = getContractFromName("treasury");
  handleMsg = {
      toggle_queue: {
        "address":address,
        "role" : role
    }
  }
  response = await client.execute(treasurycontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

}

async function giveTokenRole(client, contract, role, calculator=undefined){

  let treasurycontract = getContractFromName("treasury");
   // queue and toggle reward manager
  await queue(client, contract.address, role);

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


function getStoredFee(contractName,operation){

  if(contractName in gasPrices){
    let gas = parseInt(gasPrices[contractName][operation]*gasFactor);
    let amount = parseInt(gas*gasPrice);
    let gasString = gas.toString()
    let amountString = amount.toString()
    let uploadPrice = {
      amount: [{ amount: amountString, denom: "uscrt" }],
      gas: gasString,
    }
    console.log("Using custom Fees");
    return uploadPrice;
  }
  console.log("Using standard Fees");
}


async function upload_code(contract_file,client,contractName="Default Contract"){

  // Upload the wasm of a simple contract
  const wasm = fs.readFileSync(contract_file);
  console.log('Uploading contract')

  let uploadReceipt;
  let uploadFee = getStoredFee(contractName,"store");
  if(uploadFee){
    uploadReceipt = await client.upload(wasm, {}, "", uploadFee);
  }else{
    uploadReceipt = await client.upload(wasm, {});
  }

  // Get the code ID from the receipt
  const codeId = uploadReceipt.codeId;
  console.log('codeId: ', codeId);

  const contractCodeHash = await client.restClient.getCodeHashByCodeId(codeId);

  return [codeId,contractCodeHash];

}

async function instantiate_contract(client, codeId, initMsg, contractName){

  // Getting the fees from file
  let contract;
  let initFee = getStoredFee(contractName,"init");
  if(initFee){
    contract = await client.instantiate(
      codeId, 
      initMsg, 
      contractName + Math.ceil(Math.random()*10000),
      "",
      [],
      initFee
    );
  }else{
    contract = await client.instantiate(codeId, initMsg, contractName + Math.ceil(Math.random()*10000));
  }
  return contract;
}

async function upload_contract(contract_file, client, initMsg, contractName = "Default Contract"){

  let [codeId,contractCodeHash] = await upload_code(contract_file,client,contractName);

  contract = await instantiate_contract(client, codeId, initMsg, contractName);

  return [contractCodeHash,contract]

}

const snip_contract = "../snip20impl/contract.wasm"
const testnet_snip_contract = "../snip20impl-testnet/contract.wasm"


const main = async () => {

  if(arguments[3] == "transfer"){

    // Transfer to the selected address, to always use the same address
    let client = await get_client(process.env.MNEMONIC_TESTNET);



    const signingPen = await Secp256k1Pen.fromMnemonic(process.env.MNEMONIC_TESTNET);
    const memo = 'My first secret transaction, sending uscrt to my own address';

    const sendMsg = {
        type: "cosmos-sdk/MsgSend",
        value: {
            from_address: await get_testnet_default_address(),
            to_address: await get_address(),
            amount: [
                {
                    denom: "uscrt",
                    amount: "100000000000000000",
                },
            ],
        },
    };

    const fee = {
        amount: [
            {
                amount: "50000",
                denom: "uscrt",
            },
        ],
        gas: "100000",
    };
    
    const chainId = await client.getChainId();
    const { accountNumber, sequence } = await client.getNonce(await get_testnet_default_address());
    const signBytes = makeSignBytes([sendMsg], fee, chainId, memo, accountNumber, sequence);
    const signature = await signingPen.sign(signBytes);
    const signedTx = {
        msg: [sendMsg],
        fee: fee,
        memo: memo,
        signatures: [signature],
    };
    const { logs, transactionHash } = await client.postTx(signedTx);
    console.log(logs,transactionHash);
  }
  
  console.log("On démarre")
  // Create connection to DataHub Secret Network node
  client = await get_client();

  const accAddress = await get_address();

   //Maybe we use the code only once
   const snip20_folder = "snip20impl"
  const snip20_contract = "../" + snip20_folder + "/contract.wasm"
  //We start by uploading the token contract
  let [tokenId, tokenHash] = await upload_code(snip20_contract,client,"token");
  console.log("Token sur le réseau !!");


  const tokenName = "OVL";
  // We start by deploying the token
  const OVLInitMsg = {
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
  
  let OVLcontract = await instantiate_contract(client, tokenId, OVLInitMsg, tokenName);
  let OVLcontractCodeHash = tokenHash;

  //const [OVLcontractCodeHash, OVLcontract] = await upload_contract(snip_contract, client, OVLInitMsg, tokenName);
  
  contracts["OVL"] = [OVLcontractCodeHash, OVLcontract];
  console.log("Deployed" + tokenName);

  //Then we deploy sUST to interact with the treasury
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

  let [sUSTcontractCodeHash, sUSTcontract] = await upload_contract(testnet_snip_contract, client, sUSTInitMsg, "sUST");
  
  console.log("Deployed sUST");

  contracts["sUST"] = [sUSTcontractCodeHash, sUSTcontract];

  let data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

   // We upload the sOVL contract

  const sOVL_folder = "s-OVL"
  const sOVL_contract = "../" + sOVL_folder + "/contract.wasm"

  const sOVLInitMsg = {
      "decimals" : 9,
      "index": initialIndex,
      "name" : "Staked Fund",
      "symbol" : "SOVL",
      "config": {
        "public_total_supply": true,
      },
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  
  const [sOVLcontractCodeHash, sOVLcontract] = await upload_contract(sOVL_contract, client, sOVLInitMsg, "s" + tokenName);

  console.log("Deployed sOVL");

  contracts["sOVL"] = [sOVLcontractCodeHash, sOVLcontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);
  
  //END

  // We upload the treasury contract

  const treasury_folder = "treasury"
  const treasury_contract = "../" + treasury_folder + "/contract.wasm"

  const treasuryInitMsg = {
      "name":"Fund treasury",
      "OVL": {"address":OVLcontract.contractAddress,"code_hash":OVLcontractCodeHash},
      "sOVL" : {"address":sOVLcontract.contractAddress,"code_hash":sOVLcontractCodeHash},
      "reserve_tokens": [
        {"address":sUSTcontract.contractAddress,"code_hash":sUSTcontractCodeHash},
        ],
      "blocks_needed_for_queue" : 0,
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  
  const [treasurycontractCodeHash, treasurycontract] = await upload_contract(treasury_contract, client, treasuryInitMsg, "treasury");

  console.log("Deployed Treasury");

  contracts["treasury"] = [treasurycontractCodeHash, treasurycontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  // We upload the bond calculator contract

  let folder = "bond_calculator"
  let contract = "../" + folder + "/contract.wasm"

  let InitMsg = {
      "OVL": {"address":OVLcontract.contractAddress,"code_hash":OVLcontractCodeHash},
  }
  const [CalculatorcontractCodeHash, Calculatorcontract] = await upload_contract(contract, client, InitMsg, "bond_calculator");

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
      "OVL": {"address":OVLcontract.contractAddress,"code_hash":OVLcontractCodeHash},
      "epoch_length": epochLengthInBlocks, 
      "next_epoch_block": firstEpochBlock,
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  const [DistributorcontractCodeHash, Distributorcontract] = await upload_contract(contract, client, InitMsg, "distributor");

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
      "OVL": {"address":OVLcontract.contractAddress,"code_hash":OVLcontractCodeHash},
      "sOVL": {"address":sOVLcontract.contractAddress,"code_hash":sOVLcontractCodeHash},
      "prng_seed" : Buffer.from("Something really random").toString('base64'),
  }
  
  const [StakingcontractCodeHash, Stakingcontract] = await upload_contract(contract, client, InitMsg,"staking");

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
      "sOVL": {"address":sOVLcontract.contractAddress,"code_hash":sOVLcontractCodeHash},
  }
  
  const [StakingWarmupContractCodeHash, StakingWarmupContract] = await upload_contract(contract, client, InitMsg, "warmup");

  console.log("Deployed StakingWarmup");
  contracts["staking-warmup"] = [StakingWarmupContractCodeHash, StakingWarmupContract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END


  // We upload the staking helper contract

  folder = "staking-helper"
  contract = "../" + folder + "/contract.wasm"

  InitMsg = {
      "staking": {"address":Stakingcontract.contractAddress,"code_hash":StakingcontractCodeHash},
      "OVL": {"address":OVLcontract.contractAddress,"code_hash":OVLcontractCodeHash},
  }
  
  const [StakingHelperContractCodeHash, StakingHelperContract] = await upload_contract(contract, client, InitMsg, "helper");

  console.log("Deployed StakingHelper");
  contracts["staking-helper"] = [StakingHelperContractCodeHash, StakingHelperContract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END


  // We upload the sUST bond

  folder = "bond_depository"
  contract = "../" + folder + "/contract.wasm"

  var prng_seed = uuid.v4();
  InitMsg = {
    "dao" : DAOAddress,
    "OVL" :  {"address":OVLcontract.contractAddress,"code_hash":OVLcontractCodeHash},
    "principle" :  {"token":{
      "address":sUSTcontract.contractAddress,
      "code_hash":sUSTcontractCodeHash
    }},
    "treasury" : {"address":treasurycontract.contractAddress,"code_hash":treasurycontractCodeHash},
    "prng_seed" : Buffer.from(prng_seed).toString('base64'),
    "symbol": "BSUST",
    "name": "sUST bond"
  }
  
  let [sUSTBondContractCodeHash, sUSTBondContract] = await upload_contract(contract, client, InitMsg,"sUSTBond");
  sUSTBondContract.principle = "sUST"
  console.log("Deployed Bond depository (sUST)");
  contracts["sUST-bond"] = [sUSTBondContractCodeHash, sUSTBondContract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  //Now we can interact with the deployed contracts


  //We add the reserve depositor roles to our treasury tokens
  await giveRole(client, sUSTBondContract.contractAddress,"ReserveDepositor");
  console.log("Added the sUST bond Reservedepositor role");


  //We set the bond terms
  handleMsg = {
        initialize_bond_terms : {
          "control_variable": daiBondBCV,
          "fee" : bondFee,
          "initial_debt" : initialBondDebt,
          "max_debt" : maxBondDebt,
          "max_payout" : maxBondPayout,
          "minimum_price" : minBondPrice,
          "maximum_price" : maxBondPrice,
          "vesting_term" : bondVestingLength,
      }
    }
  response = await client.execute(sUSTBondContract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

  // Set staking for sUST bond
  handleMsg = {
      set_staking : {
        "staking": {"address":Stakingcontract.contractAddress,"code_hash":StakingcontractCodeHash},
    }
  }
  response = await client.execute(sUSTBondContract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

   // Initialize sOVL
  handleMsg = {
      initialize : {
         "staking_contract": Stakingcontract.contractAddress,
    }
  }
  response = await client.execute(sOVLcontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

// set distributor contract and warmup contract
    handleMsg = {
      set_contract : {
        "contract_type" : "distributor",
        "contract" : {"address":Distributorcontract.contractAddress,"code_hash":DistributorcontractCodeHash},
    }
  }
  response = await client.execute(Stakingcontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

  handleMsg = {
      set_contract : {
        "contract_type" : "warmup_contract",
        "contract" : {"address":StakingWarmupContract.contractAddress,"code_hash":StakingWarmupContractCodeHash},
    }
  }
  response = await client.execute(Stakingcontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

// Add staking contract as distributor recipient
    handleMsg = {
      add_recipient : {
        "recipient" : Stakingcontract.contractAddress,
        "reward_rate" : initialRewardRate,
    }
  }
  console.log(handleMsg);
  response = await client.execute(Distributorcontract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

  await giveRole(client, Distributorcontract.contractAddress,"RewardManager");
  console.log("Added the distributor RewardManager role");

  await giveRole(client, accAddress,"ReserveDepositor");
  console.log("Added the curretn address ReserveDepositor role");

  await giveRole(client, accAddress,"LiquidityDepositor");
  console.log("Added the current address LiquidityDepositor role");


  /* Allow the treasury to mint new OVL */
  handleMsg = {
    add_minters:{
      "minters" : [treasurycontract.contractAddress],
    }
  };
  response = await client.execute(OVLcontract.contractAddress,handleMsg); 
  console.log("Treasury set as an OVL minter");

  /* Deposit sUST into the treasury */
  handleMsg = {
    send:{
      "amount" : initialUSTDeposit,
      "recipient" : treasurycontract.contractAddress,
      "recipient_code_hash" : treasurycontractCodeHash,
      "msg": Buffer.from(JSON.stringify({deposit: {profit: initialTreasuryProfit }})).toString('base64'),
    }
  };

  response = await client.execute(sUSTcontract.contractAddress,handleMsg); 
  console.log("Deposited UST");


  // We stake some OVL 
  handleMsg = {
      send : {
      "recipient":Stakingcontract.contractAddress,
      "recipient_code_hash":StakingcontractCodeHash,
      "amount":initialStake,
      "msg" : Buffer.from(JSON.stringify({stake: {recipient: accAddress}})).toString('base64')
    }
  }

  response = await client.execute(OVLcontract.contractAddress,handleMsg);
  console.log("Staked some OVL");
  
  /* And then claim it from the warmup contract */
  handleMsg = {
      claim : {
      "recipient":accAddress
    }
  }

  console.log(handleMsg);
  response = await client.execute(Stakingcontract.contractAddress,handleMsg);
  console.log("And Claimed from Warmup");

  // We deploy some liquidity to the secretswap pairs

    // First we create a new pair, with the created tokens
  const create_pair = {
    "create_pair": {
      "asset_infos": [
        {
          "token": {
            "contract_addr": getContractFromName("sUST").contractAddress,
            "token_code_hash": getCodeHashFromName("sUST"),
            "viewing_key": "" // ignored, can be whatever
          }
        },
        {
          "token": {
            "contract_addr": getContractFromName("OVL").contractAddress,
            "token_code_hash": getCodeHashFromName("OVL"),
            "viewing_key": "" // ignored, can be whatever
          }
        }
      ]
    }
  }

  let factoryAddress = contracts["pair-factory"][1].contractAddress

  response = await client.execute(factoryAddress,create_pair);
  
  let liquidityTokenAddr,pairContractAddr;
  let LPtokenHash = contracts["pair-factory"][0]["LPTokenCodeHash"],pairHash = contracts["pair-factory"][0]["pairCodeHash"];

  response.logs[0].events[1].attributes.forEach(function(log){
    if(log.key == "liquidity_token_addr"){
      liquidityTokenAddr = log.value;
    }else if (log.key == "pair_contract_addr"){
      pairContractAddr = log.value;
    }
  });

  console.log("Created the OVL-UST pair");
  contracts["OVL-UST-LP"] = {};
  contracts["OVL-UST-LP"] = [
    LPtokenHash,
    {
      contractAddress:liquidityTokenAddr,
      "pair":[pairHash,{contractAddress:pairContractAddr}]
    }
  ];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //Then we register the liquidity token with the treasury


  let liquidity_token_contract = {
    address: contracts["OVL-UST-LP"][1].contractAddress,
    code_hash: contracts["OVL-UST-LP"][0],
    pair_address: contracts["OVL-UST-LP"][1].pair[1].contractAddress,
    pair_code_hash: contracts["OVL-UST-LP"][1].pair[0],
  }

  let calculator = {
    code_hash:CalculatorcontractCodeHash,
    address:Calculatorcontract.contractAddress,
  }

  await giveTokenRole(client, liquidity_token_contract, "LiquidityToken", calculator);


  //Then we have to provide some liquidity for the contract to work

  // First we set an allowance for the pair to spend the tokens

  await increase_allowance(client, "sUST", pairContractAddr, initialUSTLiquidity);
  await increase_allowance(client, "OVL", pairContractAddr, initialOVLLiquidity);

  // Then we actually provide liquidity

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
          "amount": initialUSTLiquidity
        },
        {
          "info": {
            "token": {
            "contract_addr": getContractFromName("OVL").contractAddress,
            "token_code_hash": getCodeHashFromName("OVL"),
            "viewing_key": "" // ignored, can be whatever
          }
          },
          "amount": initialOVLLiquidity
        }
      ]
    }
  }

  await client.execute(pairContractAddr,handleMsg);


  // Now we deploy the LP bond

  var prng_seed = uuid.v4();
  folder = "bond_depository"
  contract = "../" + folder + "/contract.wasm"

  InitMsg = {
    "dao" : DAOAddress,
    "OVL" :  {"address":getContractFromName("OVL").contractAddress,"code_hash":getCodeHashFromName("OVL")},
    "principle" :  {
      "token":{
        "address":liquidityTokenAddr,
        "code_hash":LPtokenHash
      },"pair":{
        "address":pairContractAddr,
        "code_hash":pairHash
      }
    },
    "bond_calculator":{"address":getContractFromName("bond_calculator").contractAddress,"code_hash":getCodeHashFromName("bond_calculator")},
    "treasury" : {"address":getContractFromName("treasury").contractAddress,"code_hash":getCodeHashFromName("treasury")},
    "prng_seed" : Buffer.from(prng_seed).toString('base64'),
    "symbol": "BSLP",
    "name": "OVL-UST LP bond"
  }
  
  let [LPBondContractCodeHash, LPBondContract] = await upload_contract(contract, client, InitMsg,"OVL-UST LP Bond");
  LPBondContract.principle = "OVL-UST-LP"
  console.log("Deployed UST-OVL LP bond ");
  contracts["OVL-UST-LP-bond"] = [LPBondContractCodeHash, LPBondContract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);


  await giveRole(client, LPBondContract.contractAddress, "LiquidityDepositor");

  //We set the bond terms
  handleMsg = {
        initialize_bond_terms : {
          "control_variable": daiBondBCV,
          "fee" : bondFee,
          "initial_debt" : initialBondDebt,
          "max_debt" : maxBondDebt,
          "max_payout" : maxBondPayout,
          "minimum_price" : "0",
          "maximum_price" : maxBondPrice,
          "vesting_term" : bondVestingLength,
      }
    }
  response = await client.execute(LPBondContract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));

  handleMsg = {
      set_staking : {
         "staking": {"address":getContractFromName("staking").contractAddress,"code_hash":getCodeHashFromName("staking")},
    }
  }
  response = await client.execute(LPBondContract.contractAddress,handleMsg);
  console.log("Response: ", response.transactionHash,"\n", base64ToJson(response.data));


  /*
  // We bond some UST 
  handleMsg = {
      send : {
      "recipient":sUSTBondContract.contractAddress,
      "recipient_code_hash":sUSTBondContractCodeHash,
      "amount":"1000000000",
      "msg" : Buffer.from(JSON.stringify({deposit :{max_price:'60000', depositor:await get_address()}})).toString('base64')
    }
  }

  response = await client.execute(sUSTcontract.contractAddress,handleMsg);
  console.log("Bonded some sUST");


  // We bond some SCRT
  handleMsg = {
      send : {
      "recipient":sSCRTBondContract.contractAddress,
      "recipient_code_hash":sSCRTBondContractCodeHash,
      "amount":"100000000",
      "msg" : Buffer.from(JSON.stringify({deposit : {max_price:'60000', depositor:await get_address()}})).toString('base64')
    }
  }
  response = await client.execute(sSCRTcontract.contractAddress,handleMsg);
  console.log("Bonded some sSCRT");

  */



  console.log('Successfully Deployed the contract');
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
