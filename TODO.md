$ ./target/release/replay 24716734
...
===
STEP MISMATCH:
YEVM: Step {
    pc: 8902,
    op: 250,
    name: "STATICCALL",
    data: None,
    gas: 3025,
    stack: 10,
    memory: 1728,
    debug: [
        "CALL:to=0x52b494fc67457c55b865ec1a0718ecaa00ad36ba,gas=190603",
        "depth=3",
    ],
}
REVM: Step {
    pc: 8902,
    op: 250,
    name: "STATICCALL",
    data: None,
    gas: 2984,
    stack: 10,
    memory: 1728,
    debug: [
        "cost=190744",
        "depth=3",
    ],
}
(skip: 610719)
0xf3186abef207d772c6a8daa50eaa8ebf087bcb75931290a59508fd97a23dd6b7 [type:2]: FAIL: [32/127, 801ms/353ms, fetches:8/448ms]
 ok: have 0 want 1
gas: have 89336 want 192850
...
0xf31ece55ab8b7c75bcaeb60a6f332872bfe10ebb66f482f131f064d2edcde24b [type:2]: FAIL: [36/127, 701ms/479ms, fetches:4/222ms]
gas: have 231442 want 252745
125/127 OK
---
$ ./target/release/replay 24720304
...
===
STEP MISMATCH:
YEVM: Step {
    pc: 2187,
    op: 80,
    name: "POP",
    data: None,
    gas: 105824,
    stack: 22,
    memory: 352,
    debug: [
        "cost=2",
        "depth=1",
    ],
}
REVM: Step {
    pc: 2223,
    op: 91,
    name: "JUMPDEST",
    data: None,
    gas: 105825,
    stack: 23,
    memory: 352,
    debug: [
        "cost=1",
        "depth=1",
    ],
}
(skip: 978)
0x88cb2e978aa7ce3d8e78650c2ada5bb7be492f8705b9bd899e19364c172b7797 [type:2]: FAIL: [1/373, 458ms/458ms, fetches:0/0ms]
gas: have 52518 want 115430
0xc87521a6e4216ebbd2ed4849dd88a99280fd2819553b80ea6c55fe8eb79dc487 [type:2]: FAIL: [2/373, 103ms/103ms, fetches:0/0ms]
gas: have 52506 want 115418
...
353/373 OK
---
$ ./target/release/replay 24720420
0xd952ce74bc75177ccd0485bede5813e334afe6248f74819586219355131a22df [type:2]: OK [1/307, 173159 gas, 1618ms/849ms, fetches:19/769ms]
===
STEP MISMATCH:
YEVM: Step {
    pc: 1119,
    op: 241,
    name: "CALL",
    data: None,
    gas: 2325,
    stack: 13,
    memory: 1184,
    debug: [
        "CALL:to=0xcad97616f91872c02ba3553db315db4015cbe850,gas=146489",
        "depth=2",
    ],
}
REVM: Step {
    pc: 1119,
    op: 241,
    name: "CALL",
    data: None,
    gas: 2284,
    stack: 13,
    memory: 1184,
    debug: [
        "cost=149130",
        "depth=2",
    ],
}
(skip: 12516)
0xc7842eee33fca792ca567f964bb0cffa7093410b2928673122b8bd3bd007e256 [type:2]: FAIL: [2/307, 520ms/389ms, fetches:2/131ms]
 ok: have 0 want 1
gas: have 43689 want 158913
...