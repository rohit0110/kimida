// Drives src/kimida.ts (the code the page uses) with a Keypair standing in for the wallet.
// Proves the browser-side encoding is accepted by the program and the Rust keeper.
//
//   npx tsx scripts/devnet-smoke.ts <owner-keypair.json> <beneficiary-pubkey> [intervalSec]
//
// Env (defaults = the devnet demo deployment): RPC_URL, PROGRAM_ID, KEEPER_PUBKEY, GRACE_SECONDS

import { readFileSync } from "node:fs";
import { ComputeBudgetProgram, Connection, Keypair, PublicKey, Transaction } from "@solana/web3.js";
import { chainTime, checkIn, closePlan, createPlan, fetchPlan, type OwnerWallet } from "../src/kimida";

const [keyPath, benArg, intervalArg] = process.argv.slice(2);
if (!keyPath || !benArg) {
  console.error("usage: tsx scripts/devnet-smoke.ts <owner-keypair.json> <beneficiary-pubkey> [intervalSec]");
  process.exit(1);
}

const conn = new Connection(process.env.RPC_URL ?? "https://api.devnet.solana.com", "confirmed");
const ctx = {
  programId: new PublicKey(process.env.PROGRAM_ID ?? "Egn9Kgqa8sLT2F5u5DS26Bg791eW51HfKKuqSSp8euke"),
  keeper: new PublicKey(process.env.KEEPER_PUBKEY ?? "AsEvw9nKTdCjW3hK3RiJtwNb2rNwaf8EYZmyRABCFGBM"),
};
const owner = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(keyPath, "utf8"))));
let prompts = 0; // each signTransaction call is one wallet prompt
const wallet: OwnerWallet = {
  publicKey: owner.publicKey,
  signTransaction: async (tx: Transaction) => {
    prompts++;
    if (process.env.MUTATE === "1" && tx.feePayer?.equals(ctx.keeper)) {
      // behave like a wallet that prepends priority-fee instructions
      tx.instructions.unshift(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 5 }));
    }
    tx.partialSign(owner);
    return tx;
  },
};
const beneficiary = new PublicKey(benArg);
const interval = Number(intervalArg ?? 40);
const grace = Number(process.env.GRACE_SECONDS ?? 10);

const show = async (label: string) => {
  const p = await fetchPlan(conn, ctx, owner.publicKey);
  const now = await chainTime(conn);
  console.log(
    label,
    p && {
      status: p.status,
      beneficiary: p.beneficiary.toBase58(),
      interval: p.checkinInterval,
      grace: p.gracePeriod,
      timeLeftToSign: p.signBy - now,
      secondsToDeadline: p.deadline - now,
    },
  );
};

console.log("owner", owner.publicKey.toBase58());
await show("before:");
const { nonce, signature } = await createPlan(conn, ctx, wallet, beneficiary, interval, grace);
console.log("created plan, nonce", nonce.toBase58(), "tx", signature, "| prompts:", prompts);
if (prompts !== 3) throw new Error(`expected 3 prompts to create, got ${prompts}`);
prompts = 0;
await show("after create:");

await new Promise((r) => setTimeout(r, 5000));
const plan = (await fetchPlan(conn, ctx, owner.publicKey))!;
const sig2 = await checkIn(conn, ctx, wallet, plan);
console.log("checked in, tx", sig2, "| prompts:", prompts);
if (prompts !== 1) throw new Error(`expected 1 prompt to check in, got ${prompts}`);
await show("after check-in:");

if (process.env.CLOSE === "1") {
  prompts = 0;
  const sig3 = await closePlan(conn, ctx, wallet);
  console.log("closed, tx", sig3, "| prompts:", prompts);
  await show("after close:");
}
