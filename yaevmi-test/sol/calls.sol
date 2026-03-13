// SPDX-License-Identifier: MIT
pragma solidity ^0.8.34;

contract Caller {
    Callee private callee;
    uint private timestamp;

    constructor() {
        timestamp = block.timestamp;
    }

    function create() external returns (address) {
        Callee created = new Callee();
        callee = created;
        return address(created);
    }

    function call(uint a, uint b) external view returns (uint) {
        uint ret = callee.call(a, b);
        return ret;
    }

    function callback(uint a, uint b) external view returns (uint) {
        if (timestamp + a < block.timestamp) {
            return a;
        } else {
            return b;
        }
    }
}

contract Callee {
    address private caller;

    constructor() {
        caller = msg.sender;
    }

    function call(uint a, uint b) external view returns (uint) {
        uint ret = Caller(caller).callback(a, b);
        return ret;
    }
}
