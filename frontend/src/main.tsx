import { Buffer } from "buffer";
(window as unknown as { Buffer: typeof Buffer }).Buffer = Buffer; // web3.js expects a global Buffer

import React from "react";
import ReactDOM from "react-dom/client";
import { ConnectionProvider, WalletProvider } from "@solana/wallet-adapter-react";
import { WalletModalProvider } from "@solana/wallet-adapter-react-ui";
import "@solana/wallet-adapter-react-ui/styles.css";

import App from "./App";
import { RPC_URL } from "./config";
import "./styles.css";

// No autoConnect: the adapter would silently reconnect to a previously approved wallet on load.
// Wallet Standard wallets (Phantom, Solflare, Backpack, ...) are detected automatically.
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ConnectionProvider endpoint={RPC_URL}>
      <WalletProvider wallets={[]}>
        <WalletModalProvider>
          <App />
        </WalletModalProvider>
      </WalletProvider>
    </ConnectionProvider>
  </React.StrictMode>,
);
