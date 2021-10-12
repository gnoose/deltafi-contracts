import { PublicKey, Connection, AccountInfo } from "@solana/web3.js";

export const loadAccount = async (
  connection: Connection,
  address: PublicKey,
  programId: PublicKey
): Promise<AccountInfo<Buffer>> => {
  const accountInfo = await connection.getAccountInfo(address);
  if (accountInfo === null) {
    throw new Error("Failed to find account");
  }

  if (!accountInfo.owner.equals(programId)) {
    throw new Error(
      `Invalid owner: expected ${programId.toBase58()}, found ${accountInfo.owner.toBase58()}`
    );
  }

  return accountInfo;
};

export const getMinBalanceRentForExempt = async (
  connection: Connection,
  dataLength: number
): Promise<number> => {
  return await connection.getMinimumBalanceForRentExemption(dataLength);
};
