use crate::{Acc, Int};

// TODO: RPC DTOs: block, header, tx

// TODO: tracing DTOs (events, touches, etc)

#[derive(Clone, Copy, Debug, Default)]
pub struct Head {
    pub number: u64,
    pub hash: Int,
    pub gas_price: Int, // TODO: move to tx, update opcode impl
    pub gas_limit: Int,
    pub coinbase: Acc,
    pub timestamp: Int,
    pub base_fee: Int,
    pub blob_base_fee: Int,
    pub chain_id: Int,
    pub blobhash: Int,
    pub prevrandao: Int,
}

pub struct Tx {
    // TODO
}

/*

{
  "baseFeePerGas": "0x246edf3",
  "blobGasUsed": "0x40000",
  "difficulty": "0x0",
  "excessBlobGas": "0xa2a720e",
  "extraData": "0x4275696c6465724e6574202842656176657229",
  "gasLimit": "0x3938700",
  "gasUsed": "0x1a4ed69",
  "hash": "0x6c4c6ea047f0ca4bcb0b0fbbcc2e50061d8dd24ba1ab8a22a6d3583a497c3c82",
  "logsBloom": "0xfffffef97ffdf7dfb75fbbfeffffdfffffbeefffffbbffff6ffffdffbffffaf9df7fff7f7fff6ffbffffdfdffbffffff76fffffcbf7ffbfffffeefffffff7df7bffffffdefffff8fdfdf7f7ceffdffff7ff77affe7ef5ff5fbfff57ff7eeadfffffffabfff7bdffbbfbffcfffdefffeffefffffffff5fffefdffdffbffffdfdffbffe7fffffffffdffebf7e37fdfe977f7fbdfefdffddffffffffff5f97ffffdfbffdf7fffffbff9feff77afffefffeef77ffff7fff6fffeffd77ffffeefefdb7bffffff7f7ffdfbff7ffffffffefdffdffddffffffefffffffefdfbf7fdffef7fffff3bdf7fff6cefffdfffdf7ffffeffbfbffffdafffdfbf7efdbeffff7fff",
  "miner": "0xdadb0d80178819f2319190d340ce9a924f783711",
  "mixHash": "0xf824cbc0bffaa4862fffafa96c2cce90898dda8b97fc3eaa6a85ac38075991e5",
  "nonce": "0x0000000000000000",
  "number": "0x177d354",
  "parentBeaconBlockRoot": "0xd9b957562f8477151f61eddc77bcad7451ef62eab8f4fe470402073a794e8bba",
  "parentHash": "0x63ce16602b2afe4089fa8e0a2c16be1a2dea1c4d30b65bd717d9398ddd5fba23",
  "receiptsRoot": "0x52fc280ab6b1db61781dae4cc5f81b7ddd484eae78320bdd8161d5b2fe8e7719",
  "requestsHash": "0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
  "size": "0x2a8a9",
  "stateRoot": "0x41ebb402f36d0ce15799e98d1aca83bbb5d4b08d1b6572e73b388e4ece192916",
  "timestamp": "0x69b09d6b",
  "transactions": [
    "0x0a57a0922b5abd25c79e49bbc1bf478ec923f8c580834fc0fd450ab6ad8b9630"
  ],
  "transactionsRoot": "0x3b889eb9fa0713d2e1d9e90725d45fd04ec4a550a7a390639c243c5f3b2c6215",
  "uncles": [],
  "withdrawals": [
    {
      "address": "0xb9d7934878b5fb9610b3fe8a5e441e8fad7e293f",
      "amount": "0x344741",
      "index": "0x73e482b",
      "validatorIndex": "0x21e64c"
    }
  ],
  "withdrawalsRoot": "0x76abcbef0846a5bd065d0207581dacc3cf8df4ec2ba0a8e4d4fbbe97864b82ee"
}

{
    "accessList": [],
    "blockHash": "0x1f5b98ca47015bbfa9bff1b6282fd7753a0b79053876677b22ac178df53d76c7",
    "blockNumber": "0x177e232",
    "blockTimestamp": "0x69b1509f",
    "chainId": "0x1",
    "from": "0xdadb0d80178819f2319190d340ce9a924f783711",
    "gas": "0x5208",
    "gasPrice": "0x4b2f976",
    "hash": "0xb478179e8e7f2fadfd42103007e9dfb5ea882667b1dd6e65142ef6b2a54b5668",
    "input": "0x",
    "maxFeePerGas": "0x4b2f976",
    "maxPriorityFeePerGas": "0x0",
    "nonce": "0x22792c",
    "r": "0x51a8855fd9176012c328891ee0027959fd6480ef7ca09a2aac2e287c1c311566",
    "s": "0x3e4b0c0cf854f42a0999c2ce6a9dc88b16f1b5f4e201dd61a36ed3a769719e6c",
    "to": "0x08ec37e2eb451ab6fb29fc14d215b0aeef170040",
    "transactionIndex": "0x201",
    "type": "0x2",
    "v": "0x1",
    "value": "0x1f92fa85e3ea2a",
    "yParity": "0x1"
}

*/
