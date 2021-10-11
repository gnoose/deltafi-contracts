import * as BufferLayout from "buffer-layout";

/**
 * Layout for a public key
 */
export const PublicKeyLayout = (property: string = "publicKey") =>
  BufferLayout.blob(32, property);

/**
 * Layout for U256
 */
export const U256Layout = (property: string = "u256") =>
  BufferLayout.blob(32, property);

/**
 * Layout for FixedU256
 */
export const FixedU256Layout = (property: string = "fixedU256") =>
  BufferLayout.struct([U256Layout("inner"), U256Layout("basePoint")], property);

/**
 * Layout for FixedU64
 */
export const FixedU64Layout = (property: string = "fixedU64") =>
  BufferLayout.struct(
    [BufferLayout.nu64("inner"), BufferLayout.nu64("basePoint")],
    property
  );

/**
 * Layout for fees struct
 */
export const FeesLayout = (property: string = "fees") =>
  BufferLayout.struct(
    [
      BufferLayout.nu64("adminTradeFeeNumerator"),
      BufferLayout.nu64("adminTradeFeeDenominator"),
      BufferLayout.nu64("adminWithdrawFeeNumerator"),
      BufferLayout.nu64("adminWithdrawFeeDenominator"),
      BufferLayout.nu64("tradeFeeNumerator"),
      BufferLayout.nu64("tradeFeeDenominator"),
      BufferLayout.nu64("withdrawFeeNumerator"),
      BufferLayout.nu64("withdrawFeeDenominator"),
    ],
    property
  );

/**
 * Layout for rewards struct
 */
export const RewardsLayout = (property: string = "rewards") =>
  BufferLayout.struct(
    [
      BufferLayout.nu64("tradeRewardNumerator"),
      BufferLayout.nu64("tradeRewardDenominator"),
      BufferLayout.nu64("tradeRewardCap"),
    ],
    property
  );

/**
 * Layout for oracle struct
 */
export const OracleLayout = (property: string = "oracle") =>
  BufferLayout.struct(
    [
      BufferLayout.u32("period"),
      PublicKeyLayout("token0"),
      PublicKeyLayout("token1"),
      U256Layout("price0Cumulative"),
      U256Layout("price1Cumulative"),
      BufferLayout.nu64("blockTimestamp"),
      U256Layout("price0Average"),
      U256Layout("price1Average"),
    ],
    property
  );

/**
 * Layout for config info state
 */
export const ConfigInfoLayout: typeof BufferLayout.Structure = BufferLayout.struct(
  [
    BufferLayout.u8("isInitialized"),
    BufferLayout.u8("isPaused"),
    BufferLayout.nu64("ampFactor"),
    BufferLayout.ns64("futureAdminDeadline"),
    PublicKeyLayout("futureAdminKey"),
    PublicKeyLayout("adminKey"),
    PublicKeyLayout("deltafiMint"),
    FeesLayout("fees"),
    RewardsLayout("rewards"),
  ]
);

/**
 * Layout for stable swap state
 */
export const StableSwapLayout: typeof BufferLayout.Structure = BufferLayout.struct(
  [
    BufferLayout.u8("isInitialized"),
    BufferLayout.u8("isPaused"),
    BufferLayout.u8("nonce"),
    BufferLayout.nu64("initialAmpFactor"),
    BufferLayout.nu64("targetAmpFactor"),
    BufferLayout.ns64("startRampTs"),
    BufferLayout.ns64("stopRampTs"),
    PublicKeyLayout("tokenAccountA"),
    PublicKeyLayout("tokenAccountB"),
    PublicKeyLayout("deltafiToken"),
    PublicKeyLayout("poolMint"),
    PublicKeyLayout("mintA"),
    PublicKeyLayout("mintB"),
    PublicKeyLayout("deltafiMint"),
    PublicKeyLayout("adminFeeAccountA"),
    PublicKeyLayout("adminFeeAccountB"),
    FeesLayout("fees"),
    RewardsLayout("rewards"),
    FixedU64Layout("k"),
    FixedU64Layout("l"),
    BufferLayout.u8("r"),
    FixedU64Layout("baseTarget"),
    FixedU64Layout("quoteTarget"),
    FixedU64Layout("baseReserve"),
    FixedU64Layout("quoteReserve"),
    BufferLayout.nu64("isOpenTwap"),
    BufferLayout.nu64("blockTimestamp"),
    FixedU64Layout("basePriceCumulative"),
    FixedU64Layout("receiveAmount"),
    FixedU64Layout("baseBalance"),
    FixedU64Layout("quoteBalance"),
  ]
);

// !!need to be fixed
/**
 * Layout for farm state
 */
/*
export const FarmLayout: typeof BufferLayout.Structure = BufferLayout.struct([
  BufferLayout.u8("isInitialized"),
  BufferLayout.u8("isPaused"),
  BufferLayout.u8("nonce"),
  BufferLayout.nu64("initialAmpFactor"),
  BufferLayout.nu64("targetAmpFactor"),
  BufferLayout.ns64("startRampTs"),
  BufferLayout.ns64("stopRampTs"),
  BufferLayout.ns64("futureAdminDeadline"),
  PublicKeyLayout("futureAdminAccount"),
  PublicKeyLayout("adminAccount"),
  PublicKeyLayout("tokenAccountA"),
  PublicKeyLayout("tokenAccountB"),
  PublicKeyLayout("tokenPool"),
  PublicKeyLayout("mintA"),
  PublicKeyLayout("mintB"),
  PublicKeyLayout("adminFeeAccountA"),
  PublicKeyLayout("adminFeeAccountB"),
  BufferLayout.nu64("adminTradeFeeNumerator"),
  BufferLayout.nu64("adminTradeFeeDenominator"),
  BufferLayout.nu64("adminWithdrawFeeNumerator"),
  BufferLayout.nu64("adminWithdrawFeeDenominator"),
  BufferLayout.nu64("tradeFeeNumerator"),
  BufferLayout.nu64("tradeFeeDenominator"),
  BufferLayout.nu64("withdrawFeeNumerator"),
  BufferLayout.nu64("withdrawFeeDenominator"),
]);
*/

// !!need to be fixed
/**
 * Layout for farm base state
 */
/*
export const FarmBaseLayout: typeof BufferLayout.Structure = BufferLayout.struct(
  [
    BufferLayout.u8("isInitialized"),
    BufferLayout.u8("isPaused"),
    BufferLayout.u8("nonce"),
    BufferLayout.nu64("initialAmpFactor"),
    BufferLayout.nu64("targetAmpFactor"),
    BufferLayout.ns64("startRampTs"),
    BufferLayout.ns64("stopRampTs"),
    BufferLayout.ns64("futureAdminDeadline"),
    PublicKeyLayout("futureAdminAccount"),
    PublicKeyLayout("adminAccount"),
    PublicKeyLayout("tokenAccountA"),
    PublicKeyLayout("tokenAccountB"),
    PublicKeyLayout("tokenPool"),
    PublicKeyLayout("mintA"),
    PublicKeyLayout("mintB"),
    PublicKeyLayout("adminFeeAccountA"),
    PublicKeyLayout("adminFeeAccountB"),
    BufferLayout.nu64("adminTradeFeeNumerator"),
    BufferLayout.nu64("adminTradeFeeDenominator"),
    BufferLayout.nu64("adminWithdrawFeeNumerator"),
    BufferLayout.nu64("adminWithdrawFeeDenominator"),
    BufferLayout.nu64("tradeFeeNumerator"),
    BufferLayout.nu64("tradeFeeDenominator"),
    BufferLayout.nu64("withdrawFeeNumerator"),
    BufferLayout.nu64("withdrawFeeDenominator"),
  ]
);
*/
