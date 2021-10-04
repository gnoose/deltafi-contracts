import { RewardsLayout } from "src/layout";
import { NumberU64 } from "src/util/u64";
import { Struct } from "./struct";

export interface IRewards {
  tradeRewardNumerator: NumberU64;
  tradeRewardDenominator: NumberU64;
  tradeRewardCap: NumberU64;
}

export class Rewards extends Struct {
  constructor(rewards: IRewards) {
    super(rewards, RewardsLayout());
  }

  static fromBuffer(buf: Buffer): IRewards {
    const layout = RewardsLayout();
    return layout.decode(buf) as IRewards;
  }
}
