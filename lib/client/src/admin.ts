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
