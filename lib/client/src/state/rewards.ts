import BN from "bn.js";

import { struct } from "buffer-layout";
import { u64 } from "../util";

export interface Rewards {
  tradeRewardNumerator: BN;
  tradeRewardDenominator: BN;
  tradeRewardCap: BN;
}

/** @internal */
export const RewardsLayout = (property: string = "rewards") =>
  struct<Rewards>(
    [
      u64("tradeRewardNumerator"),
      u64("tradeRewardDenominator"),
      u64("tradeRewardCap"),
    ],
    property
  );
