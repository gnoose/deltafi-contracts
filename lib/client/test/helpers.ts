import fs from "fs";
import { Keypair, PublicKey, Connection } from "@solana/web3.js";

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function newAccountWithLamports(
  connection: Connection,
  lamports: number = 1000000
): Promise<Keypair> {
  const account = Keypair.generate();

  try {
    const airdropSignature = await connection.requestAirdrop(
      account.publicKey,
      lamports
    );
    await connection.confirmTransaction(airdropSignature);
    return account;
  } catch (e) {
    // tslint:disable:no-console
    throw new Error(`Airdrop of ${lamports} failed`);
  }
}

export const getDeploymentInfo = () => {
  const data = fs.readFileSync("../../last-deploy.json", "utf-8");
  const deployInfo = JSON.parse(data);
  return {
    clusterUrl: deployInfo.clusterUrl,
    stableSwapProgramId: new PublicKey(deployInfo.swapProgramId),
  };
};
