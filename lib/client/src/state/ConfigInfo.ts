import { AccountInfo, PublicKey } from "@solana/web3.js";
import { struct } from "buffer-layout";
import BN from "bn.js";

import { publicKey, u64, bool, AccountParser } from "../util";
import { Fees, Rewards, FeesLayout, RewardsLayout } from ".";

export interface ConfigInfo {
  isInitialized: boolean;
  isPaused: boolean;
  ampFactor: BN;
  futureAdminDeadline: BN;
  futureAdminKey: PublicKey;
  adminKey: PublicKey;
  detafiMint: PublicKey;
  fees: Fees;
  rewards: Rewards;
}

/** @internal */
export const ConfigInfoLayout = struct<ConfigInfo>(
  [
    bool("isInitialized"),
    bool("isPaused"),
    u64("ampFactor"),
    u64("futureAdminDeadline"),
    publicKey("futureAdminKey"),
    publicKey("adminKey"),
    publicKey("deltafiMint"),
    FeesLayout("fees"),
    RewardsLayout("rewards"),
  ],
  "configInfo"
);

export const CONFIG_SIZE = ConfigInfoLayout.span;

export const isConfigInfo = (info: AccountInfo<Buffer>): boolean => {
  return info.data.length === CONFIG_SIZE;
};

export const parserConfigInfo: AccountParser<ConfigInfo> = (
  pubkey: PublicKey,
  info: AccountInfo<Buffer>
) => {
  if (!isConfigInfo(info)) return;

  const buffer = Buffer.from(info.data);
  const configInfo = ConfigInfoLayout.decode(buffer);

  if (!configInfo.isInitialized) return;

  return {
    pubkey,
    info,
    data: configInfo,
  };
};
