import {
  Keypair,
  Connection,
  PublicKey,
  LAMPORTS_PER_SOL,
  clusterApiUrl,
} from "@solana/web3.js";
import { Token, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import BN from "bn.js";

import { getDeploymentInfo, newAccountWithLamports, sleep } from "./helpers";
import {
  CLUSTER_URL,
  AMP_FACTOR,
  BOOTSTRAP_TIMEOUT,
  DEFAULT_FEES,
  DEFAULT_REWARDS,
  DEFAULT_TOKEN_DECIMALS,
  K,
  I,
  TWAP_OPEN,
  MIN_AMP,
  MIN_RAMP_DURATION,
} from "../src/constants";

import {
  admin,
  AdminInitializeData,
  swap,
  InitializeData,
  // RampAData,
} from "../src";

const INITIAL_TOKEN_A_AMOUNT = LAMPORTS_PER_SOL;
const INITIAL_TOKEN_B_AMOUNT = LAMPORTS_PER_SOL;

describe("e2e test for admin instructions", () => {
  // Cluster connection
  let connection: Connection;
  // Fee payer
  let payer: Keypair;
  // Admin account
  let owner: Keypair;
  // Swap user account
  let user: Keypair;
  // Config account
  let configAccount: Keypair;
  // Token swap account
  let tokenSwap: Keypair;
  // authority of the token and accounts
  let authority: PublicKey;
  // nonce used to generate the authority public key
  let nonce: number;
  // Pool token
  let poolMint: Token;
  let poolToken: PublicKey;
  // Tokens to swap
  let mintA: Token;
  let mintB: Token;
  let tokenA: PublicKey;
  let tokenB: PublicKey;
  // Admin fee account
  let adminFeeKeyA: PublicKey;
  let adminFeeKeyB: PublicKey;
  // Deltafi token
  let deltafiMint: Token;
  let deltafiToken: PublicKey;
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
    user = await newAccountWithLamports(connection, LAMPORTS_PER_SOL);
    swapProgramId = getDeploymentInfo().stableSwapProgramId;
    configAccount = new Keypair();
    tokenSwap = new Keypair();
    [authority, nonce] = await PublicKey.findProgramAddress(
      [tokenSwap.publicKey.toBuffer()],
      swapProgramId
    );

    await admin.initialize(
      connection,
      payer,
      configAccount,
      owner,
      adminInitData,
      swapProgramId
    );

    // creating pool mint
    poolMint = await Token.createMint(
      connection,
      payer,
      authority,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );

    // // creating pool account
    poolToken = await poolMint.createAccount(user.publicKey);

    // creating token A mint
    mintA = await Token.createMint(
      connection,
      payer,
      user.publicKey,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );
    // creating token A account
    tokenA = await mintA.createAccount(authority);
    adminFeeKeyA = await mintA.createAccount(owner.publicKey);
    await mintA.mintTo(tokenA, user, [], INITIAL_TOKEN_A_AMOUNT);

    // creating token B mint
    mintB = await Token.createMint(
      connection,
      payer,
      user.publicKey,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );

    tokenB = await mintB.createAccount(authority);
    adminFeeKeyB = await mintB.createAccount(owner.publicKey);
    await mintB.mintTo(tokenB, user, [], INITIAL_TOKEN_B_AMOUNT);

    // creating deltafi token
    deltafiMint = await Token.createMint(
      connection,
      payer,
      authority,
      null,
      DEFAULT_TOKEN_DECIMALS,
      TOKEN_PROGRAM_ID
    );
    deltafiToken = await deltafiMint.createAccount(user.publicKey);

    const swapInitData: InitializeData = {
      nonce,
      k: new BN(K),
      i: new BN(I),
      isOpenTwap: new BN(TWAP_OPEN),
    };

    await swap.initialize(
      connection,
      payer,
      configAccount,
      tokenSwap,
      authority,
      adminFeeKeyA,
      adminFeeKeyB,
      tokenA,
      tokenB,
      poolMint.publicKey,
      poolToken,
      deltafiToken,
      swapInitData,
      swapProgramId
    );

    done();
  }, BOOTSTRAP_TIMEOUT);

  it("load configuration", async () => {
    const loadedConfig = await admin.loadConfig(
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

  // it("apply ramp_a", async () => {
  //   const ramp: RampAData = {
  //     targetAmp: new BN(MIN_AMP * 100),
  //     stopRampTimestamp: new BN(MIN_RAMP_DURATION),
  //   };

  //   await admin.applyRampA(
  //     connection,
  //     payer,
  //     configAccount.publicKey,
  //     tokenSwap,
  //     owner,
  //     ramp,
  //     swapProgramId
  //   );

  //   const loadSwapInfo = await swap.loadSwapInfo(
  //     connection,
  //     tokenSwap.publicKey,
  //     swapProgramId
  //   );

  //   expect(loadSwapInfo.initialAmpFactor.toNumber()).toEqual(AMP_FACTOR);
  //   expect(loadSwapInfo.targetAmpFactor.toNumber()).toEqual(MIN_AMP * 100);
  //   expect(loadSwapInfo.startRampTo.toNumber()).toEqual(MIN_RAMP_DURATION);
  //   expect(loadSwapInfo.stopRampTo.toNumber()).toEqual(MIN_RAMP_DURATION);
  // });

  it("stop ramp", async () => {
    await admin.stopRamp(
      connection,
      payer,
      configAccount.publicKey,
      tokenSwap,
      owner,
      swapProgramId
    );

    const loadSwapInfo = await swap.loadSwapInfo(
      connection,
      tokenSwap.publicKey,
      swapProgramId
    );

    expect(loadSwapInfo.initialAmpFactor.toNumber()).toEqual(AMP_FACTOR);
    expect(loadSwapInfo.targetAmpFactor.toNumber()).toEqual(AMP_FACTOR);
    expect(loadSwapInfo.basePriceCumulativeLast.inner.toNumber()).toEqual(0);
  });
});
