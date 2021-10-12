import BN from "bn.js";

import { struct } from "buffer-layout";
import { u64 } from "../util";

export interface Fees {
  adminTradeFeeNumerator: BN;
  adminTradeFeeDenominator: BN;
  adminWithdrawFeeNumerator: BN;
  adminWithdrawFeeDenominator: BN;
  tradeFeeNumerator: BN;
  tradeFeeDenominator: BN;
  withdrawFeeNumerator: BN;
  withdrawFeeDenominator: BN;
}

/** @internal */
export const FeesLayout = (property: string = "fees") =>
  struct<Fees>(
    [
      u64("adminTradeFeeNumerator"),
      u64("adminTradeFeeDenominator"),
      u64("adminWithdrawFeeNumerator"),
      u64("adminWithdrawFeeDenominator"),
      u64("tradeFeeNumerator"),
      u64("tradeFeeDenominator"),
      u64("withdrawFeeNumerator"),
      u64("withdrawFeeDenominator"),
    ],
    property
  );
