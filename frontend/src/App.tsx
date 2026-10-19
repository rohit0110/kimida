import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useConnection, useWallet } from "@solana/wallet-adapter-react";
import { useWalletModal } from "@solana/wallet-adapter-react-ui";
import { PublicKey } from "@solana/web3.js";

import { CLUSTER_LABEL, GRACE_SECONDS, KEEPER, MIN_INTERVAL_SECONDS, PROGRAM_ID } from "./config";
import { explain, type Explained } from "./errors";
import {
  chainTime,
  checkIn,
  closePlan,
  createPlan,
  createPlanCost,
  fetchPlan,
  type OwnerWallet,
  type Plan,
} from "./kimida";

const ctx = { programId: PROGRAM_ID, keeper: KEEPER };

const UNITS = [
  { label: "Seconds", secs: 1 },
  { label: "Minutes", secs: 60 },
  { label: "Hours", secs: 3600 },
  { label: "Days", secs: 86400 },
] as const;

function fmt(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const parts = [d && `${d}d`, (d || h) && `${h}h`, (d || h || m) && `${m}m`, `${s % 60}s`];
  return parts.filter(Boolean).join(" ");
}

function short(pk: PublicKey): string {
  const s = pk.toBase58();
  return `${s.slice(0, 4)}…${s.slice(-4)}`;
}

const sol = (lamports: number) => (lamports / 1e9).toFixed(4);

export default function App() {
  const { connection } = useConnection();
  const { publicKey, signTransaction, wallet: selected, connected, connecting, connect, disconnect, select } = useWallet();
  const { visible, setVisible } = useWalletModal();

  const [plan, setPlan] = useState<Plan | null | undefined>(undefined); // undefined = loading
  const [skew, setSkew] = useState(0); // on-chain clock minus local clock, seconds
  const [now, setNow] = useState(() => Date.now() / 1000);
  const [balance, setBalance] = useState<number | null>(null);
  const [cost, setCost] = useState<number | null>(null);
  const [beneficiary, setBeneficiary] = useState("");
  const [amount, setAmount] = useState("30");
  const [unit, setUnit] = useState<number>(1);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<Explained | null>(null);

  // Without autoConnect, picking a wallet in the modal only selects it. Connect right after the pick,
  // but only if the user opened the modal themselves (never on page load).
  const [wantConnect, setWantConnect] = useState(false);
  useEffect(() => {
    if (visible && !selected) setWantConnect(true);
  }, [visible, selected]);
  useEffect(() => {
    if (!wantConnect || !selected || connected || connecting) return;
    setWantConnect(false);
    connect().catch((e) => setError(explain(e)));
  }, [wantConnect, selected, connected, connecting, connect]);

  function onConnectClick() {
    if (selected) connect().catch((e) => setError(explain(e)));
    else setVisible(true);
  }

  const [menuOpen, setMenuOpen] = useState(false);
  async function changeWallet() {
    setMenuOpen(false);
    try {
      await disconnect(); // ask the wallet to drop this site first, so the next connect can prompt again
    } catch {
      /* already disconnected */
    }
    select(null); // forget the remembered wallet so the picker opens fresh and connects on pick
    setVisible(true);
  }
  async function disconnectWallet() {
    setMenuOpen(false);
    await disconnect().catch((e) => setError(explain(e)));
  }

  const wallet: OwnerWallet | null = useMemo(
    () => (publicKey && signTransaction ? { publicKey, signTransaction } : null),
    [publicKey, signTransaction],
  );

  const current = useRef<string | null>(null);
  current.current = publicKey?.toBase58() ?? null;

  const refresh = useCallback(async () => {
    const asked = publicKey?.toBase58() ?? null;
    if (!publicKey) {
      setPlan(undefined);
      setBalance(null);
      return;
    }
    try {
      const [p, chain, bal, need] = await Promise.all([
        fetchPlan(connection, ctx, publicKey),
        chainTime(connection),
        connection.getBalance(publicKey, "confirmed"),
        createPlanCost(connection),
      ]);
      if (current.current !== asked) return; // the account changed while this was in flight
      setPlan(p);
      setSkew(chain - Date.now() / 1000);
      setBalance(bal);
      setCost(need);
    } catch (e) {
      setError(explain(e));
    }
  }, [connection, publicKey]);

  useEffect(() => {
    // New account (connect, disconnect, or a profile switch in the wallet): drop everything tied to the old one.
    setPlan(undefined);
    setBalance(null);
    setCost(null);
    setBeneficiary("");
    setError(null);
    void refresh();
    const t = setInterval(() => void refresh(), 10_000);
    return () => clearInterval(t);
  }, [refresh]);

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => clearInterval(t);
  }, []);

  const benefKey = useMemo(() => {
    try {
      return beneficiary.trim() ? new PublicKey(beneficiary.trim()) : null;
    } catch {
      return null;
    }
  }, [beneficiary]);
  const intervalSec = Math.round(Number(amount) * UNITS[unit].secs);

  // One message per problem with the form, shown under the field it belongs to.
  const benefProblem = beneficiary.trim() && !benefKey ? "This is not a valid Solana address." : null;
  const periodProblem = !Number.isFinite(intervalSec) || intervalSec <= 0
    ? "Enter a period greater than zero."
    : intervalSec < MIN_INTERVAL_SECONDS
      ? `The shortest period this deployment accepts is ${fmt(MIN_INTERVAL_SECONDS)}.`
      : null;
  const fundsProblem =
    balance !== null && cost !== null && balance < cost
      ? `You need about ${sol(cost)} SOL to open a plan (most of it a refundable deposit). This wallet has ${sol(balance)} SOL.`
      : null;
  const formOk = !!benefKey && !periodProblem && !fundsProblem;

  async function run(label: string, fn: () => Promise<unknown>) {
    setBusy(label);
    setError(null);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(null);
    }
  }

  const chainNow = now + skew;

  return (
    <div className="shell">
      <header className="top">
        <span className="brand">
          KimiDa
          <span className="kanji" lang="ja">
            君だ
          </span>
        </span>
        <div className="top-right">
          <span className="net">{CLUSTER_LABEL}</span>
          {publicKey ? (
            <div className="menu">
              <button className="outline mono" onClick={() => setMenuOpen((o) => !o)} aria-expanded={menuOpen}>
                {short(publicKey)}
              </button>
              {menuOpen && (
                <div className="menu-list" role="menu">
                  <button onClick={() => void navigator.clipboard.writeText(publicKey.toBase58()).finally(() => setMenuOpen(false))}>
                    Copy address
                  </button>
                  <button onClick={() => void changeWallet()}>Change wallet</button>
                  <button onClick={() => void disconnectWallet()}>Disconnect</button>
                </div>
              )}
            </div>
          ) : (
            <button className="outline" disabled={connecting} onClick={onConnectClick}>
              {connecting ? "Connecting…" : "Connect"}
            </button>
          )}
        </div>
      </header>

      <main className="main">
        <section className="intro">
          <h1>Succession for your Solana wallet</h1>
          <p>
            Name a beneficiary and a check-in period. Your assets stay in your own wallet. If you stop checking in,
            your SOL moves to your beneficiary.
          </p>
        </section>

        <section className="card" aria-live="polite">
          {!wallet && (
            <div className="stack">
              <div className="empty">Connect a wallet to create or manage a plan.</div>
              <button className="primary" disabled={connecting} onClick={onConnectClick}>
                {connecting ? "Connecting…" : "Connect wallet"}
              </button>
            </div>
          )}

          {wallet && plan === undefined && <div className="empty">Loading…</div>}

          {wallet && plan === null && (
            <div className="stack">
              <div className="field">
                <div className="field-head">
                  <label htmlFor="ben">Beneficiary</label>
                  {balance !== null && <span className="muted">Balance {sol(balance)} SOL</span>}
                </div>
                <input
                  id="ben"
                  placeholder="Wallet address"
                  value={beneficiary}
                  onChange={(e) => setBeneficiary(e.target.value)}
                  spellCheck={false}
                  autoComplete="off"
                />
                {benefProblem && <p className="problem">{benefProblem}</p>}
              </div>

              <div className="field">
                <label htmlFor="period">Check in every</label>
                <div className="row">
                  <input
                    id="period"
                    type="number"
                    min="1"
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                  />
                  <select value={unit} onChange={(e) => setUnit(Number(e.target.value))} aria-label="Unit">
                    {UNITS.map((u, i) => (
                      <option key={u.label} value={i}>
                        {u.label}
                      </option>
                    ))}
                  </select>
                </div>
                {periodProblem && <p className="problem">{periodProblem}</p>}
              </div>

              <dl className="summary">
                <div>
                  <dt>Grace period</dt>
                  <dd>{fmt(GRACE_SECONDS)}</dd>
                </div>
                <div>
                  <dt>Refundable deposit</dt>
                  <dd>{cost !== null ? `~${sol(cost)} SOL` : "…"}</dd>
                </div>
                <div>
                  <dt>Wallet approvals</dt>
                  <dd>3</dd>
                </div>
              </dl>

              {fundsProblem && <p className="problem">{fundsProblem}</p>}

              <button
                className="primary"
                disabled={!formOk || !!busy}
                onClick={() =>
                  run("Creating plan, approve 3 prompts in your wallet…", () =>
                    createPlan(connection, ctx, wallet, benefKey!, intervalSec, GRACE_SECONDS),
                  )
                }
              >
                {busy ?? "Create plan"}
              </button>
            </div>
          )}

          {wallet && plan && plan.status === "Active" && (
            <div className="stack">
              <dl className="summary">
                <div>
                  <dt>Beneficiary</dt>
                  <dd className="mono" title={plan.beneficiary.toBase58()}>
                    {short(plan.beneficiary)}
                  </dd>
                </div>
                <div>
                  <dt>Check in every</dt>
                  <dd>{fmt(plan.checkinInterval)}</dd>
                </div>
              </dl>

              <div className="countdown">
                {chainNow < plan.signBy ? (
                  <>
                    <span className="muted">Time left to check in</span>
                    <strong>{fmt(plan.signBy - chainNow)}</strong>
                  </>
                ) : chainNow < plan.deadline ? (
                  <>
                    <span className="warn">Overdue. Grace period left</span>
                    <strong className="warn">{fmt(plan.deadline - chainNow)}</strong>
                  </>
                ) : (
                  <>
                    <span className="warn">Deadline passed</span>
                    <strong className="warn small">The plan can execute at any moment</strong>
                  </>
                )}
              </div>

              <button
                className="primary"
                disabled={!!busy}
                onClick={() => run("Checking in, approve in your wallet…", () => checkIn(connection, ctx, wallet, plan))}
              >
                {busy ?? "Check in"}
              </button>
              <button
                className="ghost"
                disabled={!!busy}
                onClick={() => {
                  if (window.confirm("Cancel this plan? Your beneficiary will no longer receive anything from it.")) {
                    void run("Cancelling…", () => closePlan(connection, ctx, wallet));
                  }
                }}
              >
                Cancel plan
              </button>
            </div>
          )}

          {wallet && plan && plan.status === "Distributed" && (
            <div className="stack">
              <div className="empty">
                This plan has executed. Your balance was sent to <b className="mono">{short(plan.beneficiary)}</b>.
              </div>
              <button className="primary" disabled={!!busy} onClick={() => run("Closing…", () => closePlan(connection, ctx, wallet))}>
                {busy ?? "Start a new plan"}
              </button>
            </div>
          )}
        </section>

        {error && (
          <div className="alert" role="alert">
            <div>
              <strong>{error.title}</strong>
              <p>{error.detail}</p>
            </div>
            <button className="dismiss" onClick={() => setError(null)} aria-label="Dismiss">
              ×
            </button>
          </div>
        )}

        <section className="steps">
          <div>
            <span className="num">01</span>
            <h2>Set it up</h2>
            <p>Pick a beneficiary and a period. You approve a pre-signed transfer once.</p>
          </div>
          <div>
            <span className="num">02</span>
            <h2>Check in</h2>
            <p>Each check-in resets the timer. Your funds never leave your wallet.</p>
          </div>
          <div>
            <span className="num">03</span>
            <h2>It executes</h2>
            <p>After the deadline passes, the keeper completes the transfer to your beneficiary.</p>
          </div>
        </section>
      </main>

      <footer className="foot">
        <span>Not audited. {CLUSTER_LABEL} demo.</span>
        <a href={`https://explorer.solana.com/address/${PROGRAM_ID.toBase58()}?cluster=devnet`} target="_blank" rel="noreferrer">
          Program {short(PROGRAM_ID)}
        </a>
      </footer>
    </div>
  );
}
