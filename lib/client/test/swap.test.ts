import { Token } from "@solana/spl-token";
import {
  Account,
  Connection,
  PublicKey,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";

import { StableSwap } from "../src/stable-swap";
import { getDeploymentInfo, newAccountWithLamports, sleep } from "./helpers";
import {
  CLUSTER_URL,
  DEFAULT_TOKEN_DECIMALS,
  TOKEN_PROGRAM_ID,
  AMP_FACTOR,
  TWAP_OPEN,
  BOOTSTRAP_TIMEOUT,
  DEFAULT_FEES,
  DEFAULT_REWARDS,
  SWAP_DIRECTION_SELL_BASE,
  SWAP_DIRECTION_SELL_QUOTE,
} from "../src/constants";
import { sendAndConfirmTransaction } from "../src/util/send-and-confirm-transaction";

const INITIAL_TOKEN_A_AMOUNT = LAMPORTS_PER_SOL;
const INITIAL_TOKEN_B_AMOUNT = LAMPORTS_PER_SOL;

describe("e2e test for admin instructions", () => {
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
  let tokenPoolMint: Token;
  let userPoolAccount: PublicKey;
  // Tokens swapped
  let mintA: Token;
  let mintB: Token;
  let tokenAccountA: PublicKey;
  let tokenAccountB: PublicKey;
  // Reward token
  let deltafiTokenMint: Token;
  let deltafiTokenAccount: PublicKey;
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
    [authority, nonce] = await PublicKey.findProgramAddress(
      [stableSwapAccount.publicKey.toBuffer()],
      stableSwapProgramId
    );

    // creating pool mint
    tokenPoolMint = await Token.createMint(
      connection,
      payer,
      authority,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );

    // creating pool account
    userPoolAccount = await tokenPoolMint.createAccount(owner.publicKey);

    // creating token A
    mintA = await Token.createMint(
      connection,
      payer,
      owner.publicKey,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );
    // create token A account then mint to it
    adminFeeAccountA = await mintA.createAccount(owner.publicKey);
    tokenAccountA = await mintA.createAccount(authority);
    await mintA.mintTo(tokenAccountA, owner, [], INITIAL_TOKEN_A_AMOUNT);

    // creating token B
    mintB = await Token.createMint(
      connection,
      payer,
      owner.publicKey,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );
    // creating token B account then mint to it
    adminFeeAccountB = await mintB.createAccount(owner.publicKey);
    tokenAccountB = await mintB.createAccount(authority);
    await mintB.mintTo(tokenAccountB, owner, [], INITIAL_TOKEN_B_AMOUNT);

    // creating deltafi token
    deltafiTokenMint = await Token.createMint(
      connection,
      payer,
      owner.publicKey,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );
    deltafiTokenAccount = await deltafiTokenMint.createAccount(authority);
    // Sleep to make sure token accounts are created ...
    await sleep(500);

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
      tokenPoolMint.publicKey,
      userPoolAccount,
      deltafiTokenAccount,
      deltafiTokenMint.publicKey,
      stableSwapProgramId,
      TOKEN_PROGRAM_ID,
      nonce,
      AMP_FACTOR,
      0.5,
      10,
      TWAP_OPEN
    );

    done();
  }, BOOTSTRAP_TIMEOUT);

  it("bootstrapper's LP balance", async () => {
    const info = await tokenPoolMint.getAccountInfo(userPoolAccount);
    expect(info.amount.toNumber()).toEqual(
      INITIAL_TOKEN_A_AMOUNT + INITIAL_TOKEN_B_AMOUNT
    );
  });

  it("loadStableSwap", async () => {
    let fetchedStableSwap: StableSwap;

    fetchedStableSwap = await StableSwap.loadStableSwap(
      connection,
      stableSwapAccount.publicKey,
      stableSwapProgramId
    );

    expect(fetchedStableSwap.stableSwap).toEqual(stableSwapAccount.publicKey);
    expect(fetchedStableSwap.adminFeeAccountA).toEqual(adminFeeAccountA);
    expect(fetchedStableSwap.adminFeeAccountB).toEqual(adminFeeAccountB);
    expect(fetchedStableSwap.tokenAccountA).toEqual(tokenAccountA);
    expect(fetchedStableSwap.tokenAccountB).toEqual(tokenAccountB);
    expect(fetchedStableSwap.mintA).toEqual(mintA.publicKey);
    expect(fetchedStableSwap.mintB).toEqual(mintB.publicKey);
    expect(fetchedStableSwap.poolTokenMint).toEqual(tokenPoolMint.publicKey);
    expect(fetchedStableSwap.initialAmpFactor).toEqual(AMP_FACTOR);
    expect(fetchedStableSwap.fees.get()).toEqual(DEFAULT_FEES);
    expect(fetchedStableSwap.rewards.get()).toEqual(DEFAULT_REWARDS);
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
    info = await tokenPoolMint.getAccountInfo(userPoolAccount);
    expect(info.amount.toNumber()).toBe(4000000000);
  });

  it("withdraw", async () => {
    const withdrawalAmount = 100000;
    const poolMintInfo = await tokenPoolMint.getMintInfo();
    const oldSupply = poolMintInfo.supply.toNumber();
    const oldSwapTokenA = await mintA.getAccountInfo(tokenAccountA);
    const oldSwapTokenB = await mintB.getAccountInfo(tokenAccountB);
    const oldPoolToken = await tokenPoolMint.getAccountInfo(userPoolAccount);
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
    await tokenPoolMint.approve(
      userPoolAccount,
      authority,
      owner,
      [],
      withdrawalAmount
    );
    // Make sure all token accounts are created and approved
    await sleep(500);

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

    let info = await mintA.getAccountInfo(userAccountA);
    expect(info.amount.toNumber()).toBe(expectedWithdrawA);
    info = await mintB.getAccountInfo(userAccountB);
    expect(info.amount.toNumber()).toBe(expectedWithdrawB);
    info = await tokenPoolMint.getAccountInfo(userPoolAccount);
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

    // Swapping
    const txn = stableSwap.swap(
      userAccountA, // User source token account       | User source -> Swap source
      tokenAccountA, // Swap source token account
      tokenAccountB, // Swap destination token account | Swap dest -> User dest
      userAccountB, // User destination token account
      deltafiTokenAccount,
      deltafiTokenMint.publicKey,
      SWAP_AMOUNT_IN,
      0, // To avoid slippage errors,
      SWAP_DIRECTION_SELL_BASE
    );
    await sendAndConfirmTransaction("swap", connection, txn, payer);

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

    // Swapping;
    const txn = stableSwap.swap(
      userAccountB, // User source token account       | User source -> Swap source
      tokenAccountB, // Swap source token account
      tokenAccountA, // Swap destination token account | Swap dest -> User dest
      userAccountA, // User destination token account
      deltafiTokenAccount,
      deltafiTokenMint.publicKey,
      SWAP_AMOUNT_IN,
      0, // To avoid slippage errors
      SWAP_DIRECTION_SELL_QUOTE
    );
    await sendAndConfirmTransaction("swap", connection, txn, payer);

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
