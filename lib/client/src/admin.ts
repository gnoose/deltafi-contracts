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
import { ConfigInfo, ConfigInfoLayout, parserConfigInfo } from "./state";
import {
  AdminInitializeData,
  createAdminInitializeInstruction,
  createRampAInstruction,
  createStopRampInstruction,
  RampAData,
} from "./instructions";

export const initialize = async (
  connection: Connection,
  payer: Keypair,
  configAccount: Keypair,
  adminAccount: Keypair,
  initData: AdminInitializeData,
  swapProgramId: PublicKey
) => {
  const balanceNeeded = await getMinBalanceRentForExempt(
    connection,
    ConfigInfoLayout.span
  );
  const transaction = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: configAccount.publicKey,
      lamports: balanceNeeded,
      space: ConfigInfoLayout.span,
      programId: swapProgramId,
    })
  );

  const instruction = createAdminInitializeInstruction(
    configAccount.publicKey,
    adminAccount.publicKey,
    initData,
    swapProgramId
  );

  transaction.add(instruction);

  await sendAndConfirmTransaction(
    "create and initialize ConfigInfo account",
    connection,
    transaction,
    payer,
    configAccount,
    adminAccount
  );
};

export const loadConfig = async (
  connection: Connection,
  address: PublicKey,
  swapProgramId: PublicKey
): Promise<ConfigInfo> => {
  const accountInfo = await loadAccount(connection, address, swapProgramId);

  const parsed = parserConfigInfo(address, accountInfo);

  if (!parsed) throw new Error("Failed to load configuration account");

  return parsed.data;
};

export const applyRampA = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwapAccount: Keypair,
  adminAccount: Keypair,
  ramp: RampAData,
  swapProgramId: PublicKey
) => {
  const instruction = createRampAInstruction(
    config,
    tokenSwapAccount.publicKey,
    adminAccount.publicKey,
    ramp,
    swapProgramId
  );
  const transaction = new Transaction().add(instruction);
  await sendAndConfirmTransaction(
    "apply amplification",
    connection,
    transaction,
    payer,
    adminAccount
  );
};

export const stopRamp = async (
  connection: Connection,
  payer: Keypair,
  config: PublicKey,
  tokenSwapAccount: Keypair,
  adminAccount: Keypair,
  swapProgramId: PublicKey
) => {
  const instruction = createStopRampInstruction(
    config,
    tokenSwapAccount.publicKey,
    adminAccount.publicKey,
    swapProgramId
  );
  const transaction = new Transaction().add(instruction);
  await sendAndConfirmTransaction(
    "stop ramp",
    connection,
    transaction,
    payer,
    adminAccount
  );
};
