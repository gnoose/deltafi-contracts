import { PublicKey } from "@solana/web3.js";
import { IFees, IRewards } from "./struct";
import { NumberU64 } from "./util/u64";

export const DEFAULT_TOKEN_DECIMALS = 6;

export const TOKEN_PROGRAM_ID = new PublicKey(
  "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
);

export const ZERO_TS = 0;

export const DEFAULT_FEE_NUMERATOR = 0;
export const DEFAULT_FEE_DENOMINATOR = 1000;
export const DEFAULT_FEES: IFees = {
  adminTradeFeeNumerator: new NumberU64(DEFAULT_FEE_NUMERATOR),
  adminTradeFeeDenominator: new NumberU64(DEFAULT_FEE_DENOMINATOR),
  adminWithdrawFeeNumerator: new NumberU64(DEFAULT_FEE_NUMERATOR),
  adminWithdrawFeeDenominator: new NumberU64(DEFAULT_FEE_DENOMINATOR),
  tradeFeeNumerator: new NumberU64(1),
  tradeFeeDenominator: new NumberU64(4),
  withdrawFeeNumerator: new NumberU64(DEFAULT_FEE_NUMERATOR),
  withdrawFeeDenominator: new NumberU64(DEFAULT_FEE_DENOMINATOR),
};

export const DEFAULT_REWARD_NUMERATOR = 1;
export const DEFAULT_REWARD_DENOMINATOR = 1000;
export const DEFAULT_REWARD_CAP = 100;
export const DEFAULT_REWARDS: IRewards = {
  tradeRewardNumerator: new NumberU64(DEFAULT_REWARD_NUMERATOR),
  tradeRewardDenominator: new NumberU64(DEFAULT_REWARD_DENOMINATOR),
  tradeRewardCap: new NumberU64(DEFAULT_REWARD_CAP),
};

export const CLUSTER_URL = "http://localhost:8899";
export const BOOTSTRAP_TIMEOUT = 10000;
export const AMP_FACTOR = 100;

export const TWAP_OPEN = 1;

/// swap directions - sell base
export const SWAP_DIRECTION_SELL_BASE = 0;

/// swap directions - sell quote
export const SWAP_DIRECTION_SELL_QUOTE = 1;
