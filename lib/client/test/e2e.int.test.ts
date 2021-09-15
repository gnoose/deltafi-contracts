import fs from "fs";

import { Token } from "@solana/spl-token";
import {
  Account,
  Connection,
  LAMPORTS_PER_SOL,
  PublicKey,
} from "@solana/web3.js";

import { StableSwap } from "../src";
import { DEFAULT_TOKEN_DECIMALS, TOKEN_PROGRAM_ID } from "../src/constants";
import {
  DEFAULT_FEE_DENOMINATOR,
  DEFAULT_FEE_NUMERATOR,
  Fees,
} from "../src/fees";
import { sendAndConfirmTransaction } from "../src/util/send-and-confirm-transaction";
import { getDeploymentInfo, newAccountWithLamports, sleep } from "./helpers";

// Cluster configs
const CLUSTER_URL = "http://localhost:8899";
const BOOTSTRAP_TIMEOUT = 300000;
// Pool configs
const AMP_FACTOR = 100;
const FEES: Fees = {
  adminTradeFeeNumerator: DEFAULT_FEE_NUMERATOR,
  adminTradeFeeDenominator: DEFAULT_FEE_DENOMINATOR,
  adminWithdrawFeeNumerator: DEFAULT_FEE_NUMERATOR,
  adminWithdrawFeeDenominator: DEFAULT_FEE_DENOMINATOR,
  tradeFeeNumerator: 1,
  tradeFeeDenominator: 4,
  withdrawFeeNumerator: DEFAULT_FEE_NUMERATOR,
  withdrawFeeDenominator: DEFAULT_FEE_DENOMINATOR,
};
// Initial amount in each swap token
const INITIAL_TOKEN_A_AMOUNT = LAMPORTS_PER_SOL;
const INITIAL_TOKEN_B_AMOUNT = LAMPORTS_PER_SOL;

// Initial amount in each LP token
const INITIAL_TOKEN_LP_AMOUNT = LAMPORTS_PER_SOL;

describe("e2e test for stable swap", () => {
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
  // Token pool
  let tokenPool: Token;
  let userPoolAccount: PublicKey;
  // Tokens swapped
  let mintA: Token;
  let mintB: Token;
  let tokenAccountA: PublicKey;
  let tokenAccountB: PublicKey;
  // Admin fee accounts
  let adminFeeAccountA: PublicKey;
  let adminFeeAccountB: PublicKey;
  // Stable swap
  let stableSwap: StableSwap;
  let stableSwapAccount: Account;
  let stableSwapProgramId: PublicKey;

  beforeAll(async (done) => {
    // Bootstrap Test Environment ...
    connection = new Connection(CLUSTER_URL, "single");
    payer = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);
    owner = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);

    stableSwapProgramId = getDeploymentInfo().stableSwapProgramId;
    stableSwapAccount = new Account();
    try {
      [authority, nonce] = await PublicKey.findProgramAddress(
        [stableSwapAccount.publicKey.toBuffer()],
        stableSwapProgramId
      );
    } catch (e) {
      throw new Error(e);
    }
    // creating pool mint
    try {
      tokenPool = await Token.createMint(
        connection,
        payer,
        authority,
        null,
        DEFAULT_TOKEN_DECIMALS,
        TOKEN_PROGRAM_ID
      );
    } catch (e) {
      throw new Error(e);
    }
    // creating pool account
    try {
      userPoolAccount = await tokenPool.createAccount(owner.publicKey);
    } catch (e) {
      throw new Error(e);
    }
    // creating token A
    try {
      mintA = await Token.createMint(
        connection,
        payer,
        owner.publicKey,
        null,
        DEFAULT_TOKEN_DECIMALS,
        TOKEN_PROGRAM_ID
      );
    } catch (e) {
      throw new Error(e);
    }
    // create token A account then mint to it
    try {
      adminFeeAccountA = await mintA.createAccount(owner.publicKey);
      tokenAccountA = await mintA.createAccount(authority);
      await mintA.mintTo(tokenAccountA, owner, [], INITIAL_TOKEN_A_AMOUNT);
    } catch (e) {
      throw new Error(e);
    }
    // creating token B
    try {
      mintB = await Token.createMint(
        connection,
        payer,
        owner.publicKey,
        null,
        DEFAULT_TOKEN_DECIMALS,
        TOKEN_PROGRAM_ID
      );
    } catch (e) {
      throw new Error(e);
    }
    // creating token B account then mint to it
    try {
      adminFeeAccountB = await mintB.createAccount(owner.publicKey);
      tokenAccountB = await mintB.createAccount(authority);
      await mintB.mintTo(tokenAccountB, owner, [], INITIAL_TOKEN_B_AMOUNT);
    } catch (e) {
      throw new Error(e);
    }
    // Sleep to make sure token accounts are created ...
    await sleep(500);

    // creating token swap
    try {
      stableSwap = await StableSwap.createStableSwap(
        connection,
        payer,
        stableSwapAccount,
        authority,
        owner.publicKey,
        adminFeeAccountA,
        adminFeeAccountB,
        mintA.publicKey,
        tokenAccountA,
        mintB.publicKey,
        tokenAccountB,
        tokenPool.publicKey,
        userPoolAccount,
        mintA.publicKey,
        mintB.publicKey,
        stableSwapProgramId,
        TOKEN_PROGRAM_ID,
        nonce,
        AMP_FACTOR,
        FEES
      );
    } catch (e) {
      throw new Error(e);
    }

    done();
  }, BOOTSTRAP_TIMEOUT);

  it("bootstrapper's LP balance", async () => {
    const info = await tokenPool.getAccountInfo(userPoolAccount);
    expect(info.amount.toNumber()).toEqual(
      INITIAL_TOKEN_A_AMOUNT + INITIAL_TOKEN_B_AMOUNT
    );
  });

  it("loadStableSwap", async () => {
    let fetchedStableSwap: StableSwap;
    try {
      fetchedStableSwap = await StableSwap.loadStableSwap(
        connection,
        stableSwapAccount.publicKey,
        stableSwapProgramId
      );
    } catch (e) {
      throw new Error(e);
    }

    expect(fetchedStableSwap.stableSwap).toEqual(stableSwapAccount.publicKey);
    expect(fetchedStableSwap.adminFeeAccountA).toEqual(adminFeeAccountA);
    expect(fetchedStableSwap.adminFeeAccountB).toEqual(adminFeeAccountB);
    expect(fetchedStableSwap.tokenAccountA).toEqual(tokenAccountA);
    expect(fetchedStableSwap.tokenAccountB).toEqual(tokenAccountB);
    expect(fetchedStableSwap.mintA).toEqual(mintA.publicKey);
    expect(fetchedStableSwap.mintB).toEqual(mintB.publicKey);
    expect(fetchedStableSwap.poolTokenMint).toEqual(tokenPool.publicKey);
    expect(fetchedStableSwap.initialAmpFactor).toEqual(AMP_FACTOR);
    expect(fetchedStableSwap.fees).toEqual(FEES);
  });

  it("getVirtualPrice", async () => {
    expect(await stableSwap.getVirtualPrice()).toBe(1);
  });

  it("deposit", async () => {
    const depositAmountA = LAMPORTS_PER_SOL;
    const depositAmountB = LAMPORTS_PER_SOL;
    // Creating depositor token a account
    const userAccountA = await mintA.createAccount(owner.publicKey);
    await mintA.mintTo(userAccountA, owner, [], depositAmountA);
    await mintA.approve(userAccountA, authority, owner, [], depositAmountA);
    // Creating depositor token b account
    const userAccountB = await mintB.createAccount(owner.publicKey);
    await mintB.mintTo(userAccountB, owner, [], depositAmountB);
    await mintB.approve(userAccountB, authority, owner, [], depositAmountB);
    // Make sure all token accounts are created and approved
    await sleep(500);

    try {
      // Depositing into swap
      const txn = stableSwap.deposit(
        userAccountA,
        userAccountB,
        userPoolAccount,
        depositAmountA,
        depositAmountB,
        0 // To avoid slippage errors
      );
      await sendAndConfirmTransaction("deposit", connection, txn, payer);
    } catch (e) {
      throw new Error(e);
    }

    let info = await mintA.getAccountInfo(userAccountA);
    expect(info.amount.toNumber()).toBe(0);
    info = await mintB.getAccountInfo(userAccountB);
    expect(info.amount.toNumber()).toBe(0);
    info = await mintA.getAccountInfo(tokenAccountA);
    expect(info.amount.toNumber()).toBe(
      INITIAL_TOKEN_A_AMOUNT + depositAmountA
    );
    info = await mintB.getAccountInfo(tokenAccountB);
    expect(info.amount.toNumber()).toBe(
      INITIAL_TOKEN_B_AMOUNT + depositAmountB
    );
    info = await tokenPool.getAccountInfo(userPoolAccount);
    expect(info.amount.toNumber()).toBe(4000000000);
  });

  it("withdraw", async () => {
    const withdrawalAmount = 100000;
    const poolMintInfo = await tokenPool.getMintInfo();
    const oldSupply = poolMintInfo.supply.toNumber();
    const oldSwapTokenA = await mintA.getAccountInfo(tokenAccountA);
    const oldSwapTokenB = await mintB.getAccountInfo(tokenAccountB);
    const oldPoolToken = await tokenPool.getAccountInfo(userPoolAccount);
    const expectedWithdrawA = Math.floor(
      (oldSwapTokenA.amount.toNumber() * withdrawalAmount) / oldSupply
    );
    const expectedWithdrawB = Math.floor(
      (oldSwapTokenB.amount.toNumber() * withdrawalAmount) / oldSupply
    );

    // Creating withdraw token A account
    const userAccountA = await mintA.createAccount(owner.publicKey);
    // Creating withdraw token B account
    const userAccountB = await mintB.createAccount(owner.publicKey);
    // Approving withdrawal from pool account
    await tokenPool.approve(
      userPoolAccount,
      authority,
      owner,
      [],
      withdrawalAmount
    );
    // Make sure all token accounts are created and approved
    await sleep(500);

    try {
      // Withdrawing pool tokens for A and B tokens
      const txn = await stableSwap.withdraw(
        userAccountA,
        userAccountB,
        userPoolAccount,
        withdrawalAmount,
        0, // To avoid slippage errors
        0 // To avoid spliiage errors
      );
      await sendAndConfirmTransaction("withdraw", connection, txn, payer);
    } catch (e) {
      throw new Error(e);
    }

    let info = await mintA.getAccountInfo(userAccountA);
    expect(info.amount.toNumber()).toBe(expectedWithdrawA);
    info = await mintB.getAccountInfo(userAccountB);
    expect(info.amount.toNumber()).toBe(expectedWithdrawB);
    info = await tokenPool.getAccountInfo(userPoolAccount);
    expect(info.amount.toNumber()).toBe(
      oldPoolToken.amount.toNumber() - withdrawalAmount
    );
    const newSwapTokenA = await mintA.getAccountInfo(tokenAccountA);
    expect(newSwapTokenA.amount.toNumber()).toBe(
      oldSwapTokenA.amount.toNumber() - expectedWithdrawA
    );
    const newSwapTokenB = await mintB.getAccountInfo(tokenAccountB);
    expect(newSwapTokenB.amount.toNumber()).toBe(
      oldSwapTokenB.amount.toNumber() - expectedWithdrawB
    );
  });

  it("swap A->B", async () => {
    // Swap accounts before swap
    const oldSwapTokenA = await mintA.getAccountInfo(tokenAccountA);
    const oldSwapTokenB = await mintB.getAccountInfo(tokenAccountB);
    // Amount passed to swap instruction
    const SWAP_AMOUNT_IN = 100000;
    // Creating swap token a account
    const userAccountA = await mintA.createAccount(owner.publicKey);
    await mintA.mintTo(userAccountA, owner, [], SWAP_AMOUNT_IN);
    await mintA.approve(userAccountA, authority, owner, [], SWAP_AMOUNT_IN);
    // Creating swap token b account
    const userAccountB = await mintB.createAccount(owner.publicKey);
    // Make sure all token accounts are created and approved
    await sleep(500);

    try {
      // Swapping
      const txn = stableSwap.swap(
        userAccountA, // User source token account       | User source -> Swap source
        tokenAccountA, // Swap source token account
        tokenAccountB, // Swap destination token account | Swap dest -> User dest
        userAccountB, // User destination token account
        SWAP_AMOUNT_IN,
        0 // To avoid slippage errors
      );
      await sendAndConfirmTransaction("swap", connection, txn, payer);
    } catch (e) {
      throw new Error(e);
    }
    // Make sure swap was complete
    await sleep(500);

    let info = await mintA.getAccountInfo(userAccountA);
    expect(info.amount.toNumber()).toBe(0);
    info = await mintA.getAccountInfo(tokenAccountA);
    expect(info.amount.toNumber()).toBe(
      oldSwapTokenA.amount.toNumber() + SWAP_AMOUNT_IN
    );
    const EXPECTED_AMOUNT_OUT = 75000; // EXPECTED_AMOUNT_OUT = SWAP_AMOUNT_IN * (1 - FEES)
    info = await mintB.getAccountInfo(userAccountB);
    expect(info.amount.toNumber()).toBe(EXPECTED_AMOUNT_OUT);
    info = await mintB.getAccountInfo(tokenAccountB);
    expect(info.amount.toNumber()).toBe(
      oldSwapTokenB.amount.toNumber() - EXPECTED_AMOUNT_OUT
    );
  });

  it("swap B->A", async () => {
    // Swap accounts before swap
    const oldSwapTokenA = await mintA.getAccountInfo(tokenAccountA);
    const oldSwapTokenB = await mintB.getAccountInfo(tokenAccountB);
    // Amount passed to swap instruction
    const SWAP_AMOUNT_IN = 100000;
    // Creating swap token b account
    const userAccountB = await mintB.createAccount(owner.publicKey);
    await mintB.mintTo(userAccountB, owner, [], SWAP_AMOUNT_IN);
    await mintB.approve(userAccountB, authority, owner, [], SWAP_AMOUNT_IN);
    // Creating swap token a account
    const userAccountA = await mintA.createAccount(owner.publicKey);
    // Make sure all token accounts are created and approved
    await sleep(500);

    try {
      // Swapping;
      const txn = stableSwap.swap(
        userAccountB, // User source token account       | User source -> Swap source
        tokenAccountB, // Swap source token account
        tokenAccountA, // Swap destination token account | Swap dest -> User dest
        userAccountA, // User destination token account
        SWAP_AMOUNT_IN,
        0 // To avoid slippage errors
      );
      await sendAndConfirmTransaction("swap", connection, txn, payer);
    } catch (e) {
      throw new Error(e);
    }

    // Make sure swap was complete
    await sleep(500);

    let info = await mintB.getAccountInfo(userAccountB);
    expect(info.amount.toNumber()).toBe(0);
    info = await mintB.getAccountInfo(tokenAccountB);
    expect(info.amount.toNumber()).toBe(
      oldSwapTokenB.amount.toNumber() + SWAP_AMOUNT_IN
    );
    const EXPECTED_AMOUNT_OUT = 75001; // EXPECTED_AMOUNT_OUT = SWAP_AMOUNT_IN * (1 - FEES)
    info = await mintA.getAccountInfo(userAccountA);
    expect(info.amount.toNumber()).toBe(EXPECTED_AMOUNT_OUT);
    info = await mintA.getAccountInfo(tokenAccountA);
    expect(info.amount.toNumber()).toBe(
      oldSwapTokenA.amount.toNumber() - EXPECTED_AMOUNT_OUT
    );
  });
});

describe("e2e test for liquidity mining", () => {
  // Cluster connection
  let connection: Connection;
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
  let mintPool: Token;
  let tokenAccountPool: PublicKey;
  // Admin fee accounts
  let adminFeeAccountPool: PublicKey;
  // Farm
  let farm: Farm;
  let farmAccount: Account;
  let farmBaseAccount: Account;
  let farmProgramId: PublicKey;

  beforeAll(async (done) => {
    connection = new Connection(CLUSTER_URL, "single");
    payer = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);
    owner = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);

    farmProgramId = getDeploymentInfo().stableSwapProgramId;
    farmAccount = new Account();
    farmBaseAccount = new Account();
    try {
      [authority, nonce] = await PublicKey.findProgramAddress(
        [farmAccount.publickKey.toBuffer()],
        farmProgramId,
      )
    } catch (e) {
      throw new Error(e);
    }
    // creating deltafi mint
    try {
      tokenDeltafi = await Token.createMint(
        connection,
        payer,
        authority,
        null,
        DEFAULT_TOKEN_DECIMALS,
        TOKEN_PROGRAM_ID,
      );
    } catch (e) {
      throw new Error(e);
    }
    // creating deltafi account
    try {
      userDeltafiAccount = await tokenDeltafi.createAccount(owner.publicKey);
    } catch (e) {
      throw new Error(e);
    }
    // creating token LP
    try {
      mintPool = await Token.createMint(
        connection,
        payer,
        owner.publicKey,
        null,
        DEFAULT_TOKEN_DECIMALS,
        TOKEN_PROGRAM_ID,
      );
    } catch (e) {
      throw new Error(e);
    }
    // create token pool account then mint to it
    try {
      adminFeeAccountPool = await mintPool.createAccount(owner.publicKey);
      tokenAccountPool = await mintPool.createAccount(authority);
      await mintPool.mintTo(tokenAccountPool, owner, [], INITIAL_TOKEN_LP_AMOUNT)
    } catch (e) {
      throw new Error(e);
    }
    // Sleep to make sure token accounts are created ...
    await sleep(500);

    // creating farm
    try {
      farm = await Farm.createFarm(
        connection,
        payer,
        farmAccount,
        authority,
        owner.publicKey,
        adminFeeAccountPool,
        mintPool.publicKey,
        tokenAccountPool,
        tokenDeltafi.publicKey,
        userDeltafiAccount,
        farmProgramId,
        TOKEN_PROGRAM_ID,
        nonce,
        FEES
      );
    } catch (e) {
      throw new Error(e);
    }

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
    try {
      fetchedFarm = await Farm.loadFarm(
        connection,
        farmAccount.publicKey,
        farmBaseAccount.publicKey,
        farmProgramId,
      );
    } catch (e) {
      throw new Error(e);
    }

    expect(fetchedFarm.farm).toEqual(farmAccount.publicKey);
    expect(fetchedFarm.adminFeeAccountPool).toEqual(adminFeeAccountPool);
    // ...
  });

  // not sure
  // it("getVirtualPrice", async () => {
  //   expect(await stableSwap.getVirtualPrice()).toBe(1);
  // });

  it("deposit", async() => {
    const depositeAmountPool = LAMPORTS_PER_SOL;
    // creating depostor token lp account
    const userAccountPool = await mintPool.createAccount(owner.publicKey);
    await mintPool.mintTo(userAccountPool, owner, [], depositeAmountPool);
    await mintPool.approve(userAccountPool, authority, owner, [], depositeAmountPool);
    // Make sure all token accounts are created and approved
    await sleep(500);
    try {
      // Depositing into swap
      const txn = farm.deposit(
        userAccountPool,
        userDeltafiAccount,
        depositeAmountPool,
        0, // To avoid slippage errors
      );
      await sendAndConfirmTransaction("deposit", connection, txn, payer);
    } catch (e) {
      throw new Error(e);
    }

    let info = await mintPool.getAccountInfo(userAccountPool);
    expect(info.amount.toNumber()).toBe(0);
    info = await mintPool.getAccountInfo(tokenAccountPool);
    expect(info.amount.toNumber()).toBe(
      INITIAL_TOKEN_LP_AMOUNT + depositeAmountPool
    );

    // change the time with future then check reward using printPendingDetafi
    // ...
  });

  it ("withdraw", async() => {
    const withdrawalAmount = 100000;
    const poolMintInfo = await tokenAccountPool.getMintInfo();
    const oldSupply = poolMintInfo.supply.toNumber();
    const oldFarmPool = await mintPool.getAccountInfo(tokenAccountPool);
    const oldPoolToken = await tokenAccountPool.getAccountInfo(userPoolAcount);
    const expectedWithdrawPool = Math.floor(
      (oldFarmPool.amount.toNumber * withdrawalAmount) / oldSupply
    );

    // creating withdraw token LP account
    const userAccountPool = await mintPool.createAccount(owner.publicKey);
    // approving withdrawal from pool and deltafi account
    await tokenAccountPool.approve(
      userPoolAccount,
      authority,
      [],
      withdrawalAmount
    );
    // make sure all token accounts are created and approved
    await sleep(500);

    try {
      const txn = await farm.withdraw(
        userAccountPool,
        userDeltafiAccount,
        withdrawalAmount,
        0, // To avoid slippage errors
        0, // To avoid slippage errors
      );
      await sendAndConfirmTransaction("withdraw", connection, txn, payer);
    } catch(e) {
      throw new Error(e);
    }

    let info = await mintPool.getAccountInfo(userAccountPool);
    expect(info.amount.toNumber()).toBe(expectedWithdrawPool);
    // ...

    // change the time with future then check reward using printPendingDetafi
    // ...    
  });

  it ("emergencyWithdraw", async() => {
    const withdrawalAmount = 100000;
    const poolMintInfo = await tokenAccountPool.getMintInfo();
    const oldSupply = poolMintInfo.supply.toNumber();
    const oldFarmPool = await mintPool.getAccountInfo(tokenAccountPool);
    const oldPoolToken = await tokenAccountPool.getAccountInfo(userPoolAcount);
    const expectedWithdrawPool = Math.floor(
      (oldFarmPool.amount.toNumber * withdrawalAmount) / oldSupply
    );

    // creating withdraw token LP account
    const userAccountPool = await mintPool.createAccount(owner.publicKey);
    // approving withdrawal from pool and deltafi account
    await tokenAccountPool.approve(
      userPoolAccount,
      authority,
      [],
      withdrawalAmount
    );
    // make sure all token accounts are created and approved
    await sleep(500);

    try {
      const txn = await farm.emergencyWithdraw(
        userAccountPool,
        userDeltafiAccount,
        withdrawalAmount,
        0, // To avoid slippage errors
        0, // To avoid slippage errors
      );
      await sendAndConfirmTransaction("emergencyWithdraw", connection, txn, payer);
    } catch(e) {
      throw new Error(e);
    }

    let info = await mintPool.getAccountInfo(userAccountPool);
    expect(info.amount.toNumber()).toBe(expectedWithdrawPool);
    // ...

    // change the time with future then check reward is zero using printPendingDetafi
    // ...    
  });
});
