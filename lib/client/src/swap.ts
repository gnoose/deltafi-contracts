import type { Connection } from "@solana/web3.js";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";

import {
  getMinBalanceRentForExempt,
  loadAccount,
  sendAndConfirmTransaction,
} from "./util";
import { SwapInfo, SwapInfoLayout, parseSwapInfo } from "./state";
import { InitializeData, createInitSwapInstruction } from "./instructions";

export const initialize = async (
  connection: Connection,
  payer: Keypair,
  configAccount: Keypair,
  swapAccount: Keypair,
  authority: PublicKey,
  adminFeeKeyA: PublicKey,
  adminFeeKeyB: PublicKey,
  tokenA: PublicKey,
  tokenB: PublicKey,
  poolMint: PublicKey,
  poolToken: PublicKey,
  deltafiToken: PublicKey,
  initData: InitializeData,
  swapProgramId: PublicKey
) => {
  const balanceNeeded = await getMinBalanceRentForExempt(
    connection,
    SwapInfoLayout.span
  );
  const transaction = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: swapAccount.publicKey,
      lamports: balanceNeeded,
      space: SwapInfoLayout.span,
      programId: swapProgramId,
    })
  );

  const instruction = createInitSwapInstruction(
    configAccount.publicKey,
    swapAccount.publicKey,
    authority,
    adminFeeKeyA,
    adminFeeKeyB,
    tokenA,
    tokenB,
    poolMint,
    poolToken,
    deltafiToken,
    initData,
    swapProgramId
  );

  transaction.add(instruction);

  await sendAndConfirmTransaction(
    "create and initialize SwapInfo account",
    connection,
    transaction,
    payer,
    swapAccount
  );
};

export const loadSwapInfo = async (
  connection: Connection,
  address: PublicKey,
  swapProgramId: PublicKey
): Promise<SwapInfo> => {
  const accountInfo = await loadAccount(connection, address, swapProgramId);

  const parsed = parseSwapInfo(address, accountInfo);

  if (!parsed) throw new Error("Failed to load configuration account");

  return parsed.data;
};
