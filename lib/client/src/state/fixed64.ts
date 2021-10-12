import BN from "bn.js";

import { struct, u8 } from "buffer-layout";
import { u64 } from "../util";

export interface FixedU64 {
  inner: BN;
  precision: number;
}

/** @internal */
export const FixedU64Layout = (property: string = "fixedU64") =>
  struct<FixedU64>([u64("inner"), u8("precision")], property);
