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

const initialOHMLiquidity = "100000000000";

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

  let client = await get_client();


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
            "contract_addr": getContractFromName("OHM").contractAddress,
            "token_code_hash": getCodeHashFromName("OHM"),
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

  console.log("Created the OHM-UST pair");
  contracts["OHM-UST-LP"] = {};
  contracts["OHM-UST-LP"] = [
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
    address: contracts["OHM-UST-LP"][1].contractAddress,
    code_hash: contracts["OHM-UST-LP"][0],
    pair_address: contracts["OHM-UST-LP"][1].pair[1].contractAddress,
    pair_code_hash: contracts["OHM-UST-LP"][1].pair[0],
  }

  let calculator = {
    code_hash:CalculatorcontractCodeHash,
    address:Calculatorcontract.contractAddress,
  }

  await giveTokenRole(client, liquidity_token_contract, "LiquidityToken", calculator);


  //Then we have to provide some liquidity for the contract to work

  // First we set an allowance for the pair to spend the tokens

  await increase_allowance(client, "sUST", pairContractAddr, initialUSTLiquidity);
  await increase_allowance(client, "OHM", pairContractAddr, initialOHMLiquidity);

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
            "contract_addr": getContractFromName("OHM").contractAddress,
            "token_code_hash": getCodeHashFromName("OHM"),
            "viewing_key": "" // ignored, can be whatever
          }
          },
          "amount": initialOHMLiquidity
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
    "ohm" :  {"address":getContractFromName("OHM").contractAddress,"code_hash":getCodeHashFromName("OHM")},
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
    "name": "OHM-UST LP bond"
  }
  
  let [LPBondContractCodeHash, LPBondContract] = await upload_contract(contract, client, InitMsg,"OHM-UST LP Bond");
  LPBondContract.principle = "OHM-UST-LP"
  console.log("Deployed UST-OHM LP bond ");
  contracts["OHM-UST-LP-bond"] = [LPBondContractCodeHash, LPBondContract];
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
