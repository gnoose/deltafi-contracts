import { AccountInfo, PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import { blob, Layout, u8 } from "buffer-layout";

export type Parser<T> = (data: Buffer) => T | undefined;

export type AccountParser<T> = (
  pubkey: PublicKey,
  info: AccountInfo<Buffer>
) =>
  | {
      pubkey: PublicKey;
      info: AccountInfo<Buffer>;
      data: T;
    }
  | undefined;

/** @internal */
export interface EncodeDecode<T> {
  decode: (buffer: Buffer, offset?: number) => T;
  encode: (src: T, buffer: Buffer, offset?: number) => number;
}

/** @internal */
export const encodeDecode = <T>(layout: Layout<T>): EncodeDecode<T> => {
  const decode = layout.decode.bind(layout);
  const encode = layout.encode.bind(layout);
  return { decode, encode };
};

/** @internal */
export const bool = (property = "bool"): Layout<boolean> => {
  const layout = u8(property);
  const { encode, decode } = encodeDecode(layout);

  const boolLayout = (layout as Layout<unknown>) as Layout<boolean>;

  boolLayout.decode = (buffer: Buffer, offset: number) => {
    const src = decode(buffer, offset);
    return !!src;
  };

  boolLayout.encode = (bool: boolean, buffer: Buffer, offset: number) => {
    const src = Number(bool);
    return encode(src, buffer, offset);
  };

  return boolLayout;
};

/** @internal */
export const publicKey = (property = "publicKey"): Layout<PublicKey> => {
  const layout = blob(32, property);
  const { encode, decode } = encodeDecode(layout);

  const publicKeyLayout = (layout as Layout<unknown>) as Layout<PublicKey>;

  publicKeyLayout.decode = (buffer: Buffer, offset: number) => {
    const src = decode(buffer, offset);
    return new PublicKey(src);
  };

  publicKeyLayout.encode = (
    publicKey: PublicKey,
    buffer: Buffer,
    offset: number
  ) => {
    const src = publicKey.toBuffer();
    return encode(src, buffer, offset);
  };

  return publicKeyLayout;
};

/** @internal */
export const bigInt = (length: number) => (property = "bigInt"): Layout<BN> => {
  const layout = blob(length, property);
  const { encode, decode } = encodeDecode(layout);

  const bigIntLayout = (layout as Layout<unknown>) as Layout<BN>;

  bigIntLayout.decode = (buffer: Buffer, offset: number) => {
    const src = decode(buffer, offset);
    return new BN(src as Buffer, 10, "le");
  };

  bigIntLayout.encode = (bigInt: BN, buffer: Buffer, offset: number) => {
    const src = bigInt.toBuffer("le", length);
    return encode(src, buffer, offset);
  };

  return bigIntLayout;
};

/** @internal */
export const u64 = bigInt(8);

/** @internal */
export const u128 = bigInt(16);
