const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey
} = require("secretjs");

const fs = require("fs");

require('dotenv').config();


//DAO Address
const DAOAddress = "secretnothing189765"

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

function _arrayBufferToBase64( buffer ) {
    var binary = '';
    var bytes = new Uint8Array( buffer );
    var len = bytes.byteLength;
    for (var i = 0; i < len; i++) {
        binary += String.fromCharCode( bytes[ i ] );
    }
    return window.btoa( binary );
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
  const  [StakingcontractCodeHash, Stakingcontract] = await contracts["staking"];
  const [StakingWarmupContractCodeHash, StakingWarmupContract] = await contracts["staking-warmup"];
  const [sUSTBondContractCodeHash, sUSTBondContract] = contracts["sUST-bond"];
  const [sSCRTBondContractCodeHash, sSCRTBondContract] = contracts["sSCRT-bond"];
   

  let handleMsg,response;
  console.log(base64ToJson("eyJ0b2tlbl9pbmZvIjp7Im5hbWUiOiJGb25kQ29tbXVuIiwic3ltYm9sIjoiRkNUIiwiZGVjaW1hbHMiOjksInRvdGFsX3N1cHBseSI6IjYwMDAwMDAwMDAwMDAwMCJ9fQ=="));
  console.log(Buffer.from([0, 49, 188, 237, 2, 219, 0, 0]).readBigInt64BE());
  /*
  hyiuy
  queryMsg = {
    debt_decay:{block_height:1000}
  };
  */

  queryMsg = {
    bond_price_in_usd:{block_height:500}
  };

  queryMsg = {
    max_payout:{}
  };
  queryMsg = {
    gons_for_balance:{amount:"1"}
  };
  
  response = await client.queryContractSmart(sOHMcontract.contractAddress,queryMsg)
  console.log(response);

  queryMsg = {
    managing_addresses:{role:"ReserveManager"}
  };

  response = await client.queryContractSmart(treasurycontract.contractAddress,queryMsg)
  console.log(response);




  queryMsg = {
    index:{}
  };
  
  response = await client.queryContractSmart(sOHMcontract.contractAddress,queryMsg)
  console.log(response);

  queryMsg = {
    rebase_history:{page_size:10}
  };

  response = await client.queryContractSmart(sOHMcontract.contractAddress,queryMsg)
  console.log(response);

  let data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);


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
