// Decode the stored partial sweep of a plan account and list its instructions.
//   npx tsx scripts/inspect-sweep.ts <plan-pda>
import { Connection, PublicKey, Transaction } from "@solana/web3.js";
import { decodePlan } from "../src/kimida";

const conn = new Connection(process.env.RPC_URL ?? "https://api.devnet.solana.com", "confirmed");
const acc = await conn.getAccountInfo(new PublicKey(process.argv[2]), "confirmed");
if (!acc) throw new Error("plan account not found");
const plan = decodePlan(acc.data);
console.log({ owner: plan.owner.toBase58(), beneficiary: plan.beneficiary.toBase58(), status: plan.status, sweepBytes: plan.sweepTx.length });
const tx = Transaction.from(plan.sweepTx);
console.log("feePayer", tx.feePayer?.toBase58(), "blockhash", tx.recentBlockhash);
tx.instructions.forEach((ix, i) =>
  console.log(i, ix.programId.toBase58(), "accounts:", ix.keys.length, "data bytes:", ix.data.length),
);
console.log("signatures:", tx.signatures.map((s) => `${s.publicKey.toBase58().slice(0, 6)}:${s.signature ? "signed" : "-"}`).join(" "));
