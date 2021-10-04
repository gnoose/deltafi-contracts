import * as BufferLayout from "buffer-layout";
import {
  Account,
  PublicKey,
  TransactionInstruction,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";

import { NumberU64 } from "../util/u64";
import { Fees, Rewards } from "src/struct";
import { FeesLayout, RewardsLayout } from "../layout";

export enum AdminInstruction {
  RampA = 100,
  StopRamp,
  Pause,
  Unpause,
  SetFeeAccount,
  ApplyNewAdmin,
  CommitNewAdmin,
  SetNewFees,
  SetNewRewards,
}

export const createRampAInstruction = (
  tokenSwap: PublicKey,
  authority: PublicKey,
  adminAccount: PublicKey,
  targetAmp: NumberU64,
  stopRampTimestamp: NumberU64,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwap, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    BufferLayout.nu64("targetAmp"),
    BufferLayout.nu64("stopRampTimestamp"),
  ]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.RampA,
      targetAmp: targetAmp.toBuffer(),
      stopRampTimestamp: stopRampTimestamp.toBuffer(),
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createStopRampInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.StopRamp,
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createPauseInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.Pause,
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createUnpauseInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.Unpause,
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetFeeAccountInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  newFeeAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
    { pubkey: newFeeAccount, isSigner: false, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.SetFeeAccount,
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createApplyNewAdminInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.ApplyNewAdmin,
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createCommitNewAdminInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  newAdminAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
    { pubkey: newAdminAccount, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([BufferLayout.u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.CommitNewAdmin,
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetNewFeesInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  newFees: Fees,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    FeesLayout("newFees"),
  ]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.CommitNewAdmin,
      newFees: newFees.toBuffer(),
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetNewRewardsInstruction = (
  tokenSwapAccount: Account,
  authority: PublicKey,
  adminAccount: PublicKey,
  newRewards: Rewards,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: tokenSwapAccount.publicKey, isSigner: true, isWritable: false },
    { pubkey: authority, isSigner: false, isWritable: false },
    { pubkey: adminAccount, isSigner: true, isWritable: false },
  ];
  const dataLayout = BufferLayout.struct([
    BufferLayout.u8("instruction"),
    RewardsLayout("newRewards"),
  ]);
  let data = Buffer.alloc(dataLayout.span);
  const encodeLength = dataLayout.encode(
    {
      instruction: AdminInstruction.CommitNewAdmin,
      newRewards: newRewards.toBuffer(),
    },
    data
  );
  data = data.slice(0, encodeLength);

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};
