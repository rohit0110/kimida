// Turns wallet, RPC and program failures into a title and a plain-language detail the page can show.
// No framework imports: kimida.ts throws KimiDaUserError, and the page calls explain().

export interface Explained {
  title: string;
  detail: string;
}

/** An error whose message is already written for the user. */
export class KimiDaUserError extends Error {
  readonly title: string;
  constructor(title: string, detail: string) {
    super(detail);
    this.name = "KimiDaUserError";
    this.title = title;
  }
}

// Anchor custom errors start at 6000, in declaration order in programs/kimida/src/error.rs.
const PROGRAM_ERRORS: Record<number, Explained> = {
  6000: {
    title: "Check-in period out of range",
    detail: "The check-in period or the grace period is shorter or longer than this deployment allows.",
  },
  6001: {
    title: "Nonce account is not controlled by the keeper",
    detail: "The nonce account was created with a different authority. Create the plan again.",
  },
  6002: {
    title: "Pre-signed transaction is too large",
    detail: "The stored sweep transaction exceeds the 1,232 byte limit.",
  },
  6003: {
    title: "Plan is not active",
    detail: "This plan has already executed. Close it and start a new one.",
  },
  6004: { title: "Plan cannot be closed in its current state", detail: "The plan has an unexpected status." },
  6005: {
    title: "Sweep transaction is malformed",
    detail: "The first instruction must advance the nonce. Sign again to rebuild it.",
  },
  6006: {
    title: "Deadline has not passed",
    detail: "The plan cannot execute until the check-in period and grace period have both ended.",
  },
  6007: {
    title: "Beneficiary does not match the plan",
    detail: "The beneficiary or owner account differs from what the plan was created with.",
  },
  6008: {
    title: "Keeper or nonce does not match the plan",
    detail: "The keeper or nonce account differs from what the plan was created with.",
  },
  6009: { title: "Plan already executed", detail: "The balance has already been sent to the beneficiary." },
  6010: { title: "Invalid beneficiary", detail: "The beneficiary address cannot be the empty address." },
  6011: {
    title: "Nonce account is not valid",
    detail: "The nonce account is missing, uninitialised, or an old-format nonce that cannot be used.",
  },
};

function messageOf(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

function logsOf(e: unknown): string {
  const logs = (e as { logs?: unknown })?.logs;
  return Array.isArray(logs) ? logs.join("\n") : "";
}

export function explain(e: unknown): Explained {
  if (e instanceof KimiDaUserError) return { title: e.title, detail: e.message };

  const msg = messageOf(e);
  // Wallet adapters wrap the real cause in `.error`.
  const inner = (e as { error?: unknown })?.error;
  const text = `${msg}\n${inner ? messageOf(inner) : ""}\n${logsOf(e)}`;

  if (/user rejected|rejected the request|request rejected|declined|denied|cancell?ed|4001/i.test(text)) {
    return {
      title: "Request cancelled in your wallet",
      detail: "You closed or rejected the wallet prompt, so nothing was submitted.",
    };
  }

  if (/wallet not connected|WalletNotConnected|WalletNotReady|wallet.*not (ready|installed)/i.test(text)) {
    return { title: "Wallet not connected", detail: "Connect a wallet and try again." };
  }

  const custom = text.match(/"Custom":\s*(\d+)/) ?? text.match(/custom program error: 0x([0-9a-f]+)/i);
  if (custom) {
    const code = text.includes("custom program error") ? parseInt(custom[1], 16) : Number(custom[1]);
    if (PROGRAM_ERRORS[code]) return PROGRAM_ERRORS[code];
  }
  const anchorNum = text.match(/Error Number:\s*(\d+)/);
  if (anchorNum && PROGRAM_ERRORS[Number(anchorNum[1])]) return PROGRAM_ERRORS[Number(anchorNum[1])];

  if (/already in use|AccountAlreadyInUse|custom program error: 0x0\b/i.test(text)) {
    return {
      title: "This wallet already has a plan",
      detail: "Each wallet can hold one plan. Cancel the existing plan first, or use another wallet.",
    };
  }

  if (
    /insufficient (lamports|funds)|no record of a prior credit|insufficient funds for (rent|fee)|attempt to debit/i.test(
      text,
    )
  ) {
    return {
      title: "Not enough SOL in this wallet",
      detail: "The wallet cannot cover the network fee and the refundable account deposits. Add SOL and try again.",
    };
  }

  if (/blockhash not found|block height exceeded|has expired|TransactionExpired/i.test(text)) {
    return {
      title: "Transaction expired",
      detail: "The network did not confirm it in time, usually because a wallet prompt stayed open too long. Try again.",
    };
  }

  if (/nonce account not found/i.test(text)) {
    return {
      title: "Nonce account not found",
      detail: "The account this plan relies on no longer exists on this network.",
    };
  }

  if (/429|too many requests|rate limit/i.test(text)) {
    return {
      title: "Network is rate limiting requests",
      detail: "The public RPC endpoint is busy. Wait a few seconds and try again, or set a dedicated VITE_RPC_URL.",
    };
  }

  if (/failed to fetch|networkerror|fetch failed|ECONN|timeout|timed out|502|503|504/i.test(text)) {
    return {
      title: "Cannot reach the Solana network",
      detail: "The RPC endpoint did not respond. Check your connection and try again.",
    };
  }

  if (/simulation failed|transaction failed|SendTransactionError/i.test(text)) {
    return {
      title: "The network rejected the transaction",
      detail: logsOf(e).split("\n").slice(-2).join(" ").trim() || msg,
    };
  }

  return { title: "Something went wrong", detail: msg.length > 240 ? `${msg.slice(0, 240)}…` : msg };
}
