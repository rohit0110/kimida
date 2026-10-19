// Browser-side port of anchor/client (owner side). No framework imports and no import.meta, so the
// same file runs in the page and in scripts/devnet-smoke.ts.
//
// Instruction discriminators come from anchor/target/idl/kimida.json.

import {
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";

import { KimiDaUserError } from "./errors";

export interface Ctx {
  programId: PublicKey;
  keeper: PublicKey;
}

/** Minimal wallet shape (matches @solana/wallet-adapter-react's useWallet()). */
export interface OwnerWallet {
  publicKey: PublicKey;
  signTransaction: (tx: Transaction) => Promise<Transaction>;
}

const DISC = {
  initializePlan: Uint8Array.from([207, 161, 230, 194, 86, 77, 169, 8]),
  checkIn: Uint8Array.from([209, 253, 4, 217, 250, 241, 207, 50]),
  heirSweep: Uint8Array.from([236, 111, 107, 122, 133, 152, 28, 231]),
  closePlan: Uint8Array.from([45, 137, 184, 220, 162, 253, 161, 8]),
};

const NONCE_ACCOUNT_LEN = 80;
const MAX_SWEEP_TX_LEN = 1232;

export type PlanStatus = "Active" | "Distributed";

export interface Plan {
  owner: PublicKey;
  beneficiary: PublicKey;
  keeper: PublicKey;
  nonceAccount: PublicKey;
  checkinInterval: number;
  gracePeriod: number;
  lastCheckin: number;
  status: PlanStatus;
  /** The stored owner-signed partial sweep (wire bytes). */
  sweepTx: Buffer;
  /** last_checkin + interval, i.e. when the owner must have signed again. */
  signBy: number;
  /** last_checkin + interval + grace, i.e. when the keeper may execute the sweep. */
  deadline: number;
}

export function planPda(ctx: Ctx, owner: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("plan"), owner.toBuffer()], ctx.programId)[0];
}

// ---- encoding helpers (Borsh) ----

function i64(n: number): Buffer {
  const b = Buffer.alloc(8);
  b.writeBigInt64LE(BigInt(n));
  return b;
}

function vecU8(bytes: Uint8Array): Buffer {
  const len = Buffer.alloc(4);
  len.writeUInt32LE(bytes.length);
  return Buffer.concat([len, bytes]);
}

// ---- reads ----

/** Plan layout (REQ25): discriminator(8) owner beneficiary keeper nonce | i64 x3 | status | bump | vec. */
export function decodePlan(data: Buffer): Plan {
  const pk = (o: number) => new PublicKey(data.subarray(o, o + 32));
  const n = (o: number) => Number(data.readBigInt64LE(o));
  const checkinInterval = n(136);
  const gracePeriod = n(144);
  const lastCheckin = n(152);
  const sweepLen = data.readUInt32LE(162);
  return {
    owner: pk(8),
    beneficiary: pk(40),
    keeper: pk(72),
    nonceAccount: pk(104),
    checkinInterval,
    gracePeriod,
    lastCheckin,
    status: data[160] === 0 ? "Active" : "Distributed",
    sweepTx: Buffer.from(data.subarray(166, 166 + sweepLen)),
    signBy: lastCheckin + checkinInterval,
    deadline: lastCheckin + checkinInterval + gracePeriod,
  };
}

export async function fetchPlan(conn: Connection, ctx: Ctx, owner: PublicKey): Promise<Plan | null> {
  const acc = await conn.getAccountInfo(planPda(ctx, owner), "confirmed");
  return acc ? decodePlan(acc.data) : null;
}

/** On-chain Clock unix time, the clock the program's deadline check uses. */
export async function chainTime(conn: Connection): Promise<number> {
  const acc = await conn.getAccountInfo(SYSVAR_CLOCK_PUBKEY, "confirmed");
  if (!acc) throw new KimiDaUserError("Cannot read network time", "The clock account could not be fetched from the RPC.");
  return Number(acc.data.readBigInt64LE(32));
}

async function readNonce(conn: Connection, nonce: PublicKey) {
  const acc = await conn.getAccountInfo(nonce, "confirmed");
  if (!acc) throw new KimiDaUserError("Nonce account not found", "The nonce account for this plan does not exist on this network.");
  const d = acc.data;
  if (d.length < NONCE_ACCOUNT_LEN || d.readUInt32LE(0) !== 1 || d.readUInt32LE(4) !== 1) {
    throw new KimiDaUserError("Nonce account is not valid", "It is uninitialised or an old-format nonce, which cannot be used.");
  }
  return {
    authority: new PublicKey(d.subarray(8, 40)),
    blockhash: new PublicKey(d.subarray(40, 72)).toBase58(), // durable nonce value, base58 like a blockhash
  };
}

// ---- transactions ----

async function sendSigned(conn: Connection, tx: Transaction): Promise<string> {
  const sig = await conn.sendRawTransaction(tx.serialize(), { skipPreflight: false });
  const res = await conn.confirmTransaction(sig, "confirmed");
  if (res.value.err) throw new Error(`transaction failed: ${JSON.stringify(res.value.err)}`); // explain() reads the Custom code
  return sig;
}

/** Owner creates the durable nonce account with authority = keeper (REQ21). */
async function createNonceAccount(conn: Connection, ctx: Ctx, w: OwnerWallet): Promise<PublicKey> {
  const nonce = Keypair.generate();
  const lamports = await conn.getMinimumBalanceForRentExemption(NONCE_ACCOUNT_LEN);
  const tx = new Transaction().add(
    ...SystemProgram.createNonceAccount({
      fromPubkey: w.publicKey,
      noncePubkey: nonce.publicKey,
      authorizedPubkey: ctx.keeper,
      lamports,
    }).instructions,
  );
  tx.feePayer = w.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash("confirmed")).blockhash;
  const signed = await w.signTransaction(tx); // wallet first, then the extra signer
  signed.partialSign(nonce);
  await sendSigned(conn, signed);
  return nonce.publicKey;
}

/**
 * REQ22: [AdvanceNonceAccount, heir_sweep], blockhash = durable nonce, fee payer = keeper,
 * signed by the owner only. The keeper's signature slot stays empty. Returns wire bytes.
 */
async function buildPartialSweep(
  conn: Connection,
  ctx: Ctx,
  w: OwnerWallet,
  beneficiary: PublicKey,
  nonce: PublicKey,
): Promise<Buffer> {
  const info = await readNonce(conn, nonce);
  if (!info.authority.equals(ctx.keeper)) {
    throw new KimiDaUserError("Nonce account is not controlled by the keeper", "Create the plan again.");
  }

  const advance = SystemProgram.nonceAdvance({ noncePubkey: nonce, authorizedPubkey: ctx.keeper });
  const sweep = new TransactionInstruction({
    programId: ctx.programId,
    data: Buffer.from(DISC.heirSweep),
    keys: [
      { pubkey: w.publicKey, isSigner: true, isWritable: true },
      { pubkey: ctx.keeper, isSigner: true, isWritable: false },
      { pubkey: beneficiary, isSigner: false, isWritable: true },
      { pubkey: planPda(ctx, w.publicKey), isSigner: false, isWritable: true },
      { pubkey: nonce, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
  });

  // Compute-budget instructions go AFTER advance (a durable-nonce tx needs advance first). Wallets
  // inject their own priority-fee instructions at the front when none are present, which would
  // break the nonce, so we include ours up front to keep the wallet from touching the message.
  const tx = new Transaction().add(
    advance,
    ComputeBudgetProgram.setComputeUnitLimit({ units: 100_000 }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 1 }),
    sweep,
  );
  tx.feePayer = ctx.keeper;
  tx.recentBlockhash = info.blockhash;
  const before = tx.serializeMessage();
  const signed = await w.signTransaction(tx);
  if (!Buffer.from(signed.serializeMessage()).equals(before)) {
    throw new KimiDaUserError(
      "Your wallet changed the transaction",
      "It altered the pre-signed sweep (for example by adding priority-fee instructions). A modified sweep cannot execute, so nothing was created. Try another wallet or disable its automatic fee settings.",
    );
  }
  const bytes = signed.serialize({ requireAllSignatures: false, verifySignatures: false });
  if (bytes.length > MAX_SWEEP_TX_LEN) throw new KimiDaUserError("Pre-signed transaction is too large", `It is ${bytes.length} bytes; the limit is ${MAX_SWEEP_TX_LEN}.`);
  return bytes;
}

const PLAN_ACCOUNT_LEN = 8 + 32 * 4 + 8 * 3 + 1 + 1 + 4 + MAX_SWEEP_TX_LEN; // 8 + Plan::INIT_SPACE
const FEE_BUFFER = 20_000; // three transactions at 5,000 lamports, with headroom

/** Lamports the owner needs to create a plan: nonce rent + plan rent (both refundable) + fees. */
export async function createPlanCost(conn: Connection): Promise<number> {
  const [nonce, plan] = await Promise.all([
    conn.getMinimumBalanceForRentExemption(NONCE_ACCOUNT_LEN),
    conn.getMinimumBalanceForRentExemption(PLAN_ACCOUNT_LEN),
  ]);
  return nonce + plan + FEE_BUFFER;
}

/** UC1: create nonce, sign the partial sweep, initialize the plan. Three wallet prompts. */
export async function createPlan(
  conn: Connection,
  ctx: Ctx,
  w: OwnerWallet,
  beneficiary: PublicKey,
  intervalSec: number,
  graceSec: number,
): Promise<{ nonce: PublicKey; signature: string }> {
  const [need, have] = await Promise.all([createPlanCost(conn), conn.getBalance(w.publicKey, "confirmed")]);
  if (have < need) {
    throw new KimiDaUserError(
      "Not enough SOL in this wallet",
      `Creating a plan needs about ${(need / 1e9).toFixed(4)} SOL (most of it a refundable deposit); this wallet has ${(have / 1e9).toFixed(4)} SOL.`,
    );
  }
  const nonce = await createNonceAccount(conn, ctx, w);
  const sweepTx = await buildPartialSweep(conn, ctx, w, beneficiary, nonce);

  const ix = new TransactionInstruction({
    programId: ctx.programId,
    keys: [
      { pubkey: w.publicKey, isSigner: true, isWritable: true },
      { pubkey: planPda(ctx, w.publicKey), isSigner: false, isWritable: true },
      { pubkey: nonce, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: Buffer.concat([
      Buffer.from(DISC.initializePlan),
      beneficiary.toBuffer(),
      ctx.keeper.toBuffer(),
      i64(intervalSec),
      i64(graceSec),
      vecU8(sweepTx),
    ]),
  });
  const tx = new Transaction().add(ix);
  tx.feePayer = w.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash("confirmed")).blockhash;
  const signature = await sendSigned(conn, await w.signTransaction(tx));
  return { nonce, signature };
}

/**
 * UC2: reset the timer. One wallet prompt in the normal case.
 *
 * The partial sweep only depends on the nonce value, beneficiary and keeper, and ed25519 is
 * deterministic, so re-signing would produce the same bytes. We resend the stored bytes (the program
 * still overwrites `sweep_tx`, REQ07) and only re-sign if the nonce value has moved on.
 */
export async function checkIn(conn: Connection, ctx: Ctx, w: OwnerWallet, plan: Plan): Promise<string> {
  const stored = plan.sweepTx.length > 0 ? Transaction.from(plan.sweepTx) : null;
  const nonce = await readNonce(conn, plan.nonceAccount);
  // Reuse only a well-formed stored sweep: advance first, built on the nonce's current value.
  // A plan whose stored sweep was altered by a wallet is repaired by re-signing here.
  const wellFormed =
    !!stored &&
    stored.recentBlockhash === nonce.blockhash &&
    stored.instructions.length >= 2 &&
    stored.instructions[0].programId.equals(SystemProgram.programId) &&
    stored.instructions[0].data.readUInt32LE(0) === 4; // AdvanceNonceAccount
  const sweepTx =
    wellFormed
      ? plan.sweepTx
      : await buildPartialSweep(conn, ctx, w, plan.beneficiary, plan.nonceAccount);

  const ix = new TransactionInstruction({
    programId: ctx.programId,
    keys: [
      { pubkey: w.publicKey, isSigner: true, isWritable: false },
      { pubkey: planPda(ctx, w.publicKey), isSigner: false, isWritable: true },
    ],
    data: Buffer.concat([Buffer.from(DISC.checkIn), vecU8(sweepTx)]),
  });
  const tx = new Transaction().add(ix);
  tx.feePayer = w.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash("confirmed")).blockhash;
  return sendSigned(conn, await w.signTransaction(tx));
}

/**
 * UC4: close the plan (REQ15). Rent returns to the owner and the stored sweep is deleted; the keeper
 * then burns the nonce account (REQ16). Allowed while Active or Distributed. One wallet prompt.
 */
export async function closePlan(conn: Connection, ctx: Ctx, w: OwnerWallet): Promise<string> {
  const ix = new TransactionInstruction({
    programId: ctx.programId,
    keys: [
      { pubkey: w.publicKey, isSigner: true, isWritable: true },
      { pubkey: planPda(ctx, w.publicKey), isSigner: false, isWritable: true },
    ],
    data: Buffer.from(DISC.closePlan),
  });
  const tx = new Transaction().add(ix);
  tx.feePayer = w.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash("confirmed")).blockhash;
  return sendSigned(conn, await w.signTransaction(tx));
}
