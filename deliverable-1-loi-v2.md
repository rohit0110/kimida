# Turbin3 Capstone, Deliverable 1: Letter of Intent & On-Chain Use Cases (Draft v2)

**Project:** KimiDa, non-custodial digital succession for Solana wallets.

## Overview

KimiDa is a non-custodial inheritance protocol for Solana wallets. It lets an owner name one or more beneficiaries and set a check-in interval that proves they are still active, without ever moving assets into an escrow or handing out a shared keypair while they are alive. If the owner misses their check-in window, an on-chain instruction sweeps the current balance from the owner's wallet to the named beneficiary or beneficiaries. The goal is to make crypto inheritance as simple as it should be: assets move from A to B when the trigger fires, with no added custody layer and no loss of control for the owner while they're still using their own funds.

## Part A: Define It Yourself

### 1. Core Value Proposition & Product-Market Fit (PMF)

The idea is inheritance without giving up access to your own assets. Assets sitting in a wallet need to be able to pass on to one or more accounts the owner declares up front. The owner decides how often they need to check in to prove they're still alive, and if that check-in window is breached, a transaction moves the assets from the owner's wallet to the beneficiary or beneficiaries. Right now no product on Solana lets this happen without either locking the assets into an escrow account or relying on a shared keypair between owner and beneficiary, and both of those force the owner to give up full control of their own funds while they're still alive and using them. Inheritance should be simple: A goes to B when the trigger fires. That's the problem KimiDa is solving.

**Coverage check, missing value drivers:**

- Cost and friction of the check-in itself. What does "checking in" actually require of the owner (a wallet signature, an app ping), and does that friction undercut the "simple" pitch.
- Partial estates. What happens when the owner wants to split assets unevenly across multiple beneficiaries, or leave different asset types to different people.
- Revocability. Can the owner cancel or change beneficiaries at any point before the trigger fires, and how fast does that take effect.
- Multi-asset coverage. Does this cover SOL only at first, or SPL tokens, NFTs, and staked positions too.
- False trigger risk. What happens if the owner is alive but misses a check-in (travel, lost device, hospital stay) and assets get swept prematurely.
- Legal and probate recognition. Does an on-chain transfer hold up against real-world inheritance law, or does it just sit alongside it, disconnected.

### 2. Target Markets & User Profiles

**Market segments:**

1. Long-term self-custody holders with meaningful balances who've been in crypto for years and hold outside exchanges.
2. Crypto-native families, where one person manages the wallet but a spouse, kids, or parents would need the assets someday.
3. Aging or health-risk holders (older individuals, or people in physically risky lines of work) who want continuity without setting up a formal will or trust for digital assets.
4. Retail solo holders with no legal will, who've left "inheritance planning" unset because existing options like a lawyer or trust don't handle crypto well.
5. DAOs and multisig treasury operators needing a signer-succession or key-continuity plan for a shared wallet. Secondary use case, not the initial focus.

**Top user profiles:**

1. **The Owner.** A self-custody Solana user, technical enough to manage their own wallet, wants continuity for their family without giving up day-to-day control.
2. **The Beneficiary.** Usually a family member, not necessarily crypto literate. Needs to be able to claim assets with minimal technical steps, just needing access to a wallet they were already given.
3. **The Relayer/Keeper.** A third party (the protocol itself, a keeper bot, or the beneficiary) that submits the trigger transaction once the check-in window is missed. Not a paying customer, but an operational actor the product depends on.

**Coverage check against real ecosystem demand:**

The pain point (self-custody assets becoming unreachable after death) is well documented, and the fact that at least three other projects (Eternal Key, Dead Man's Vault, Deadhand) are attacking this same problem is itself evidence that real demand exists. What isn't yet validated: whether users will adopt an added protocol and an added step for this versus doing nothing, since the status quo for most crypto holders is having no plan at all, and whether the "everyday user" segment is reachable at all without a distribution channel like a wallet integration or exchange partnership, since inheritance isn't a top-of-mind, everyday use case. Open item: talk to actual self-custody holders in Solana communities and on X to find out whether check-in friction or beneficiary-side UX is the real adoption blocker, since that's currently an assumption, not a finding.

### 3. Competitor Landscape

**Primary research:**

1. **Eternal Key** ([github.com/retrogtx/eternal-key](https://github.com/retrogtx/eternal-key)), Solana. Dead man's switch style, but assets go into an escrow account while the owner is alive.
2. **Dead Man's Vault / DMV** ([github.com/Romulus-Sol/DMV](https://github.com/Romulus-Sol/DMV)), Solana, built for Solana Seeker. Heartbeat-based liveness check with a 4-stage escalation warning, and permissionless, trustless execution once the deadline passes, meaning anyone (the app, a beneficiary, or a keeper) can submit the distribution and the program pays out only to the pre-set beneficiaries in pre-set shares. But it uses the same underlying pattern as Eternal Key: the owner deposits SOL, SPL tokens, or NFTs into a vault PDA up front, so the assets sit outside the owner's normal wallet the entire time they're alive.
3. **Deadhand Protocol** ([deadhandprotocol.com](https://www.deadhandprotocol.com/)), multi-chain including Solana. Doesn't use an on-chain vault at all. It splits the owner's seed phrase into three shards with Shamir's Secret Sharing, holds one shard on a server, and releases enough shards to reconstruct the key if the owner misses check-ins. This is cryptography-based key custody, not a smart-contract-native transfer, and it means the service holds a piece of the owner's key.
4. **Sarcophagus** (Ethereum plus Arweave, not Solana). Same dead man's switch category, DAO-run and VC-funded (raised $5.47M), but used more broadly for encrypted data and document release rather than a live wallet-balance sweep, and it isn't on Solana at all.

**AI coverage check and PMF adjustment:**

The original claim, "no product exists on Solana which allows transfer of assets without escrow or a shared keypair," doesn't fully hold once Dead Man's Vault is accounted for. DMV is the closest direct competitor. It is genuinely non-custodial in the sense that matters most, no third party such as a company or a human-controlled multisig ever holds a key that can move your funds, control sits entirely in program logic. But that's a different claim from "not an escrow or vault model." DMV still requires the owner to deposit SOL, SPL tokens, or NFTs into a program-owned vault PDA up front, and the owner cannot use those specific assets normally out of their everyday wallet while the switch is armed. That's the exact same objection the original PMF raised against Eternal Key, just without a human custodian in the loop. The accurate, defensible claim is narrower than what was originally written:

> No Solana product lets the owner keep using their assets completely normally, with no separate vault and no deposit step, while still guaranteeing an automatic, trustless transfer on death or incapacitation.

That's the actual gap KimiDa needs to hold, and it's the reason the live-balance-sweep mechanism (see the POC work) matters: it's the only approach found so far that avoids a deposit step entirely.

### 4. Founder-Market Fit (FMF)

I'm a generalist engineer with 4 years of professional experience. Most of that is in SRE, but I have solid experience across mobile app development, full-stack development, and DevOps, including freelance work building Solana Mobile demo applications. I got into crypto in April 2025. My motivation for building this is that common-sense ideas and solutions should already exist without unnecessary complication. When they don't, that's a real gap, and it makes mass adoption less likely.

## Part B: Adversarial Analysis & Refinement

### 1. Red Team Strategy

| # | Target | Attack point | Team assessment |
|---|--------|--------------|------------------|
| 1 | Value Prop | The check-in mechanism is a liveness oracle problem. A missed check-in from travel, hospitalization, a lost phone, or just forgetting isn't the same as death. Sweeping the owner's live wallet to a beneficiary while the owner is alive and just late is a failure mode that could be worse than the problem being solved. | **Valid, high priority.** Needs a grace period with staged warnings before the sweep executes, not a single hard deadline. DMV's 4-stage escalation is worth copying the pattern of. |
| 2 | Value Prop | Sweeping the *live* balance at execution time, rather than a fixed amount set at signing, means a beneficiary's actual inheritance is unpredictable, and a briefly compromised owner key near the trigger date could drain the wallet before the beneficiary ever claims. | **Partly valid.** This is a real tradeoff to disclose clearly to owners and beneficiaries, but it's the direct consequence of the design decision that lets the owner keep normal control while alive. Not a blocker, needs to be stated as an explicit product behavior rather than left implicit. |
| 3 | PMF | "No product exists on Solana which allows transfer without escrow or shared key" was proven false by a two-minute search that found Dead Man's Vault. Any pitch built on a "no competitor" claim is one search away from falling apart. | **Confirmed.** Already corrected in Part A #3 above with a narrower, defensible claim. |
| 4 | PMF | "Inheritance should be simple, A to B" is a slogan, not a validated need. No one's been asked whether check-in friction, beneficiary UX, or trust in a new, unaudited program is the actual barrier stopping people from planning this today, versus just not caring or not knowing a solution exists. | **Valid, unresolved.** Flagged as an open outreach item in the Part A #2 coverage check. |
| 5 | Market Segments | "Fit for everyone who wishes to pass on their crypto assets" is too broad to function as a market segment. "Everyone" isn't a distribution strategy or a beachhead. | **Confirmed.** Addressed by replacing it with the prioritized segment list in Part A #2. |
| 6 | Market Segments | "The family doesn't need to be tech literate" glosses over the fact that someone still has to get the beneficiary a wallet, explain a seed phrase, and explain that funds will appear automatically. That's a real onboarding and support burden. | **Partly valid.** A genuine UX and support design problem, not a fatal flaw in the model. |
| 7 | Competitors | Only one direct competitor (Eternal Key) had been named, from a single GitHub link, before this pass. Pitching or building without checking a raises tracker, a hackathon project explorer, and social chatter means building blind to at least three more live projects in the same niche. | **Confirmed.** Addressed by the research pass in Part A #3. Recommend a recurring quarterly check rather than a one-time list. |
| 8 | Competitors | Deadhand's server-assisted Shamir's Secret Sharing approach solves an adjacent problem (key custody and recovery) without any smart contract, no gas, no new program to trust. That's a legitimate substitute threat, not a "different chain, doesn't count" dismissal. | **Partly valid.** Worth tracking, but the trust model differs meaningfully (Deadhand holds a key shard on a server, which self-custody purists will reject outright), so it's a segment-dependent threat rather than a universal one. |

### 2. Refine Strategy

Updates to Phase 1 based on the valid points above:

- **Value prop:** add an explicit grace period with staged warnings before the sweep executes, instead of a single hard deadline, addressing finding 1.
- **Value prop:** state plainly that the swept amount is the live balance at execution time, not a fixed amount set at signing, so owners and beneficiaries both understand the tradeoff up front, addressing finding 2.
- **PMF:** narrow the claim from "no product exists" to "no product avoids a vault deposit while keeping non-custodial guarantees," addressing findings 3 and 5.
- **Market segments:** drop "everyone" as a stated market. Use the prioritized segment list from Part A #2, with long-term self-custody holders already active in Solana communities as the first beachhead.
- **Competitors:** keep a running list (Eternal Key, Dead Man's Vault, Deadhand, Sarcophagus) and recheck it quarterly against a raises tracker, hackathon project explorers, GitHub, and X, addressing finding 7.
- **Open item, not yet resolved:** run informal outreach in Solana communities and on X to test whether check-in friction or beneficiary UX is the actual adoption blocker, before building further, addressing finding 4.

### 3. Red Team FMF

**Attack points:**

- Four years of experience with an SRE focus and freelance mobile work is a generalist background, not a specialized Solana or Anchor security background. This protocol will hold or move other people's savings on a single missed check-in, and the founder only entered crypto in April 2025, under 18 months of crypto-specific experience as of this writing.
- The stated motivation ("common sense ideas should already exist") explains why the founder wants to build this, but doesn't establish a network or distribution advantage. There's no stated relationship with the Solana Foundation, existing wallets (Phantom, Solflare, Backpack), or estate-planning and legal partners, all of whom would actually be needed to reach the "everyday families" segment.
- The founder's own POC work already surfaced a real, subtle security flaw (durable-nonce single-use fragility) that wasn't anticipated going in. That's a point in favor of technical rigor once building, but it's also evidence of learning Solana's edge cases in public, on a product where an inheritance-moving bug is unusually unforgiving.

**Positioning refinements:**

- Lead with the hands-on infra and SRE background as a strength for a protocol where uptime and correctness under adversarial retry conditions matter. The POC's durable-nonce finding is proof of that rigor, not a liability, and should be framed that way.
- Be explicit that Solana-specific and Anchor-specific expertise is being actively built through this exact project, rather than claiming pre-existing expertise that isn't there.
- Treat the lack of network and distribution as an open gap to close before mainnet, not something to ignore. Plan to get in front of existing wallet teams or Solana community and DevRel channels for distribution, since that's currently the weakest link in the FMF.

## Phase 2: Rough Draft of First Possible Use Cases & On-Chain Requirements

Design note before the tables: a vault deposit is not part of this build. That's the whole point of the PMF in Part A, if the owner has to deposit SOL into a program-owned account up front, KimiDa is just Eternal Key or Dead Man's Vault with different branding. So native SOL uses the presigned durable-nonce mechanism validated in the Appendix POC, and SPL tokens use the delegate-approve model, neither one ever takes custody of the owner's assets while they're alive.

The tradeoff, documented in the Appendix, is that the presigned-tx path is a single-use bearer instrument: one premature submission attempt, by anyone holding the bytes, burns the nonce permanently with no retry. Since there's no vault to fall back on, this draft manages that risk operationally instead of by giving up the no-deposit property: custody of the signed bytes is restricted to a single trusted relay rather than published or handed to the beneficiary, the relay reads the Plan PDA's on-chain state and refuses to broadcast before the deadline, and every check-in rotates the nonce and has the owner re-sign a fresh copy, so old copies that leaked or floated around go stale on their own. None of this makes a premature submission impossible, it reduces who could ever attempt one and how long a given copy stays dangerous.

### Part A: Identify the Actors

**Direct Actors** (signers who invoke instructions):

- **Owner.** Creates the plan, sets up the durable nonce account for SOL, approves the program as a token delegate for SPL assets, checks in and re-signs a fresh presigned SOL sweep each cycle, updates beneficiaries, and can revoke or close the plan while it's still active. Never deposits assets anywhere, everything stays in the owner's own wallet and accounts.
- **Relay/Keeper.** A permissionless caller responsible for two things: holding the single copy of the owner's presigned SOL sweep transaction, and submitting it (along with advancing warning stages and submitting the token distribution) once the deadline has genuinely passed. Before ever broadcasting, it checks the Plan PDA's on-chain state itself as a safety gate. It signs transactions but never gains any value from them, and could be a bot, a cron job, or the beneficiary acting in that role for the token path (the beneficiary should not hold the raw SOL sweep bytes, to keep custody of that single-use artifact to one trusted party).
- **Protocol Admin.** A small multisig or governance PDA that owns global protocol configuration. Signs only for protocol-level actions like pausing new plan creation in an emergency.

**Beneficiaries** (gain value without directly signing):

- **Beneficiary.** Named by the owner, receives SOL or tokens once the plan's deadline passes and someone else submits the distribution instruction. Doesn't need to sign anything to receive SOL, and for tokens only needs an existing associated token account.

**Administrators:**

- **Protocol Config PDA.** Holds global parameters (minimum and maximum allowed check-in interval, pause flag). Not a person, but the account whose state gates certain instructions.

**Stakeholders** (external parties with interest, not on-chain signers):

- **Wallet providers** (Phantom, Solflare, Backpack). Distribution depends on these wallets surfacing check-in reminders and plan status to users, without their support adoption is limited to people who remember to use a separate app.
- **Estate planning and legal advisors.** Interested in whether an on-chain transfer holds up against real-world probate law, even though they never touch the chain.
- **Other legal heirs.** Family members not named as on-chain beneficiaries who may have a real-world legal claim that conflicts with an instant, automatic on-chain transfer.
- **Security researchers and auditors.** Have an interest in the correctness of the program given the POC already found one non-obvious fragility before mainnet.

### Part B: Design the Use Cases

Each use case below is one atomic state transition mapped to one Anchor instruction handler.

**1. Create Inheritance Plan**
- Handler: `initialize_plan`
- Actor: Owner (signer)
- Preconditions: no existing plan PDA for this owner.
- State transition: no plan account exists, to a new Plan PDA created holding the owner's pubkey, the check-in interval, the beneficiary list with share splits, `last_checkin` set to the current Clock timestamp, and status set to Active.

**2. Register Nonce Account**
- Handler: `register_nonce`
- Actor: Owner (signer)
- Preconditions: plan exists and is Active, signer matches `plan.owner`, the nonce account already exists (created separately via the standard System Program, nonce authority set to the owner) and holds only its own rent-exempt balance, no inheritance assets.
- State transition: the nonce account's pubkey is recorded on the Plan PDA, so `heir_sweep` can later verify it's being submitted against the expected nonce account rather than a substituted one. No SOL belonging to the owner's estate moves anywhere.

**3. Approve Token Delegate**
- Handler: `approve_token_delegate`
- Actor: Owner (signer)
- Preconditions: plan exists and is Active.
- State transition: the delegate field on the owner's own SPL token account is set to the Plan PDA, with a delegated amount equal to the balance enrolled. The tokens never leave the owner's own account, no vault deposit needed for this asset type.

**4. Check In**
- Handler: `check_in`
- Actor: Owner (signer)
- Preconditions: plan exists, signer matches `plan.owner`.
- State transition: `plan.last_checkin` updates to the current Clock timestamp. If the plan was in a warning stage, status resets back to Active. Alongside this on-chain call, the owner advances their own nonce account (they hold nonce authority) and re-signs a fresh presigned `heir_sweep` transaction for the relay. Since a stale copy's embedded nonce no longer matches, submitting it fails inside the `AdvanceNonceAccount` instruction itself, which per the POC's own finding is the one failure mode that does not burn anything, so old copies simply stop working rather than becoming a live risk.

**5. Update Beneficiaries**
- Handler: `update_beneficiaries`
- Actor: Owner (signer)
- Preconditions: plan is Active, deadline has not yet passed (owner keeps full control up until the final stage).
- State transition: the beneficiary list and share splits stored on the plan are replaced with the new set.

**6. Revoke and Close Plan**
- Handler: `close_plan`
- Actor: Owner (signer)
- Preconditions: plan is Active, deadline has not yet passed.
- State transition: any delegated token approval is reset to zero, the nonce account is closed and its rent reclaimed by the owner (who already holds nonce authority), the Plan PDA is closed, and its rent is reclaimed by the owner. There's no vault to unwind, since the owner's SOL and tokens never left their own accounts.

**7. Advance Warning Stage**
- Handler: `advance_warning_stage`
- Actor: Keeper/Relayer (signer, permissionless, anyone can call)
- Preconditions: `Clock >= plan.last_checkin + plan.checkin_interval + stage_offset`, and the plan's current stage is earlier than the stage being advanced to.
- State transition: `plan.status` moves from Active to Warning Stage N, or from one warning stage to the next. This borrows the staged-escalation pattern from Dead Man's Vault so a single missed check-in doesn't go straight to a sweep, addressing the false-negative risk raised in the Part B red team.

**8. Distribute SOL**
- Handler: `heir_sweep`
- Actor: Relay/Keeper (signer, submits the owner's presigned durable-nonce transaction); Beneficiary receives passively, no signature needed.
- Preconditions: the transaction is the owner's presigned `[AdvanceNonceAccount, heir_sweep(beneficiary)]`, the embedded nonce matches the registered nonce account's current value, `Clock` is past `plan.last_checkin + plan.checkin_interval` (plus any warning-stage offsets), plan has not already been distributed.
- State transition: `AdvanceNonceAccount` executes first, then `heir_sweep` reads the owner's live lamport balance and CPIs `system_program::transfer(owner, beneficiary, amount)` for that live amount, not a balance frozen at signing. `plan.status` becomes Distributed. If the deadline has not actually passed, this whole transaction fails and the nonce still advances, permanently invalidating this specific presigned copy, which is why the relay is expected to verify the deadline itself before ever broadcasting.

**9. Distribute Tokens**
- Handler: `distribute_tokens`
- Actor: Keeper/Relayer (signer, permissionless); Beneficiary receives passively into an existing associated token account.
- Preconditions: same deadline condition as use case 8, delegate approval on the owner's token account is still valid.
- State transition: tokens move from the owner's own token account to each beneficiary's token account via `transfer_checked`, using the Plan PDA's delegate authority. The amount is computed from the owner's live token balance at execution time, not a balance frozen at signing.

**10. Set Protocol Paused**
- Handler: `set_protocol_paused`
- Actor: Protocol Admin (signer)
- Preconditions: signer matches `protocol_config.admin`.
- State transition: `protocol_config.paused` flips true or false. While paused, `initialize_plan` and `register_nonce` are blocked for new plans, but check-ins, warning-stage advances, and distributions for plans already in flight are unaffected, so no existing plan gets stuck waiting on an admin action.

## Appendix: Technical Feasibility POC (supporting Phase 2 groundwork)

This section is exploratory technical validation done ahead of Phase 2, kept here for context. It isn't part of the Part A/B structure above.

### The proposed mechanism (presigned durable-nonce transaction)

1. While alive, the owner creates a durable nonce account and signs one transaction, offline, right now: `[AdvanceNonceAccount, heir_sweep(beneficiary)]`. `heir_sweep` is a custom instruction with **no amount parameter**, it just names the owner's wallet and the beneficiary's wallet.
2. That signed transaction is held, by the beneficiary, by a relayer, or just published publicly (there's nothing secret in it). It never expires on its own: durable-nonce transactions check whether the tx's embedded nonce matches what's currently in the nonce account, not blockhash freshness.
3. Months later, whoever holds the bytes submits it. The owner's signature is genuine, produced back when the owner was present, so `is_signer` is true for the owner's account, including inside the CPI.
4. Inside `heir_sweep`, the program reads `owner.lamports()` **live, at execution time**, then CPIs `system_program::transfer(owner, beneficiary, amount)`. This succeeds because the System Program sees a real signer.
5. A `Clock >= last_checkin + timeout` check inside the program gates the whole thing, same as any other claim path.

I didn't believe this should be possible. A presigned transaction with a frozen amount can't work (the signature commits to the instruction data), and "presign a blank, fill in the amount later" doesn't exist on Solana. But this isn't that: no amount is in the instruction data at all. It's a presigned *call*, not a presigned *value*, and the call computes its own amount at execution time. So I built a POC.

### POC result: half right, and the half that's wrong is worse than expected

**[`poc-heir-sweep/`](poc-heir-sweep/), full code, a real `solana-test-validator` run, and the detailed writeup is in [`poc-heir-sweep/README.md`](poc-heir-sweep/README.md).**

**Confirmed:** the mechanism works exactly as described. A durable-nonce tx signed today, held untouched, submitted for the first time after the timeout, with the owner's balance having *changed* in the meantime, sweeps the live balance, not the balance that existed at signing time. No amount is ever frozen. This part of the idea is real, and it corrects a claim in an earlier draft of this document that flatly said presigning couldn't work.

**Found, empirically, on a live validator:** a single premature submission attempt, by anyone holding the bytes, not just the beneficiary, and even though it fails, permanently destroys the transaction. Solana's durable nonce accounts advance on any failed attempt (unless the failure is in the `AdvanceNonceAccount` instruction itself), specifically to stop free replay attacks. That means the *one and only* valid copy of the owner's authorization is a single-use bearer instrument: one bad-timed or malicious early attempt, and it's gone forever, with the owner in no position to sign a new one.

Compare this to the SPL `approve()` delegate model: a premature `claim_delegated_assets` call just errors and changes nothing. You can retry it forever, for free. The presigned-transaction model can't offer that guarantee, the exact trick that lets it stay valid indefinitely (durable nonce) is what makes one bad attempt unrecoverable. **That fragility, not "you can't presign a variable amount," is the real reason to prefer an on-chain delegated-authority or vault model over a bearer presigned transaction for native SOL.**

### Where this leaves the design

Native SOL has no `approve()` equivalent, so *something* has to be captured in advance for a beneficiary to move it later without the owner. The decision, made for this draft: accept the fragility and mitigate it operationally, rather than fall back to a vault.

A vault deposit was considered and rejected. It's retry-safe and removes the fragility entirely, but it also removes the entire reason KimiDa exists: it would mean the owner gives up normal use of their SOL while alive, exactly the objection this project raised against Eternal Key and Dead Man's Vault in Part A #3. Adopting it would turn KimiDa into a third version of a product that already exists twice.

Instead, the mitigation is operational, laid out in the Phase 2 use cases above: custody of the presigned bytes is restricted to a single trusted relay instead of being published or handed to the beneficiary, the relay checks the Plan PDA's on-chain state before ever broadcasting, and every check-in has the owner advance their own nonce account and re-sign a fresh copy, which makes stale copies fail harmlessly inside `AdvanceNonceAccount` rather than sit around as a live risk. This narrows the danger window to between one check-in and the next, and to whoever the owner trusts as the relay, but it does not make a premature submission impossible. That residual risk is a real, disclosed tradeoff of avoiding a vault, not a solved problem.

Everything else that was in the prior draft (the SPL-delegate path for SPL/Token-2022) is still true and still the plan for those asset types, since delegate approval was never the problem, it's already retry-safe and deposit-free.

## Sources

- [Eternal Key](https://eternal-key.vercel.app/) / [GitHub](https://github.com/retrogtx/eternal-key)
- [Dead Man's Vault (DMV), GitHub](https://github.com/Romulus-Sol/DMV)
- [Deadhand Protocol](https://www.deadhandprotocol.com/)
- [Sarcophagus overview, Decrypt](https://decrypt.co/90032/crypto-dead-mans-switch-sarcophagus-raises-5-47m-from-vcs-via-dao)
