const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const fs = require("fs");

const { fromBase64 } = require("@iov/encoding");
require('dotenv').config();

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

const getAPY = async(client,tokenContract) => {
  console.log('Querying the APY...');
  let response = await client.queryContractSmart(tokenContract.contractAddress, { "get_count": {}});
  console.log(response);
  return response["apy"];
}

const getTotalValueDeposited = async(client,tokenContract) => {
  console.log('Querying the Value Deposited...');
  let response = await client.queryContractSmart(tokenContract.contractAddress, { "contract_balance": {}});
  console.log(response);
  return response["amount"];
}

const getIndex = async(client,tokenContract) => {
  console.log('Querying the Index...');
  let response = await client.queryContractSmart(tokenContract.contractAddress, { "index": {}});
  console.log(response);
  return response["index"];
}

const main = async () => {


  // Create connection to DataHub Secret Network node
  const client = await get_client();
  const accAddress = await get_address();

  let rawdata = fs.readFileSync('contract_data.json');
  let contracts = JSON.parse(rawdata);

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
  
  console.log(base64ToJson("eyJ2aWV3aW5nX2tleV9lcnJvciI6eyJtc2ciOiJXcm9uZyB2aWV3aW5nIGtleSBmb3IgdGhpcyBhZGRyZXNzIG9yIHZpZXdpbmcga2V5IG5vdCBzZXQifX0="));

  await getIndex(client,Stakingcontract);
  await getTotalValueDeposited(client,Stakingcontract);

}



main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
