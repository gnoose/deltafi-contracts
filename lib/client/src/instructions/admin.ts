import {
  PublicKey,
  TransactionInstruction,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { struct, u8 } from "buffer-layout";
import BN from "bn.js";

import { Fees, Rewards } from "../state";
import { FeesLayout, RewardsLayout } from "../state";
import { u64 } from "../util";

export enum AdminInstruction {
  Initialize = 100,
  RampA,
  StopRamp,
  Pause,
  Unpause,
  SetFeeAccount,
  ApplyNewAdmin,
  CommitNewAdmin,
  SetNewFees,
  SetNewRewards,
}

export interface AdminInitializeData {
  ampFactor: BN;
  fees: Fees;
  rewards: Rewards;
}

/** @internal */
export const AdminInitializeDataLayout = struct<AdminInitializeData>(
  [u64("ampFactor"), FeesLayout("fees"), RewardsLayout("rewards")],
  "initData"
);

export interface RampAData {
  targetAmp: BN;
  stopRampTimestamp: BN;
}

/** @internal */
export const RampADataLayout = struct<RampAData>(
  [u64("targetAmp"), u64("stopRampTimestamp")],
  "ramp"
);

export const createAdminInitializeInstruction = (
  config: PublicKey,
  adminKey: PublicKey,
  initData: AdminInitializeData,
  programId: PublicKey
): TransactionInstruction => {
  const keys = [
    { pubkey: config, isSigner: true, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction"), AdminInitializeDataLayout]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.Initialize,
      initData,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createRampAInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  ramp: RampAData,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction"), RampADataLayout]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.RampA,
      ramp,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createStopRampInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.StopRamp,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createPauseInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: true, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.Pause,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createUnpauseInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: true, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.Unpause,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetFeeAccountInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  newFeeAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: true, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: newFeeAccount, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.SetFeeAccount,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createApplyNewAdminInstruction = (
  config: PublicKey,
  adminKey: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.ApplyNewAdmin,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createCommitNewAdminInstruction = (
  config: PublicKey,
  adminKey: PublicKey,
  newAdminAccount: PublicKey,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: true, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
    { pubkey: newAdminAccount, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.CommitNewAdmin,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetNewFeesInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  newFees: Fees,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: true, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction"), FeesLayout("newFees")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.CommitNewAdmin,
      newFees,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};

export const createSetNewRewardsInstruction = (
  config: PublicKey,
  tokenSwap: PublicKey,
  adminKey: PublicKey,
  newRewards: Rewards,
  programId: PublicKey
) => {
  const keys = [
    { pubkey: config, isSigner: false, isWritable: false },
    { pubkey: tokenSwap, isSigner: true, isWritable: true },
    { pubkey: adminKey, isSigner: true, isWritable: false },
  ];
  const dataLayout = struct([u8("instruction"), RewardsLayout("newRewards")]);
  let data = Buffer.alloc(dataLayout.span);
  dataLayout.encode(
    {
      instruction: AdminInstruction.CommitNewAdmin,
      newRewards,
    },
    data
  );

  return new TransactionInstruction({
    keys,
    data,
    programId,
  });
};
