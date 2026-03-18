use yaevmi_base::{Acc, Int, acc};
use yaevmi_core::{Call, aux::create_address, state::Account};
use yaevmi_misc::buf::Buf;

/// Deploy Hello, then read message() — should return "it works!".
#[tokio::test]
async fn test_hello_deploy_and_read() -> eyre::Result<()> {
    let combined = super::load()?;
    let hello_src = &combined.contracts["sol/hello.sol:Hello"];

    let deployer = acc("0xAA");
    let hello_addr = create_address(&deployer, 0);

    // Deploy Hello
    let deploy = Call {
        by: deployer,
        to: Acc::ZERO,
        gas: 500_000,
        eth: Int::ZERO,
        data: hello_src.bin.clone(),
    };
    let env = vec![(
        deployer,
        Account {
            value: super::ethers(1),
            nonce: Int::ZERO,
            code: (Buf::default(), Int::ZERO),
        },
        vec![],
    )];

    let head = super::head();
    let tx0 = super::tx(0);
    let exp = crate::revm::run(deploy.clone(), head.clone(), env.clone(), tx0.clone()).await?;
    let res = super::run(deploy, head.clone(), env, tx0).await?;
    pretty_assertions::assert_eq!(res, exp, "deploy Hello must match revm");

    let deployed: Acc = res.0.to();
    assert_eq!(deployed, hello_addr, "Hello deployed at expected address");

    // Read message()
    let env1 = res.4;
    let read_call = Call {
        by: deployer,
        to: hello_addr,
        gas: 500_000,
        eth: Int::ZERO,
        data: Buf(super::selector("message()")),
    };
    let tx1 = super::tx(1);
    let exp1 = crate::revm::run(read_call.clone(), head.clone(), env1.clone(), tx1.clone()).await?;
    let res1 = super::run(read_call, head, env1, tx1).await?;
    pretty_assertions::assert_eq!(res1, exp1, "message() must match revm");

    Ok(())
}

/// Deploy Owner with value, then test get(), set(), odd().
#[tokio::test]
async fn test_owner_get_set_odd() -> eyre::Result<()> {
    let combined = super::load()?;
    let owner_src = &combined.contracts["sol/hello.sol:Owner"];

    let deployer = acc("0xAA");
    let stranger = acc("0xBB");

    // Deploy Owner(value_=7)
    let mut deploy_data = owner_src.bin.0.clone();
    deploy_data.extend_from_slice(Int::from(7u64).as_ref());

    let deploy = Call {
        by: deployer,
        to: Acc::ZERO,
        gas: 500_000,
        eth: Int::ZERO,
        data: Buf(deploy_data),
    };
    let env = vec![
        (
            deployer,
            Account {
                value: super::ethers(1),
                nonce: Int::ZERO,
                code: (Buf::default(), Int::ZERO),
            },
            vec![],
        ),
        (
            stranger,
            Account {
                value: super::ethers(1),
                nonce: Int::ZERO,
                code: (Buf::default(), Int::ZERO),
            },
            vec![],
        ),
    ];

    let head = super::head();
    let tx0 = super::tx(0);
    let exp0 = crate::revm::run(deploy.clone(), head.clone(), env.clone(), tx0.clone()).await?;
    let res0 = super::run(deploy, head.clone(), env, tx0).await?;
    pretty_assertions::assert_eq!(res0, exp0, "deploy Owner must match revm");

    let owner_addr: Acc = res0.0.to();
    let env1 = res0.4;

    // get() should return 7
    let get_call = Call {
        by: deployer,
        to: owner_addr,
        gas: 500_000,
        eth: Int::ZERO,
        data: Buf(super::selector("get()")),
    };
    let tx1 = super::tx(1);
    let exp1 = crate::revm::run(get_call.clone(), head.clone(), env1.clone(), tx1.clone()).await?;
    let res1 = super::run(get_call, head.clone(), env1.clone(), tx1).await?;
    pretty_assertions::assert_eq!(res1, exp1, "get() must match revm");

    // odd() should return true (7 is odd)
    let odd_call = Call {
        by: deployer,
        to: owner_addr,
        gas: 500_000,
        eth: Int::ZERO,
        data: Buf(super::selector("odd()")),
    };
    let tx2 = super::tx(1);
    let exp2 = crate::revm::run(odd_call.clone(), head.clone(), env1.clone(), tx2.clone()).await?;
    let res2 = super::run(odd_call, head.clone(), env1.clone(), tx2).await?;
    pretty_assertions::assert_eq!(res2, exp2, "odd() must match revm");

    // set(10) by owner should succeed
    let mut set_data = super::selector("set(uint256)");
    set_data.extend_from_slice(Int::from(10u64).as_ref());
    let set_call = Call {
        by: deployer,
        to: owner_addr,
        gas: 500_000,
        eth: Int::ZERO,
        data: Buf(set_data.clone()),
    };
    let tx3 = super::tx(1);
    let exp3 = crate::revm::run(set_call.clone(), head.clone(), env1.clone(), tx3.clone()).await?;
    let res3 = super::run(set_call, head.clone(), env1.clone(), tx3).await?;
    pretty_assertions::assert_eq!(res3, exp3, "set(10) by owner must match revm");

    // set(10) by stranger should revert
    let stranger_set = Call {
        by: stranger,
        to: owner_addr,
        gas: 500_000,
        eth: Int::ZERO,
        data: Buf(set_data),
    };
    let tx4 = super::tx(0);
    let exp4 = crate::revm::run(
        stranger_set.clone(),
        head.clone(),
        env1.clone(),
        tx4.clone(),
    )
    .await?;
    let res4 = super::run(stranger_set, head, env1, tx4).await?;
    pretty_assertions::assert_eq!(res4, exp4, "set() by stranger must match revm");

    Ok(())
}
