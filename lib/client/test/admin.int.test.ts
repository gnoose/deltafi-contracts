import {
  Keypair,
  Connection,
  PublicKey,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import BN from "bn.js";

import { getDeploymentInfo, newAccountWithLamports, sleep } from "./helpers";
import {
  CLUSTER_URL,
  AMP_FACTOR,
  BOOTSTRAP_TIMEOUT,
  DEFAULT_FEES,
  DEFAULT_REWARDS,
} from "../src/constants";
import { SwapInfo } from "../src/state";
import { admin } from "../src";
import { AdminInitializeData } from "../src";

describe("e2e test for admin instructions", () => {
  // Cluster connection
  let connection: Connection;
  // Fee payer
  let payer: Keypair;
  // Admin account
  let owner: Keypair;
  // Config account
  let configAccount: Keypair;
  // Token swap account
  let tokenSwap: Keypair;
  // Swap info
  let swapInfo: SwapInfo;
  // Swap program ID
  let swapProgramId: PublicKey;
  // Admin initialize data
  let adminInitData: AdminInitializeData = {
    ampFactor: new BN(AMP_FACTOR),
    fees: DEFAULT_FEES,
    rewards: DEFAULT_REWARDS,
  };

  beforeAll(async (done) => {
    // Bootstrap test env
    connection = new Connection(CLUSTER_URL, "single");
    payer = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);
    owner = new Keypair();
    swapProgramId = getDeploymentInfo().stableSwapProgramId;
    configAccount = new Keypair();

    await sleep(500);

    await admin.initialize(
      connection,
      payer,
      configAccount,
      owner,
      adminInitData,
      swapProgramId
    );

    done();
  }, BOOTSTRAP_TIMEOUT);

  it("load configuration", async () => {
    let loadedConfig = await admin.loadConfig(
      connection,
      configAccount.publicKey,
      swapProgramId
    );

    expect(loadedConfig.adminKey).toEqual(owner.publicKey);
    expect(loadedConfig.ampFactor.toNumber()).toEqual(AMP_FACTOR);
    expect(
      loadedConfig.rewards.tradeRewardNumerator.toString("hex", 8)
    ).toEqual(DEFAULT_REWARDS.tradeRewardNumerator.toString("hex", 8));
    expect(
      loadedConfig.rewards.tradeRewardDenominator.toString("hex", 8)
    ).toEqual(DEFAULT_REWARDS.tradeRewardDenominator.toString("hex", 8));
    expect(loadedConfig.rewards.tradeRewardCap.toString("hex", 8)).toEqual(
      DEFAULT_REWARDS.tradeRewardCap.toString("hex", 8)
    );
    expect(loadedConfig.fees.adminTradeFeeNumerator.toString("hex", 8)).toEqual(
      DEFAULT_FEES.adminTradeFeeNumerator.toString("hex", 8)
    );
    expect(
      loadedConfig.fees.adminTradeFeeDenominator.toString("hex", 8)
    ).toEqual(DEFAULT_FEES.adminTradeFeeDenominator.toString("hex", 8));
    expect(
      loadedConfig.fees.adminWithdrawFeeNumerator.toString("hex", 8)
    ).toEqual(DEFAULT_FEES.adminWithdrawFeeNumerator.toString("hex", 8));
    expect(
      loadedConfig.fees.adminWithdrawFeeDenominator.toString("hex", 8)
    ).toEqual(DEFAULT_FEES.adminWithdrawFeeDenominator.toString("hex", 8));
    expect(loadedConfig.fees.tradeFeeNumerator.toString("hex", 8)).toEqual(
      DEFAULT_FEES.tradeFeeNumerator.toString("hex", 8)
    );
    expect(loadedConfig.fees.tradeFeeDenominator.toString("hex", 8)).toEqual(
      DEFAULT_FEES.tradeFeeDenominator.toString("hex", 8)
    );
    expect(loadedConfig.fees.withdrawFeeNumerator.toString("hex", 8)).toEqual(
      DEFAULT_FEES.withdrawFeeNumerator.toString("hex", 8)
    );
    expect(loadedConfig.fees.withdrawFeeDenominator.toString("hex", 8)).toEqual(
      DEFAULT_FEES.withdrawFeeDenominator.toString("hex", 8)
    );
    expect(loadedConfig.futureAdminDeadline.toNumber()).toEqual(0);
  });
});
