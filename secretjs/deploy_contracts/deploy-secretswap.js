const {
  EnigmaUtils, Secp256k1Pen, SigningCosmWasmClient, pubkeyToAddress, encodeSecp256k1Pubkey,makeSignBytes
} = require("secretjs");

const uuid = require("uuid");

const fs = require("fs");

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

async function upload_code(contract_file,client,contractName="Default Contract"){

  // Upload the wasm of a simple contract
  const wasm = fs.readFileSync(contract_file);
  console.log('Uploading contract')
  const uploadReceipt = await client.upload(wasm, {});

  // Get the code ID from the receipt
  const codeId = uploadReceipt.codeId;
  console.log('codeId: ', codeId);

  const contractCodeHash = await client.restClient.getCodeHashByCodeId(codeId);

  return [codeId,contractCodeHash];

}

async function instantiate_contract(client, codeId, initMsg, contractName){
  const contract = await client.instantiate(codeId, initMsg, contractName + Math.ceil(Math.random()*10000));
  return contract;
}


async function upload_contract(contract_file, client, initMsg, contractName = "Default Contract"){

  let [codeId,contractCodeHash] = await upload_code(contract_file,client,contractName);
  // contract hash, useful for contract composition
  contract = await instantiate_contract(client, codeId, initMsg, contractName);
  // Create an instance of the Counter contract, providing a starting count
  return [contractCodeHash,contract]

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
  
  console.log("On démarre");
  // Create connection to DataHub Secret Network node
  client = await get_client();

  const accAddress = await get_address();

  // Pair code id: 13 on mainnet
  const pair = "../../SecretSwap/artifacts/secretswap_pair.wasm"
  //We start by uploading the pair contract
  let [pairId,pairHash] = await upload_code(pair,client,"PAIR");
  console.log("Pair sur le réseau !!");

  //Maybe we use the code only once
  const token = "../../SecretSwap/artifacts/secretswap_token.wasm"
  //We start by uploading the pair contract
  let [tokenId, tokenHash] = await upload_code(token,client,"PAIR");
  console.log("Token sur le réseau !!");

  const factory = "../../SecretSwap/artifacts/secretswap_factory.wasm"

  const tokenName = "factory";

  const FactoryMsg = {
    "pair_code_id": pairId,
    "token_code_id": tokenId,
    "pair_code_hash": pairHash,
    "token_code_hash": tokenHash,
    "prng_seed": Buffer.from("Something really random").toString('base64'),
  }

  const [FactoryContractCodeHash, FactoryContract] = await upload_contract(factory, client, FactoryMsg, tokenName);
  console.log('Successfully Deployed the contract');


  contracts["pair-factory"] = [{
    "pairCodeHash":pairHash,
    "LPTokenCodeHash":tokenHash
  }, {
    contractAddress:FactoryContract.contractAddress
  }];

  let data = JSON.stringify(contracts);
  fs.writeFileSync('contract_data.json', data);



}

main().then(resp => {
  console.log(resp);
}).catch(err => {
  console.log(err);
})
