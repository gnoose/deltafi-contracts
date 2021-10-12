import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { PublicKey, TransactionInstruction } from "@solana/web3.js";
import { struct, u8 } from "buffer-layout";
import BN from "bn.js";

import { u64 } from "../util";

export enum SwapInstruction {
  Initialize = 0,
  Swap,
  Deposit,
  Withdraw,
  WithdrawOne,
}

export interface InitializeData {
  nonce: number;
  k: BN;
  i: BN;
  isOpenTwap: BN;
}

/** @internal */
export const InitializeDataLayout = struct<InitializeData>(
  [u8("nonce"), u64("k"), u64("i"), u64("isOpenTwap")],
  "initData"
);

export interface SwapData {
  amountIn: BN;
  minimumAmountOut: BN;
  swapDirection: BN;
}

/** @internal */
export const SwapDataLayout = struct<SwapData>(
  [u64("amountIn"), u64("minimumAmountOut"), u64("swapDirection")],
  "swapData"
);

export const createInitSwapInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  authority: PublicKey,
  adminFeeKeyA: PublicKey,
  adminFeeKeyB: PublicKey,
  tokenA: PublicKey,
  tokenB: PublicKey,
  poolMint: PublicKey,
  poolToken: PublicKey,
  deltafiToken: PublicKey,
  initData: InitializeData,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: true, isWritable: true },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminFeeKeyA, isSigner: false, isWritable: false },
    { pubkey: adminFeeKeyB, isSigner: false, isWritable: false },
    { pubkey: tokenA, isSigner: false, isWritable: false },
    { pubkey: tokenB, isSigner: false, isWritable: false },
    { pubkey: poolMint, isSigner: false, isWritable: true },
    { pubkey: poolToken, isSigner: false, isWritable: true },
    { pubkey: deltafiToken, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction"), InitializeDataLayout]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: SwapInstruction.Initialize,
      initData,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    programId,
    data,
  });
};
