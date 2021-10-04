import { NumberU64 } from "../util/u64";
import { FeesLayout } from "../layout";
import { Struct } from "./struct";

export interface IFees {
  adminTradeFeeNumerator: NumberU64;
  adminTradeFeeDenominator: NumberU64;
  adminWithdrawFeeNumerator: NumberU64;
  adminWithdrawFeeDenominator: NumberU64;
  tradeFeeNumerator: NumberU64;
  tradeFeeDenominator: NumberU64;
  withdrawFeeNumerator: NumberU64;
  withdrawFeeDenominator: NumberU64;
}

export class Fees extends Struct {
  constructor(fees: IFees) {
    super(fees, FeesLayout());
  }

  static fromBuffer(buf: Buffer): IFees {
    const layout = FeesLayout();
    return layout.decode(buf) as IFees;
  }
}
