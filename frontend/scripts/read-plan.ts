// Print the plan for an owner pubkey, using the same decoder as the page.
//   npx tsx scripts/read-plan.ts <owner-pubkey>
import { Connection, PublicKey } from "@solana/web3.js";
import { chainTime, fetchPlan } from "../src/kimida";

const owner = new PublicKey(process.argv[2]);
const conn = new Connection(process.env.RPC_URL ?? "https://api.devnet.solana.com", "confirmed");
const ctx = {
  programId: new PublicKey(process.env.PROGRAM_ID ?? "Egn9Kgqa8sLT2F5u5DS26Bg791eW51HfKKuqSSp8euke"),
  keeper: new PublicKey(process.env.KEEPER_PUBKEY ?? "AsEvw9nKTdCjW3hK3RiJtwNb2rNwaf8EYZmyRABCFGBM"),
};
const p = await fetchPlan(conn, ctx, owner);
const now = await chainTime(conn);
console.log(
  p && {
    status: p.status,
    beneficiary: p.beneficiary.toBase58(),
    interval: p.checkinInterval,
    grace: p.gracePeriod,
    leftToSign: p.signBy - now,
    toDeadline: p.deadline - now,
    nonce: p.nonceAccount.toBase58(),
  },
);
