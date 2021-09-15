import type { Connection } from "@solana/web3.js";
import {
  Account,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import { instructions, TOKEN_PROGRAM_ID } from 'src';
import { DEFAULT_FEES, DEFAULT_FEE_DENOMINATOR, Fees } from './fees';
import { depositInstruction } from './instructions';
import * as layout from "./layout";
import { loadAccount } from './util/account';
import { sendAndConfirmTransaction } from './util/send-and-confirm-transaction';

export class Farm {
  /**
   * @private
   */
  connection: Connection;

  /**
   * Program Identifier for the Farm
   */
  farmProgramId: PublicKey;
  /**
    * Program Identifier for the Token program
    */
  tokenProgramId: PublicKey;
  /**
   * The public key identifying this farm
   */
  farm: PublicKey;

  /**
   * The public key for the deltafi token mint
   */
  deltafiTokenMint: PublicKey;

  /**
   * Authority
   */
  authority: PublicKey;

  /**
   * Admin account
   */
  adminAccount: PublicKey;

  /**
   * Admin Fee Account for LP
   */
  adminFeeAccountPool: PublicKey;

  /**
   * Public key for the LP token account
   */
  tokenAccountPool: PublicKey;

  /**
   * Public key for the mint of pool token account
   */
  mintPool: PublicKey;

  /**
   * Fees
   */
  fees: Fees;

  /**
   * Constructor for new Farm client object
   * @param connection
   * @param farm
   * @param farmProgramId
   * @param deltafiTokenMint
   * @param authority
   * @param adminAccount
   * @param adminFeeAccountPool
   * @param tokenAccountPool
   * @param mintPool
   * @param fees
   */
  constructor(
    connection: Connection,
    farm: Farm,
    farmProgramId: PublicKey,
    tokenProgramId: PublicKey,
    deltafiTokenMint: PublicKey,
    authority: PublicKey,
    adminAccount: PublicKey,
    adminFeeAccountPool: PublicKey,
    tokenAccountPool: PublicKey,
    mintPool: PublicKey,
    fees: Fees = DEFAULT_FEES,
  ) {
    this.connection = connection;
    this.farm = farm;
    this.farmProgramId = farmProgramId;
    this.tokenProgramId = tokenProgramId;
    this.deltafiTokenMint = deltafiTokenMint;
    this.authority = authority;
    this.adminAccount = adminAccount;
    this.adminFeeAccountPool = adminFeeAccountPool;
    this.tokenAccountPool = tokenAccountPool;
    this.mintPool = mintPool;
    this.fees = fees;
  }

  /**
   * Get the minimum balance for the token swap account to be rent exempt
   *
   * @return Number of lamports required
   */
  static async getMinBalanceRentForExemptFarm(
    connection: Connection
  ): Promise<number> {
    return await connection.getMinimumBalanceForRentExemption(
      layout.FarmLayout.span
    );
  }

  static async getMinBalanceRentForExemptFarmBase(
    connection: Connection
  ): Promise<number> {
    return await connection.getMinimumBalanceForRentExemption(
      layout.FarmBaseLayout.span
    );
  }

  /**
   * Load an onchain Farm program
   * @param connection The connection to use
   * @param address The public key of the account to load
   * @param programId Address of the onchain Farm Program
   * @param payer Pays for the transaction
   */

  static async loadFarm(
    connection: Connection,
    address: PublicKey,
    programId: PublicKey,
  ): Promise<Farm> {
    const data = await loadAccount(connection, address, programId);
    const farmData = layout.FarmLayout.decode(data);
    if (!farmData.isInitialized) {
      throw new Error(`Invalid farm state`);
    }

    const [authority] = await PublicKey.findProgramAddress(
      [address.toBuffer()],
      programId
    );

    const adminAccount = new PublicKey(farmData.adminAccount);
    const adminFeeAccountPool = new PublicKey(farmData.adminFeeAccountPool);
    const tokenAccountPool = new PublicKey(farmData.tokenAccountPool);
    const deltafiTokenMint = new PublicKey(farmData.deltafiTokenMint);
    const mintPool = new PublicKey(farmData.mintPool);
    const tokenProgramId = TOKEN_PROGRAM_ID;
    const fees = {
      adminTradeFeeNumerator: farmData.adminTradeFeeNumerator as number,
      adminTradeFeeDenominator: farmData.adminTradeFeeDenominator as number,
      adminWithdrawFeeNumerator: farmData.adminWithdrawFeeNumerator as number,
      adminWithdrawFeeDenominator: farmData.adminWithdrawFeeDenominator as number,
      tradeFeeNumerator: farmData.tradeFeeNumerator as number,
      tradeFeeDenominator: farmData.tradeFeeDenominator as number,
      withdrawFeeNumerator: farmData.withdrawFeeNumerator as number,
      withdrawFeeDenominator: farmData.withdrawFeeDenominator as number,      
    };

    return new Farm(
      connection,
      address,
      programId,
      tokenProgramId,
      deltafiTokenMint,
      authority,
      adminAccount,
      adminFeeAccountPool,
      tokenAccountPool,
      mintPool,
      fees,
    );
  }

  /**
   * Constructor for new Farm client object
   * @param connection
   * @param payer
   * @param farmAccount
   * @param authority
   * @param adminAccount
   * @param adminFeeAccountPool
   * @param tokenAccountPool
   * @param deltafiTokenMint
   * @param deltafiTokenAccount
   * @param mintPool
   * @param farmProgramId
   * @param tokenProgramId
   * @param nonce
   * @param fees
   */
  static async createFarm(
    connection: Connection,
    payer: Account,
    farmAccount: Account,
    authority: PublicKey,
    adminAccount: PublicKey,
    adminFeeAccountPool: PublicKey,
    tokenMintPool: PublicKey,
    tokenAccountPool: PublicKey,
    deltafiTokenMint: PublicKey,
    deltafiTokenAccount: PublicKey,
    mintPool: PublicKey,
    farmProgramId: PublicKey,
    tokenProgramId: PublicKey,
    nonce: number,
    fees: Fees = DEFAULT_FEES
  ): Promise<Farm> {
    // allocate memory for the account
    const balanceNeeded = await Farm.getMinBalanceRentForExemptFarm(
      connection
    );
    const transaction = new Transaction().add(
      SystemProgram.createAccount({
        fromPubKey: payer.publicKey,
        newAccountPubKey: farmAccount.publicKey,
        lamports: balanceNeeded,
        space: layout.FarmLayout.span,
        programId: farmProgramId,
      })
    );

    const instruction = instructions.createInitFarmInstruction(
      farmAccount,
      authority,
      adminAccount,
      adminFeeAccountPool,
      tokenMintPool,
      tokenAccountPool,
      deltafiTokenMint,
      deltafiTokenAccount,
      farmProgramId,
      tokenProgramId,
      nonce,
      fees,
    );

    await sendAndConfirmTransaction(
      "createAccount and InitializeFarm",
      connection,
      transaction,
      payer,
      farmAccount,
    );

    return new Farm(
      connection,
      farmAccount.publicKey,
      farmProgramId,
      tokenProgramId,
      deltafiTokenMint,
      authority,
      adminAccount,
      adminFeeAccountPool,
      tokenAccountPool,
      mintPool,
      fees,
    );
  }

  /**
   * Deposit LP tokens into the farm
   * @param userAccountPool
   * @param deltafiTokenAccount
   * @param tokenAmountPool
   * @param minimumDeltafiTokenAmount
   */
    deposit(
    userAccountPool: PublicKey,
    deltafiTokenAccount: PublicKey,
    tokenAmountPool: number,
    minimumDeltafiTokenAmount: number
  ): Transaction {
    return new Transaction().add(
      instructions.farmDepositInstruction(
        this.farm,
        this.authority,
        userAccountPool,
        this.tokenAccountPool,
        this.deltafiTokenMint,
        deltafiTokenAccount,
        this.farmProgramId,
        this.tokenProgramId,
        tokenAmountPool,
        minimumDeltafiTokenAmount,
      )
    );
  }

  /**
   * Withdraw LP tokens from the farm
   * @param userAccountPool
   * @param poolAccount
   * @param poolTokenAmount
   * @param minimumTokenPool
   */
  withdraw(
    userAccountPool: PublicKey,
    poolAccount: PublicKey,
    poolTokenAmount: number,
    minimumTokenPool: number,
  ): Transaction {
    return new Transaction().add(
      instructions.farmWithdrawInstruction(
        this.farm,
        this.authority,
        this.deltafiTokenMint,
        poolAccount,
        this.tokenAccountPool,
        userAccountPool,
        this.adminFeeAccountPool,
        this.farmProgramId,
        this.tokenProgramId,
        poolTokenAmount,
        minimumTokenPool,
      )
    );
  }

  /**
   * Withdraw LP tokens from the farm
   * @param userAccountPool
   * @param poolAccount
   * @param poolTokenAmount
   * @param minimumTokenPool
   */
   emergencyWithdraw(
    userAccountPool: PublicKey,
    poolAccount: PublicKey,
  ): Transaction {
    return new Transaction().add(
      instructions.farmEmergencyWithdrawInstruction(
        this.farm,
        this.authority,
        this.deltafiTokenMint,
        poolAccount,
        this.tokenAccountPool,
        userAccountPool,
        this.adminFeeAccountPool,
        this.farmProgramId,
        this.tokenProgramId,
        0,
      )
    );
  }}