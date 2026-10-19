# Turbin3 Capstone — Deliverable 1: Letter of Intent & On-Chain Use Cases

**Project:** KimiDa — non-custodial digital succession for Solana wallets.

## CORE VALUE AND PROPOSITION

Idea is inheritance without giving up access to assets. Assets inside a wallet need to be passed
on to a dedicated account/multiple accounts declared by the user, user decides how often a checkin is performed
to verify user still lives, if user breaches that amount, a tx takes place which moves the assets from that account
to the beneficiar/ies.

Right now no product exists on Solana which allows transfer of assets without taking those assets into an escrow, or having a shared key-pair. Inheritance should be simple, A goes to B. Thats the solution I am aiming for.

## PMF AND COMPETITORS

The product is fit for everyone who wishes to pass on their crypto assets
Competitors like https://github.com/retrogtx/eternal-key depend on assets being locked in escrows. Making it unusable
for the person who intends to hands down the assets

that alone is a massive issue for anyone who wants to be able to enjoy their own assets as well

Its meant to target everydau users, and their families who they can give a separate wallet key to,
same public key can be added as beneficiary, the family doesnt need to be tech literate to be able to claim it
as that would happen behind the scenes, all they need is access to the beneficiary wallet

Shared key wallets also pose the issue of not being able to be in full control of the wallet and funds, as the beneficiary
is able to utilize it as well. (WHAT IF WALLET, WHERE SECOND LOG IN CANT WORK TILL DEAD MAN SWITCH TRIGGERS/ INACTIVITY OF X DAYS?)

## FMF
I am a generalist engineer, with 4 YoE as a professional. Most experienced in SRE, but have ample experience in Mobile App
Development, Full stack development and Devops, most notably working as a freelancer for Solana Mobile Demo applications.
Jumped into crypto April of 2025.
My motivaion to build this is, that common sense ideas and solutions should exist already without complications, if they dont, thats an issue and makes mass adoption unlikely

## POC

# The proposed mechanism (presigned durable-nonce transaction)

1. While alive, the owner creates a durable nonce account and signs one
   transaction, offline, right now: `[AdvanceNonceAccount, heir_sweep(beneficiary)]`.
   `heir_sweep` is a custom instruction with **no amount parameter** — it
   just names the owner's wallet and the beneficiary's wallet.
2. That signed transaction is held — by the beneficiary, by a relayer, or
   just published publicly (there's nothing secret in it). It never expires
   on its own: durable-nonce transactions check "does this tx's embedded
   nonce match what's currently in the nonce account," not blockhash
   freshness.
3. Months later, whoever holds the bytes submits it. The owner's signature
   is genuine — produced back when the owner was present — so `is_signer`
   is true for the owner's account, including inside the CPI.
4. Inside `heir_sweep`, the program reads `owner.lamports()` **live, at
   execution time**, then CPIs `system_program::transfer(owner, beneficiary, amount)`.
   This succeeds because the System Program sees a real signer.
5. A `Clock >= last_checkin + timeout` check inside the program gates the
   whole thing, same as any other claim path.

I didn't believe this should be possible, a presigned transaction with a
frozen amount can't work (the signature commits to the instruction data),
and "presign a blank, fill in the amount later" doesn't exist on Solana. But
this isn't that: no amount is in the instruction data at all. It's a
presigned *call*, not a presigned *value*, and the call computes its own
amount at execution time. So I built a POC.

## POC result: half right, and the half that's wrong is worse than expected

**→ [`poc-heir-sweep/`](poc-heir-sweep/) — full code, a real
`solana-test-validator` run, and the detailed writeup is in
[`poc-heir-sweep/README.md`](poc-heir-sweep/README.md).**

**Confirmed:** the mechanism works exactly as described. A durable-nonce tx
signed today, held untouched, submitted for the first time after the
timeout — with the owner's balance having *changed* in the meantime — sweeps
the live balance, not the balance that existed at signing time. No amount is
ever frozen. This part of the idea is real, and it corrects a claim in an
earlier draft of this document that flatly said presigning couldn't work.

**Found, empirically, on a live validator:** a single premature submission
attempt — by anyone holding the bytes, not just the beneficiary, and even
though it fails — permanently destroys the transaction. Solana's durable
nonce accounts advance on any failed attempt (unless the failure is in the
`AdvanceNonceAccount` instruction itself), specifically to stop free replay
attempts. That means the *one and only* valid copy of the owner's
authorization is a single-use bearer instrument: one bad-timed or malicious
early attempt, and it's gone forever, with the owner in no position to sign
a new one.

Compare this to the SPL `approve()` delegate model: a premature
`claim_delegated_assets` call just errors and changes nothing. You can retry
it forever, for free. The presigned-transaction model can't offer that
guarantee — the exact trick that lets it stay valid indefinitely (durable
nonce) is what makes one bad attempt unrecoverable. **That fragility, not
"you can't presign a variable amount," is the real reason to prefer an
on-chain delegated-authority / vault model over a bearer presigned
transaction for native SOL.**

## Where this leaves the design

Native SOL has no `approve()` equivalent, so *something* has to be captured
in advance for a beneficiary to move it later without the owner. Options,
still open:

- Accept the fragility, and mitigate it (route submission through a single
  trusted relay that checks the vault's on-chain gate before ever
  broadcasting — reduces the blast radius of a rogue holder, doesn't fully
  remove it).
- Fall back to the existing opt-in custodial vault path for native SOL
  specifically (owner deposits SOL into a PDA-owned vault account) — no
  presigned tx needed at all, retry-safe, at the cost of the owner giving up
  normal use of that SOL while alive.
- Something else — this is the open decision, not yet made.

Everything else that was in this document before (target market, competitor
table, actors/use-case tables, the SPL-delegate default path for
SPL/Token-2022, the red-team log) is still true and still the plan for those
asset types — it's just not repeated here because none of it changed. The
only thing this pass revisited is native SOL's claim mechanism, and that's
still unresolved pending the decision above.


