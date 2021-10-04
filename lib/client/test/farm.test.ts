import { Token } from "@solana/spl-token";
import {
  Account,
  Connection,
  PublicKey,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";

import {
  CLUSTER_URL,
  DEFAULT_TOKEN_DECIMALS,
  TOKEN_PROGRAM_ID,
  BOOTSTRAP_TIMEOUT,
  DEFAULT_FEES,
} from "../src/constants";
import { Farm } from "../src";
import { getDeploymentInfo, newAccountWithLamports, sleep } from "./helpers";
import { sendAndConfirmTransaction } from "../src/util/send-and-confirm-transaction";

// Initial amount in each LP token
const INITIAL_TOKEN_LP_AMOUNT = LAMPORTS_PER_SOL;

describe("e2e test for liquidity mining", () => {
  // Cluster connection
  let connection: Connection;
  // Fee payer
  let payer: Account;
  // authority of the token and accounts
  let authority: PublicKey;
  // nonce used to generate the authority public key
  let nonce: number;
  // owner of the user accounts
  let owner: Account;
  // Token deltafi
  let tokenDeltafi: Token;
  let userDeltafiAccount: PublicKey;
  // Farming tokens
  let tokenPool: Token;
  let tokenAccountPool: PublicKey;
  // Admin fee accounts
  let adminFeeAccountDeltafi: PublicKey;
  // Farm
  let farm: Farm;
  let farmAccount: Account;
  let farmBaseAccount: Account;
  let farmProgramId: PublicKey;
  let userFarmingAccount: Account;

  beforeAll(async (done) => {
    connection = new Connection(CLUSTER_URL, "single");
    payer = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);
    owner = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);

    farmProgramId = getDeploymentInfo().stableSwapProgramId;
    farmAccount = new Account();
    farmBaseAccount = new Account();
    userFarmingAccount = new Account();

    [authority, nonce] = await PublicKey.findProgramAddress(
      [farmAccount.publicKey.toBuffer()],
      farmProgramId
    );

    // creating deltafi mint
    tokenDeltafi = await Token.createMint(
      connection,
      payer,
      authority,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );

    // creating deltafi account
    userDeltafiAccount = await tokenDeltafi.createAccount(owner.publicKey);

    // creating token LP
    tokenPool = await Token.createMint(
      connection,
      payer,
      owner.publicKey,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );

    // create token pool account then mint to it
    adminFeeAccountDeltafi = await tokenPool.createAccount(owner.publicKey);
    tokenAccountPool = await tokenPool.createAccount(authority);
    await tokenPool.mintTo(
      tokenAccountPool,
      owner,
      [],
      INITIAL_TOKEN_LP_AMOUNT
    );

    // Sleep to make sure token accounts are created ...
    await sleep(500);

    // creating farm
    farm = await Farm.createFarm(
      connection,
      payer,
      farmBaseAccount,
      farmAccount,
      authority,
      owner.publicKey,
      adminFeeAccountDeltafi,
      tokenPool.publicKey,
      tokenAccountPool,
      tokenDeltafi.publicKey,
      userDeltafiAccount,
      tokenPool.publicKey,
      farmProgramId,
      TOKEN_PROGRAM_ID,
      nonce,
      DEFAULT_FEES
    );

    // emulate the action when clicking, so create and register userFarmmingAccount
    userFarmingAccount = new Account();
    const txn = farm.enableUser(userFarmingAccount.publicKey, owner.publicKey);
    await sendAndConfirmTransaction("enableUser", connection, txn, payer);

    await sleep(500);
    farm.userFarming = userFarmingAccount.publicKey;

    done();
  }, BOOTSTRAP_TIMEOUT);

  // not sure
  // it("bootstrapper's LP balance", async () => {
  //   const info = await tokenPool.getAccountInfo(userPoolAccount);
  //   expect(info.amount.toNumber()).toEqual(
  //     INITIAL_TOKEN_A_AMOUNT + INITIAL_TOKEN_B_AMOUNT
  //   );
  // });

  it("loadFarm", async () => {
    let fetchedFarm: Farm;
    fetchedFarm = await Farm.loadFarm(
      connection,
      farmAccount.publicKey,
      farmProgramId
    );

    expect(fetchedFarm.farm).toEqual(farmAccount.publicKey);
    expect(fetchedFarm.adminFeeAccountDeltafi).toEqual(adminFeeAccountDeltafi);
    expect(fetchedFarm.tokenAccountPool).toEqual(tokenAccountPool);
    expect(fetchedFarm.mintPool).toEqual(tokenPool);
    expect(fetchedFarm.deltafiTokenMint).toEqual(tokenDeltafi);
  });

  // not sure and I guess maybe this is the real price of deltafi
  // it("getVirtualPrice", async () => {
  //   expect(await stableSwap.getVirtualPrice()).toBe(1);
  // });

  it("deposit", async () => {
    const depositeAmountPool = LAMPORTS_PER_SOL;
    // creating depostor token lp account
    const userAccountPool = await tokenPool.createAccount(owner.publicKey);
    await tokenPool.mintTo(userAccountPool, owner, [], depositeAmountPool);
    await tokenPool.approve(
      userAccountPool,
      authority,
      owner,
      [],
      depositeAmountPool
    );
    // Make sure all token accounts are created and approved
    await sleep(500);
    // Depositing into farm
    const txn = farm.deposit(
      userAccountPool,
      userFarmingAccount.publicKey,
      userDeltafiAccount,
      nonce,
      depositeAmountPool,
      0 // To avoid slippage errors
    );
    await sendAndConfirmTransaction("deposit", connection, txn, payer);

    let info = await tokenPool.getAccountInfo(userAccountPool);
    expect(info.amount.toNumber()).toBe(0);
    info = await tokenPool.getAccountInfo(tokenAccountPool);
    expect(info.amount.toNumber()).toBe(
      INITIAL_TOKEN_LP_AMOUNT + depositeAmountPool
    );

    // change the time with future then check reward using printPendingDetafi
    // ...
  });

  it("withdraw", async () => {
    const withdrawalAmount = 100000;
    // creating depostor token lp account
    const userAccountPool = await tokenPool.createAccount(owner.publicKey);
    const poolMintInfo = await tokenPool.getMintInfo();
    const oldSupply = poolMintInfo.supply.toNumber();
    const oldFarmPool = await tokenPool.getAccountInfo(tokenAccountPool);
    const oldPoolToken = await tokenPool.getAccountInfo(userAccountPool);
    const expectedWithdrawPool = Math.floor(
      (oldFarmPool.amount.toNumber() * withdrawalAmount) / oldSupply
    );

    // approving withdrawal from pool and deltafi account
    await tokenPool.approve(
      userAccountPool,
      authority,
      owner,
      [],
      withdrawalAmount
    );
    // make sure all token accounts are created and approved
    await sleep(500);

    const txn = await farm.withdraw(
      userAccountPool,
      userDeltafiAccount,
      withdrawalAmount,
      0, // To avoid slippage errors
      0 // To avoid slippage errors
    );
    await sendAndConfirmTransaction("withdraw", connection, txn, payer);

    let info = await tokenPool.getAccountInfo(userAccountPool);
    expect(info.amount.toNumber()).toBe(expectedWithdrawPool);
    // !! confirm reward
    // ...

    // change the time with future then check reward using printPendingDetafi
    // ...
  });

  it("emergencyWithdraw", async () => {
    const withdrawalAmount = 100000;
    const userAccountPool = await tokenPool.createAccount(owner.publicKey);
    const poolMintInfo = await tokenPool.getMintInfo();
    const oldSupply = poolMintInfo.supply.toNumber();
    const oldFarmPool = await tokenPool.getAccountInfo(tokenAccountPool);
    const oldPoolToken = await tokenPool.getAccountInfo(userAccountPool);
    const expectedWithdrawPool = Math.floor(
      (oldFarmPool.amount.toNumber() * withdrawalAmount) / oldSupply
    );

    // approving withdrawal from pool and deltafi account
    await tokenPool.approve(
      userAccountPool,
      authority,
      owner,
      [],
      withdrawalAmount
    );
    // make sure all token accounts are created and approved
    await sleep(500);

    const txn = await farm.emergencyWithdraw(
      userAccountPool,
      userDeltafiAccount
    );
    await sendAndConfirmTransaction(
      "emergencyWithdraw",
      connection,
      txn,
      payer
    );

    let info = await tokenPool.getAccountInfo(userAccountPool);
    expect(info.amount.toNumber()).toBe(expectedWithdrawPool);

    // change the time with future then check reward is zero using printPendingDetafi
    // ...
  });
});
