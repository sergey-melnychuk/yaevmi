// SPDX-License-Identifier: MIT
pragma solidity ^0.8.34;

// Flash loan: borrow ETH within a single transaction, repay before it ends.

interface IBorrower {
    function onFlash(uint amount) external payable;
}

contract Flash {
    uint transient private locked;

    receive() external payable {}

    function loan(address borrower, uint amount) external {
        require(locked == 0, "reentrant");
        locked = 1;
        uint floor = address(this).balance - amount;
        IBorrower(borrower).onFlash{value: amount}(amount);
        require(address(this).balance >= floor + amount, "repay");
        locked = 0;
    }
}

// TODO: example arbitrage bot?
