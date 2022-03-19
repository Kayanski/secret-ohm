const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const uuid = require("uuid");

const fs = require("fs");

const { fromUtf8 } = require("@iov/encoding");
require('dotenv').config();

function jsonToBase64(json){
  return Buffer.from(JSON.stringify(json)).toString('base64')
}
function base64ToJson(base64String){
  return JSON.parse(Buffer.from(base64String,'base64').toString('utf8'));
}

let contracts = require("../../contract_data.json");

// Initial staking index
const initialIndex = '7675210820';

// First block epoch occurs
const firstEpochBlock = '8961000';

// What epoch will be first epoch
const firstEpochNumber = '338';

// How many blocks are in each epoch
const epochLengthInBlocks = '2200';

// Initial reward rate for epoch
const initialRewardRate = '3000';

// Initial mint for Frax and DAI (10,000,000)
const initialMint = '10000000000000000000000000';

// DAI bond BCV
const daiBondBCV = '369';

// Frax bond BCV
const fraxBondBCV = '690';

// Bond vesting length in blocks. 33110 ~ 5 days
const bondVestingLength = '33110';

// Min bond price
const minBondPrice = '50000';

// Max bond payout
const maxBondPayout = '50'

// DAO fee for bond
const bondFee = '10000';

// Max debt bond can take on
const maxBondDebt = '1000000000000000';

// Initial Bond debt
const intialBondDebt = '0'

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

  let [sSCRTcontractCodeHash, sSCRTcontract] = await contracts["sSCRT"];
  let [sUSTcontractCodeHash, sUSTcontract] = await contracts["sUST"];
  let [OHMcontractCodeHash, OHMcontract] = await contracts["OHM"];
  let [sOHMcontractCodeHash, sOHMcontract] = await contracts["sOHM"];
  let [treasurycontractCodeHash, treasurycontract]  = await contracts["treasury"];
  let [DistributorcontractCodeHash, Distributorcontract] = await contracts["staking_distributor"];
  let  [StakingcontractCodeHash, Stakingcontract] = await contracts["staking"];
  let [StakingWarmupContractCodeHash, StakingWarmupContract] = await contracts["staking-warmup"];
  let [sUSTBondContractCodeHash, sUSTBondContract] = contracts["sUST-bond"];
  let [sSCRTBondContractCodeHash, sSCRTBondContract] = contracts["sSCRT-bond"];

  // We upload the bond calculator contract

  let folder = "bond_calculator"
  let contract = "../" + folder + "/contract.wasm"

  let InitMsg = {
      "ohm": {"address":OHMcontract.contractAddress,"code_hash":OHMcontractCodeHash},
  }
  
  const [CalculatorcontractCodeHash, Calculatorcontract] = await upload_contract(contract, client, InitMsg, "bond_calculator");

  console.log("Deployed Bond Calculator");

  contracts["bond_calculator"] = [CalculatorcontractCodeHash, Calculatorcontract];
  data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);

  //END

  console.log('Successful upload');
}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
