import { AccountInfo, PublicKey } from "@solana/web3.js";
import { struct, u8 } from "buffer-layout";
import BN from "bn.js";

import { publicKey, u64, bool, AccountParser } from "../util";
import {
  Fees,
  Rewards,
  FeesLayout,
  RewardsLayout,
  FixedU64,
  FixedU64Layout,
} from ".";

export enum RState {
  /// r = 1
  One = 110,

  /// r > 1
  AboveOne,

  /// r < 1
  BelowOne,
}

export interface SwapInfo {
  isInitialized: boolean;
  isPaused: boolean;
  nonce: number;
  initialAmpFactor: BN;
  targetAmpFactor: BN;
  startRampTo: BN;
  stopRampTo: BN;
  tokenA: PublicKey;
  tokenB: PublicKey;
  deltafiToken: PublicKey;
  poolMint: PublicKey;
  tokenMintA: PublicKey;
  tokenMintB: PublicKey;
  deltafiMint: PublicKey;
  adminFeeKeyA: PublicKey;
  adminFeeKeyB: PublicKey;
  fees: Fees;
  rewards: Rewards;
  k: FixedU64;
  i: FixedU64;
  r: number;
  baseTarget: FixedU64;
  quoteTarget: FixedU64;
  baseReserve: FixedU64;
  quoteReserve: FixedU64;
  isOpenTwap: BN;
  blockTimestampLast: BN;
  basePriceCumulativeLast: FixedU64;
  receiveAmount: FixedU64;
  baseBalance: FixedU64;
  quoteBalance: FixedU64;
}

/** @internal */
export const SwapInfoLayout = struct<SwapInfo>(
  [
    bool("isInitialized"),
    bool("isPaused"),
    u8("nonce"),
    u64("initialAmpFactor"),
    u64("targetAmpFactor"),
    u64("startRampTo"),
    u64("stopRampTo"),
    publicKey("tokenA"),
    publicKey("tokenB"),
    publicKey("deltafiToken"),
    publicKey("poolMint"),
    publicKey("tokenMintA"),
    publicKey("tokenMintB"),
    publicKey("deltafiMint"),
    publicKey("adminFeeKeyA"),
    publicKey("adminFeeKeyB"),
    FeesLayout("fees"),
    RewardsLayout("rewards"),
    FixedU64Layout("k"),
    FixedU64Layout("i"),
    u8("r"),
    FixedU64Layout("baseTarget"),
    FixedU64Layout("quoteTarget"),
    FixedU64Layout("baseReserve"),
    FixedU64Layout("quoteReserve"),
    u64("isOpenTwap"),
    u64("blockTimestampLast"),
    FixedU64Layout("basePriceCumulativeLast"),
    FixedU64Layout("receiveAmount"),
    FixedU64Layout("baseBalance"),
    FixedU64Layout("quoteBalance"),
  ],
  "configInfo"
);

export const SWAP_INFO_SIZE = SwapInfoLayout.span;

export const isSwapInfo = (info: AccountInfo<Buffer>): boolean => {
  return info.data.length === SWAP_INFO_SIZE;
};

export const parseSwapInfo: AccountParser<SwapInfo> = (
  pubkey: PublicKey,
  info: AccountInfo<Buffer>
) => {
  if (!isSwapInfo(info)) return;

  const buffer = Buffer.from(info.data);
  const swapInfo = SwapInfoLayout.decode(buffer);

  if (!swapInfo.isInitialized) return;

  return {
    pubkey,
    info,
    data: swapInfo,
  };
};
