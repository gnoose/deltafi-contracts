import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";

import { Fees, Rewards } from "./state";

export const DEFAULT_TOKEN_DECIMALS = 6;

export const TOKEN_PROGRAM_ID = new PublicKey(
  "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
);

export const ZERO_TS = 0;

export const DEFAULT_FEE_NUMERATOR = 0;
export const DEFAULT_FEE_DENOMINATOR = 1000;
export const DEFAULT_FEES: Fees = {
  adminTradeFeeNumerator: new BN(DEFAULT_FEE_NUMERATOR),
  adminTradeFeeDenominator: new BN(DEFAULT_FEE_DENOMINATOR),
  adminWithdrawFeeNumerator: new BN(DEFAULT_FEE_NUMERATOR),
  adminWithdrawFeeDenominator: new BN(DEFAULT_FEE_DENOMINATOR),
  tradeFeeNumerator: new BN(1),
  tradeFeeDenominator: new BN(4),
  withdrawFeeNumerator: new BN(DEFAULT_FEE_NUMERATOR),
  withdrawFeeDenominator: new BN(DEFAULT_FEE_DENOMINATOR),
};

export const DEFAULT_REWARD_NUMERATOR = 1;
export const DEFAULT_REWARD_DENOMINATOR = 1000;
export const DEFAULT_REWARD_CAP = 100;
export const DEFAULT_REWARDS: Rewards = {
  tradeRewardNumerator: new BN(DEFAULT_REWARD_NUMERATOR),
  tradeRewardDenominator: new BN(DEFAULT_REWARD_DENOMINATOR),
  tradeRewardCap: new BN(DEFAULT_REWARD_CAP),
};

export const CLUSTER_URL = "http://localhost:8899";
export const BOOTSTRAP_TIMEOUT = 50000;
export const AMP_FACTOR = 100;
export const K = 0.5;
export const I = 100;
export const TWAP_OPEN = 1;
export const MIN_AMP = 1;
export const MAX_AMP = 1000000;
export const MIN_RAMP_DURATION = 86400;

/// swap directions - sell base
export const SWAP_DIRECTION_SELL_BASE = 0;

/// swap directions - sell quote
export const SWAP_DIRECTION_SELL_QUOTE = 1;
