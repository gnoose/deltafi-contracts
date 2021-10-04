import * as BufferLayout from "buffer-layout";
import {
  PublicKey,
  TransactionInstruction,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";

import { NumberU64 } from "../util/u64";
import { Fees } from "../struct/fees";
import { Rewards } from "../struct";
import { FeesLayout } from "../layout";

export enum SwapInstruction {
  Initialize = 0,
  Swap,
  Deposit,
  Withdraw,
  WithdrawOne,
}

export const createInitSwapInstruction = (
  tokenSwap: PublicKey,
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
  rewardTokenAccount: PublicKey,
  rewardMint: PublicKey,
  tokenProgramId: PublicKey,
  nonce: number,
  ampFactor: NumberU64,
  fees: Fees,
  rewards: Rewards,
  k: NumberU64,
  i: NumberU64,
  isOpenTwap: NumberU64,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: tokenSwap, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: false, isWritable: false },
    { pubkey: adminFeeAccountA, isSigner: false, isWritable: false },
    { pubkey: adminFeeAccountB, isSigner: false, isWritable: false },
    { pubkey: tokenMintA, isSigner: false, isWritable: false },
    { pubkey: tokenAccountA, isSigner: false, isWritable: false },
    { pubkey: tokenMintB, isSigner: false, isWritable: false },
    { pubkey: tokenAccountB, isSigner: false, isWritable: false },
    { pubkey: poolTokenMint, isSigner: false, isWritable: true },
    { pubkey: poolTokenAccount, isSigner: false, isWritable: true },
    { pubkey: rewardMint, isSigner: false, isWritable: false },
    { pubkey: rewardTokenAccount, isSigner: false, isWritable: false },
    { pubkey: tokenProgramId, isSigner: false, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    BufferLayout.u8("nonce"),
    BufferLayout.nu64("ampFactor"),
    FeesLayout("fees"),
    BufferLayout.nu64("k"),
    BufferLayout.nu64("i"),
    BufferLayout.nu64("isOpenTwap"),
  ]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: SwapInstruction.Initialize,
      nonce,
      ampFactor: ampFactor.toBuffer(),
      fees: fees.toBuffer(),
      rewards: rewards.toBuffer(),
      k: k.toBuffer(),
      i: i.toBuffer(),
      isOpenTwap: isOpenTwap.toBuffer(),
    },
    data
  );

  return new TransactionInstruction({
    keys,
    programId,
    data,
  });
};

export const createSwapInstruction = (
  tokenSwap: PublicKey,
  authority: PublicKey,
  userSource: PublicKey,
  poolSource: PublicKey,
  poolDestination: PublicKey,
  userDestination: PublicKey,
  adminDestination: PublicKey,
  rewardDestination: PublicKey,
  rewardMint: PublicKey,
  tokenProgramId: PublicKey,
  amountIn: NumberU64,
  minimumAmountOut: NumberU64,
  swapDirection: NumberU64,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: tokenSwap, isSigner: false, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: userSource, isSigner: false, isWritable: true },
    { pubkey: poolSource, isSigner: false, isWritable: true },
    { pubkey: poolDestination, isSigner: false, isWritable: true },
    { pubkey: userDestination, isSigner: false, isWritable: true },
    { pubkey: rewardDestination, isSigner: false, isWritable: true },
    { pubkey: rewardMint, isSigner: false, isWritable: true },
    { pubkey: adminDestination, isSigner: false, isWritable: true },
    { pubkey: tokenProgramId, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];

  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    BufferLayout.nu64("amountIn"),
    BufferLayout.nu64("minimumAmountOut"),
    BufferLayout.nu64("swapDirection"),
  ]);

  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: SwapInstruction.Swap,
      amountIn: amountIn.toBuffer(),
      minimumAmountOut: minimumAmountOut.toBuffer(),
      swapDirection: swapDirection.toBuffer(),
    },
    data
  );

  return new TransactionInstruction({
    keys,
    programId,
    data,
  });
};

export const createDepositInstruction = (
  tokenSwap: PublicKey,
  authority: PublicKey,
  sourceA: PublicKey,
  sourceB: PublicKey,
  intoA: PublicKey,
  intoB: PublicKey,
  poolTokenMint: PublicKey,
  poolTokenAccount: PublicKey,
  tokenProgramId: PublicKey,
  tokenAmountA: NumberU64,
  tokenAmountB: NumberU64,
  minimumPoolTokenAmount: NumberU64,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: tokenSwap, isSigner: false, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: sourceA, isSigner: false, isWritable: true },
    { pubkey: sourceB, isSigner: false, isWritable: true },
    { pubkey: intoA, isSigner: false, isWritable: true },
    { pubkey: intoB, isSigner: false, isWritable: true },
    { pubkey: poolTokenMint, isSigner: false, isWritable: true },
    { pubkey: poolTokenAccount, isSigner: false, isWritable: true },
    { pubkey: tokenProgramId, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];

  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    BufferLayout.nu64("tokenAmountA"),
    BufferLayout.nu64("tokenAmountB"),
    BufferLayout.nu64("minimumPoolTokenAmount"),
  ]);
  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: SwapInstruction.Deposit,
      tokenAmountA: tokenAmountA.toBuffer(),
      tokenAmountB: tokenAmountB.toBuffer(),
      minimumPoolTokenAmount: minimumPoolTokenAmount.toBuffer(),
    },
    data
  );

  return new TransactionInstruction({
    keys,
    programId,
    data,
  });
};

export const createWithdrawInstruction = (
  tokenSwap: PublicKey,
  authority: PublicKey,
  poolMint: PublicKey,
  sourcePoolAccount: PublicKey,
  fromA: PublicKey,
  fromB: PublicKey,
  userAccountA: PublicKey,
  userAccountB: PublicKey,
  adminFeeAccountA: PublicKey,
  adminFeeAccountB: PublicKey,
  tokenProgramId: PublicKey,
  poolTokenAmount: NumberU64,
  minimumTokenA: NumberU64,
  minimumTokenB: NumberU64,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: tokenSwap, isSigner: false, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: poolMint, isSigner: false, isWritable: true },
    { pubkey: sourcePoolAccount, isSigner: false, isWritable: true },
    { pubkey: fromA, isSigner: false, isWritable: true },
    { pubkey: fromB, isSigner: false, isWritable: true },
    { pubkey: userAccountA, isSigner: false, isWritable: true },
    { pubkey: userAccountB, isSigner: false, isWritable: true },
    { pubkey: adminFeeAccountA, isSigner: false, isWritable: true },
    { pubkey: adminFeeAccountB, isSigner: false, isWritable: true },
    { pubkey: tokenProgramId, isSigner: false, isWritable: false },
  ];

  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    BufferLayout.nu64("poolTokenAmount"),
    BufferLayout.nu64("minimumTokenA"),
    BufferLayout.nu64("minimumTokenB"),
  ]);

  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: SwapInstruction.Withdraw,
      poolTokenAmount: new NumberU64(poolTokenAmount).toBuffer(),
      minimumTokenA: new NumberU64(minimumTokenA).toBuffer(),
      minimumTokenB: new NumberU64(minimumTokenB).toBuffer(),
    },
    data
  );

  return new TransactionInstruction({
    keys,
    programId,
    data,
  });
};

export const createWithdrawOneInstruction = (
  tokenSwap: PublicKey,
  authority: PublicKey,
  poolMint: PublicKey,
  sourcePool: PublicKey,
  base: PublicKey,
  quote: PublicKey,
  userDestination: PublicKey,
  adminDestination: PublicKey,
  tokenProgramId: PublicKey,
  poolTokenAmount: NumberU64,
  minimumTokenAmount: NumberU64,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: tokenSwap, isSigner: false, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: poolMint, isSigner: false, isWritable: true },
    { pubkey: sourcePool, isSigner: false, isWritable: true },
    { pubkey: base, isSigner: false, isWritable: true },
    { pubkey: quote, isSigner: false, isWritable: true },
    { pubkey: userDestination, isSigner: false, isWritable: true },
    { pubkey: adminDestination, isSigner: false, isWritable: true },
    { pubkey: tokenProgramId, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];

  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    BufferLayout.nu64("poolTokenAmount"),
    BufferLayout.nu64("minimumTokenAmount"),
  ]);

  const data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: SwapInstruction.WithdrawOne,
      poolTokenAmount: poolTokenAmount.toBuffer(),
      minimumTokenA: minimumTokenAmount.toBuffer(),
    },
    data
  );

  return new TransactionInstruction({
    keys,
    programId,
    data,
  });
};
