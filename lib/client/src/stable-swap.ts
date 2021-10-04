import BN from "bn.js";
import type { Connection } from "@solana/web3.js";
import {
  Account,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { AccountLayout, MintLayout } from "@solana/spl-token";

import {
  TOKEN_PROGRAM_ID,
  ZERO_TS,
  DEFAULT_FEES,
  DEFAULT_REWARDS,
} from "./constants";
import { Fees, Rewards } from "./struct";
import * as instructions from "./instructions";
import * as layout from "./layout";
import { loadAccount } from "./util/account";
import { computeD } from "./util/calculator";
import { sendAndConfirmTransaction } from "./util/send-and-confirm-transaction";
import { NumberU64 } from "./util/u64";

/**
 * A program to exchange tokens against a pool of liquidity
 */
export class StableSwap {
  /**
   * @private
   */
  connection: Connection;

  /**
   * Program Identifier for the Swap program
   */
  swapProgramId: PublicKey;

  /**
   * Program Identifier for the Token program
   */
  tokenProgramId: PublicKey;

  /**
   * The public key identifying this swap program
   */
  stableSwap: PublicKey;

  /**
   * The public key for the liquidity pool token mint
   */
  poolTokenMint: PublicKey;

  /**
   * Authority
   */
  authority: PublicKey;

  /**
   * Admin account
   */
  adminAccount: PublicKey;

  /**
   * Admin fee account for token A
   */
  adminFeeAccountA: PublicKey;

  /**
   * Admin fee account for token B
   */
  adminFeeAccountB: PublicKey;

  /**
   * The public key for the first token account of the trading pair
   */
  tokenAccountA: PublicKey;

  /**
   * The public key for the second token account of the trading pair
   */
  tokenAccountB: PublicKey;

  /**
   * The public key for the mint of the first token account of the trading pair
   */
  mintA: PublicKey;

  /**
   * The public key for the mint of the second token account of the trading pair
   */
  mintB: PublicKey;

  /**
   * The public key for the deltafi token account
   */
  deltafiTokenAccount: PublicKey;

  /**
   * The public key for the mint of the deltafi token when trading
   */
  deltafiTokenMiint: PublicKey;

  /**
   * Initial amplification coefficient (A)
   */
  initialAmpFactor: number;

  /**
   * Target amplification coefficient (A)
   */
  targetAmpFactor: number;

  /**
   * Ramp A start timestamp
   */
  startRampTimestamp: number;

  /**
   * Ramp A start timestamp
   */
  stopRampTimestamp: number;

  /**
   * Fees
   */
  fees: Fees;

  /**
   * Rewards
   */
  rewards: Rewards;

  /**
   * Slope value - 0 < k < 1
   */
  k: NumberU64;

  /**
   * Mid price
   */
  i: NumberU64;

  /**
   * twap open flag
   */
  isOpenTwap: NumberU64;

  /**
   * Constructor for new StableSwap client object
   * @param connection
   * @param stableSwap
   * @param swapProgramId
   * @param tokenProgramId
   * @param poolTokenMint
   * @param authority
   * @param adminAccount
   * @param adminFeeAccountA
   * @param adminFeeAccountB
   * @param tokenAccountA
   * @param tokenAccountB
   * @param mintA
   * @param mintB
   * @param deltafiTokenAccount
   * @param deltafiTokenMint
   * @param initialAmpFactor
   * @param targetAmpFactor
   * @param startRampTimestamp
   * @param stopRampTimeStamp
   * @param fees
   * @param rewards
   * @param k
   * @param i
   * @param isOpenTwap
   */
  constructor(
    connection: Connection,
    stableSwap: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    poolTokenMint: PublicKey,
    authority: PublicKey,
    adminAccount: PublicKey,
    adminFeeAccountA: PublicKey,
    adminFeeAccountB: PublicKey,
    tokenAccountA: PublicKey,
    tokenAccountB: PublicKey,
    mintA: PublicKey,
    mintB: PublicKey,
    deltafiTokenAccount: PublicKey,
    deltafiTokenMint: PublicKey,
    initialAmpFactor: number,
    targetAmpFactor: number,
    startRampTimestamp: number,
    stopRampTimeStamp: number,
    fees: Fees = new Fees(DEFAULT_FEES),
    rewards: Rewards = new Rewards(DEFAULT_REWARDS),
    k: number | NumberU64,
    i: number | NumberU64,
    isOpenTwap: number | NumberU64
  ) {
    this.connection = connection;
    this.stableSwap = stableSwap;
    this.swapProgramId = swapProgramId;
    this.tokenProgramId = tokenProgramId;
    this.poolTokenMint = poolTokenMint;
    this.authority = authority;
    this.adminAccount = adminAccount;
    this.adminFeeAccountA = adminFeeAccountA;
    this.adminFeeAccountB = adminFeeAccountB;
    this.tokenAccountA = tokenAccountA;
    this.tokenAccountB = tokenAccountB;
    this.mintA = mintA;
    this.mintB = mintB;
    this.deltafiTokenAccount = deltafiTokenAccount;
    this.deltafiTokenMiint = deltafiTokenMint;
    this.initialAmpFactor = initialAmpFactor;
    this.targetAmpFactor = targetAmpFactor;
    this.startRampTimestamp = startRampTimestamp;
    this.stopRampTimestamp = stopRampTimeStamp;
    this.fees = fees;
    this.rewards = rewards;
    this.k = new NumberU64(k);
    this.i = new NumberU64(i);
    this.isOpenTwap = new NumberU64(isOpenTwap);
  }

  /**
   * Get the minimum balance for the token swap account to be rent exempt
   *
   * @return Number of lamports required
   */
  static async getMinBalanceRentForExemptStableSwap(
    connection: Connection
  ): Promise<number> {
    return await connection.getMinimumBalanceForRentExemption(
      layout.StableSwapLayout.span
    );
  }

  /**
   * Load an onchain StableSwap program
   * @param connection The connection to use
   * @param address The public key of the account to load
   * @param programId Address of the onchain StableSwap program
   * @param payer Pays for the transaction
   */
  static async loadStableSwap(
    connection: Connection,
    address: PublicKey,
    programId: PublicKey
  ): Promise<StableSwap> {
    const data = await loadAccount(connection, address, programId);
    const stableSwapData = layout.StableSwapLayout.decode(data);
    if (!stableSwapData.isInitialized) {
      throw new Error(`Invalid token swap state`);
    }

    const [authority] = await PublicKey.findProgramAddress(
      [address.toBuffer()],
      programId
    );
    const adminAccount = new PublicKey(stableSwapData.adminAccount);
    const adminFeeAccountA = new PublicKey(stableSwapData.adminFeeAccountA);
    const adminFeeAccountB = new PublicKey(stableSwapData.adminFeeAccountB);
    const tokenAccountA = new PublicKey(stableSwapData.tokenAccountA);
    const tokenAccountB = new PublicKey(stableSwapData.tokenAccountB);
    const poolTokenMint = new PublicKey(stableSwapData.tokenPool);
    const mintA = new PublicKey(stableSwapData.mintA);
    const mintB = new PublicKey(stableSwapData.mintB);
    const deltafiTokenAccount = new PublicKey(
      stableSwapData.deltafiTokenAccount
    );
    const dletafiTokenMint = new PublicKey(stableSwapData.deltafiTokenMint);
    const tokenProgramId = TOKEN_PROGRAM_ID;
    const initialAmpFactor = stableSwapData.initialAmpFactor;
    const targetAmpFactor = stableSwapData.targetAmpFactor;
    const startRampTimestamp = stableSwapData.startRampTs;
    const stopRampTimeStamp = stableSwapData.stopRampTs;
    const fees = Fees.fromBuffer(stableSwapData.fees);
    const rewards = Rewards.fromBuffer(stableSwapData.rewards);
    const k = NumberU64.fromBuffer(stableSwapData.k);
    const i = NumberU64.fromBuffer(stableSwapData.i);
    const isOpenTwap = NumberU64.fromBuffer(stableSwapData.isOpenTwap);

    return new StableSwap(
      connection,
      address,
      programId,
      tokenProgramId,
      poolTokenMint,
      authority,
      adminAccount,
      adminFeeAccountA,
      adminFeeAccountB,
      tokenAccountA,
      tokenAccountB,
      mintA,
      mintB,
      deltafiTokenAccount,
      dletafiTokenMint,
      initialAmpFactor,
      targetAmpFactor,
      startRampTimestamp,
      stopRampTimeStamp,
      new Fees(fees),
      new Rewards(rewards),
      k,
      i,
      isOpenTwap
    );
  }

  /**
   * Constructor for new StableSwap client object
   * @param connection
   * @param payer
   * @param stableSwapAccount
   * @param authority
   * @param adminAccount
   * @param adminFeeAccountA
   * @param adminFeeAccountB
   * @param tokenAccountA
   * @param tokenAccountB
   * @param poolTokenMint
   * @param poolTokenAccount
   * @param mintA
   * @param mintB
   * @param deltafiTokenAccount
   * @param deltafiTokenMint
   * @param swapProgramId
   * @param tokenProgramId
   * @param nonce
   * @param ampFactor
   * @param fees
   * @param rewards
   * @param k
   * @param i
   * @param isOpenTwap
   */
  static async createStableSwap(
    connection: Connection,
    payer: Account,
    stableSwapAccount: Account,
    authority: PublicKey,
    adminAccount: PublicKey,
    adminFeeAccountA: PublicKey,
    adminFeeAccountB: PublicKey,
    tokenMintA: PublicKey,
    tokenAccountA: PublicKey,
    tokenMintB: PublicKey,
    tokenAccountB: PublicKey,
    poolTokenMint: PublicKey,
    poolTokenAccount: PublicKey,
    deltafiTokenAccount: PublicKey,
    deltafiTokenMint: PublicKey,
    swapProgramId: PublicKey,
    tokenProgramId: PublicKey,
    nonce: number,
    ampFactor: number,
    k: number | NumberU64,
    i: number | NumberU64,
    isOpenTwap: number | NumberU64,
    fees: Fees = new Fees(DEFAULT_FEES),
    rewards: Rewards = new Rewards(DEFAULT_REWARDS)
  ): Promise<StableSwap> {
    // Allocate memory for the account
    const balanceNeeded = await StableSwap.getMinBalanceRentForExemptStableSwap(
      connection
    );
    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: stableSwapAccount.publicKey,
        lamports: balanceNeeded,
        space: layout.StableSwapLayout.span,
        programId: swapProgramId,
      })
    );

    const instruction = instructions.createInitSwapInstruction(
      stableSwapAccount.publicKey,
      authority,
      adminAccount,
      adminFeeAccountA,
      adminFeeAccountB,
      tokenMintA,
      tokenAccountA,
      tokenMintB,
      tokenAccountB,
      poolTokenMint,
      poolTokenAccount,
      deltafiTokenAccount,
      deltafiTokenMint,
      tokenProgramId,
      nonce,
      new NumberU64(ampFactor),
      fees,
      rewards,
      new NumberU64(k),
      new NumberU64(i),
      new NumberU64(isOpenTwap),
      swapProgramId
    );
    transaction.add(instruction);

    await sendAndConfirmTransaction(
      "createAccount and InitializeSwap",
      connection,
      transaction,
      payer,
      stableSwapAccount
    );

    return new StableSwap(
      connection,
      stableSwapAccount.publicKey,
      swapProgramId,
      tokenProgramId,
      poolTokenMint,
      authority,
      adminAccount,
      adminFeeAccountA,
      adminFeeAccountB,
      tokenAccountA,
      tokenAccountB,
      tokenMintA,
      tokenMintB,
      deltafiTokenAccount,
      deltafiTokenMint,
      ampFactor,
      ampFactor,
      ZERO_TS,
      ZERO_TS,
      fees,
      rewards,
      k,
      i,
      isOpenTwap
    );
  }

  /**
   * Get the virtual price of the pool.
   */
  async getVirtualPrice(): Promise<number> {
    let tokenAData;
    let tokenBData;
    let poolMintData;

    tokenAData = await loadAccount(
      this.connection,
      this.tokenAccountA,
      this.tokenProgramId
    );
    tokenBData = await loadAccount(
      this.connection,
      this.tokenAccountB,
      this.tokenProgramId
    );
    poolMintData = await loadAccount(
      this.connection,
      this.poolTokenMint,
      this.tokenProgramId
    );

    const tokenA = AccountLayout.decode(tokenAData);
    const tokenB = AccountLayout.decode(tokenBData);
    const amountA = NumberU64.fromBuffer(tokenA.amount);
    const amountB = NumberU64.fromBuffer(tokenB.amount);
    const D = computeD(new BN(this.initialAmpFactor), amountA, amountB);

    const poolMint = MintLayout.decode(poolMintData);
    const poolSupply = NumberU64.fromBuffer(poolMint.supply);

    return D.toNumber() / poolSupply.toNumber();
  }

  /**
   * Swap token A for token B
   * @param userSource
   * @param poolSource
   * @param poolDestination
   * @param userDestination
   * @param rewardDestination
   * @param rewardMint
   * @param amountIn
   * @param minimumAmountOut
   * @param swapDirection
   */
  swap(
    userSource: PublicKey,
    poolSource: PublicKey,
    poolDestination: PublicKey,
    userDestination: PublicKey,
    rewardDestination: PublicKey,
    rewardMint: PublicKey,
    amountIn: number,
    minimumAmountOut: number,
    swapDirection: number
  ): Transaction {
    const adminDestination =
      poolDestination === this.tokenAccountA
        ? this.adminFeeAccountA
        : this.adminFeeAccountB;
    return new Transaction().add(
      instructions.createSwapInstruction(
        this.stableSwap,
        this.authority,
        userSource,
        poolSource,
        poolDestination,
        userDestination,
        adminDestination,
        rewardDestination,
        rewardMint,
        this.tokenProgramId,
        new NumberU64(amountIn),
        new NumberU64(minimumAmountOut),
        new NumberU64(swapDirection),
        this.swapProgramId
      )
    );
  }

  /**
   * Deposit tokens into the pool
   * @param userAccountA
   * @param userAccountB
   * @param poolAccount
   * @param tokenAmountA
   * @param tokenAmountB
   * @param minimumPoolTokenAmount
   */
  deposit(
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    poolTokenAccount: PublicKey,
    tokenAmountA: number,
    tokenAmountB: number,
    minimumPoolTokenAmount: number
  ): Transaction {
    return new Transaction().add(
      instructions.createDepositInstruction(
        this.stableSwap,
        this.authority,
        userAccountA,
        userAccountB,
        this.tokenAccountA,
        this.tokenAccountB,
        this.poolTokenMint,
        poolTokenAccount,
        this.tokenProgramId,
        new NumberU64(tokenAmountA),
        new NumberU64(tokenAmountB),
        new NumberU64(minimumPoolTokenAmount),
        this.swapProgramId
      )
    );
  }

  /**
   * Withdraw tokens from the pool
   * @param userAccountA
   * @param userAccountB
   * @param poolAccount
   * @param poolTokenAmount
   * @param minimumTokenA
   * @param minimumTokenB
   */
  withdraw(
    userAccountA: PublicKey,
    userAccountB: PublicKey,
    poolAccount: PublicKey,
    poolTokenAmount: number,
    minimumTokenA: number,
    minimumTokenB: number
  ): Transaction {
    return new Transaction().add(
      instructions.createWithdrawInstruction(
        this.stableSwap,
        this.authority,
        this.poolTokenMint,
        poolAccount,
        this.tokenAccountA,
        this.tokenAccountB,
        userAccountA,
        userAccountB,
        this.adminFeeAccountA,
        this.adminFeeAccountB,
        this.tokenProgramId,
        new NumberU64(poolTokenAmount),
        new NumberU64(minimumTokenA),
        new NumberU64(minimumTokenB),
        this.swapProgramId
      )
    );
  }
}
