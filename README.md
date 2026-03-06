# Solana HFT Sniper – v4.2 (English)

> **TL;DR** – A low-latency Solana HFT sniper built on Triton "Dragon's Mouth" gRPC, Groq AI
> decision-making, Jito MEV-protected bundles and full AWS self-healing infrastructure.

[![CI/CD Pipeline](https://github.com/Timson100x/solana-hft-ultimate-v4-triton-/actions/workflows/main.yaml/badge.svg)](https://github.com/Timson100x/solana-hft-ultimate-v4-triton-/actions)

> 🇩🇪 Deutsche Version: [README_DE.md](README_DE.md)

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Quick-Start (One-Click Deploy)](#2-quick-start-one-click-deploy)
3. [Architecture & Folder Layout](#3-architecture--folder-layout)
4. [Key Components](#4-key-components)
5. [Triton February 2026 News](#5-triton-february-2026-news)
6. [Deploy on AWS (Terraform)](#6-deploy-on-aws-terraform)
7. [Local Development (Docker)](#7-local-development-docker)
8. [CI / CD (GitHub Actions)](#8-ci--cd-github-actions)
9. [Further Enhancements](#9-further-enhancements)
10. [License & Contributing](#10-license--contributing)

---

## 1. System Overview

The bot listens to **Solana's gRPC "Dragon's Mouth"** (Triton YellowStone) and receives
*intra-slot* updates (e.g. `SLOT_FIRST_SHRED`).

When a Pump.fun launch is detected:

1. **AI-Bridge** (Groq `llama-3.3-70b-versatile`) evaluates the launch metadata and replies
   **YES** or **NO**.
2. If **YES**, the bot builds a **v2-PDA** `buy` instruction (mandatory since Feb 2026).
3. The instruction is wrapped in a **Jito bundle** together with a **10 000 Lamports tip**
   → guaranteed top-of-block placement.
4. A Telegram notification is sent to the operator's phone.

All of this runs on an **AWS t4g.small** (Graviton2) instance in `eu-central-1` (Frankfurt),
co-located with Triton's Iron Mountain FRA-2 / Equinix FR4 nodes.

### Triton One Setup

| Parameter       | Value                                        |
|-----------------|----------------------------------------------|
| **Endpoint**    | `timmys-mainnet-e441.rpcpool.com:443`        |
| **Tier**        | Tier 3                                       |
| **DC**          | Frankfurt (Iron Mountain FRA-2 / Equinix FR4)|
| **Protocol**    | Dragon's Mouth gRPC                          |
| **Auth**        | `x-token` metadata header (NEVER in the URL)|
| **Keepalive**   | 30 s                                         |
| **Gzip**        | DISABLED (latency > bandwidth)               |
| **Max Msg**     | 64 MB                                        |
| **Shards**      | 1–4 Round-Robin (`AtomicUsize`)              |

---

## 2. Quick-Start (One-Click Deploy)

```bash
# 1. Clone repo
git clone https://github.com/Timson100x/solana-hft-ultimate-v4-triton-.git
cd solana-hft-ultimate-v4-triton-

# 2. Copy & fill .env (see .env.example)
cp .env.example .env
# edit .env → insert your Triton token, private key, Groq key, Telegram token etc.
chmod 600 .env

# 3. One-command setup (installs Rust, dependencies, builds the bot)
bash setup-v4.2-triton.sh
```

> ⚠️ **Never** commit your `.env` file. It is already in `.gitignore`.

### Required Secrets (`.env`)

| Variable             | Description                                  |
|----------------------|----------------------------------------------|
| `TRITON_X_TOKEN`     | Triton One API token (x-token header)        |
| `WALLET_PRIVATE_KEY` | Base58-encoded wallet private key            |
| `XAI_API_KEY`        | xAI / Grok API key for the AI bridge         |
| `HELIUS_API_KEY`     | Helius fallback RPC key                      |
| `TELEGRAM_BOT_TOKEN` | Telegram bot token for trade alerts          |
| `TELEGRAM_CHAT_ID`   | Your Telegram chat ID                        |

---

## 3. Architecture & Folder Layout

```
Triton Dragon's Mouth gRPC
    │
    ▼
Rust HFT Bot (4 Shards, Round-Robin)
    │
    ├── src/main.rs            Entry point, shard manager
    ├── src/triton.rs          Dragon's Mouth gRPC client
    ├── src/pump_tx.rs         Pump.fun v2 transactions + PDAs
    ├── src/jito_optimized.rs  Bundle builder + tip logic
    ├── src/rig_goat.rs        AI decision engine (Rig + Groq)
    └── src/monitor.rs         Telegram alerts + Prometheus metrics
    │
    ▼
Jito Block Engine (Frankfurt)
    │
    ▼
Solana Mainnet
```

### Folder layout

```
solana-hft-ultimate-v4-triton-/
├── src/
│   ├── main.rs
│   ├── triton.rs
│   ├── pump_tx.rs
│   ├── jito_optimized.rs
│   ├── rig_goat.rs
│   └── monitor.rs
├── .env.example
├── .gitignore
├── Cargo.toml
├── setup-v4.2-triton.sh
├── README.md          ← this file
└── README_DE.md       ← German version
```

---

## 4. Key Components

### 4a. gRPC Streaming – Dragon's Mouth

- Connects via `tonic` to `timmys-mainnet-e441.rpcpool.com:443`
- Auth via `x-token` metadata header (never embed the token in the URL)
- Keepalive ping every **30 s**, timeout **5 s**
- Gzip **disabled** (minimise latency)
- Max decoding message size: **64 MB**
- **4 shards** managed via `AtomicUsize` round-robin for load distribution

```rust
// Auth header – correct way
let mut metadata = tonic::metadata::MetadataMap::new();
metadata.insert("x-token", token.parse()?);
```

### 4b. Pump.fun v2 PDA Handling (Feb 2026)

> **Critical:** The new PDAs must always be appended to the **end** of the account list.

```rust
// bonding-curve-v2
let (bonding_curve_v2, _) = Pubkey::find_program_address(
    &[b"bonding-curve-v2", mint.as_ref()],
    &PUMP_PROGRAM_ID,
);

// pool-v2
let (pool_v2, _) = Pubkey::find_program_address(
    &[b"pool-v2", base_mint.as_ref()],
    &PUMP_PROGRAM_ID,
);
```

Additional changes in the Feb 2026 update:
- **Creator Rewards Sharing** enabled
- **Mayhem Mode**: 8 new fee-recipient addresses (strict order required)

### 4c. Groq AI "YES / NO" Filter

The AI bridge (module `rig_goat.rs`) calls `llama-3.3-70b-versatile` via the Rig framework:

```rust
let client = rig::providers::xai::Client::from_env(); // reads XAI_API_KEY
let decision = client.completion("Should I snipe this token? YES or NO").await?;
```

Only tokens that receive a **YES** verdict are bought. This filters rug-pulls and low-quality
launches before any on-chain transaction is sent.

### 4d. Jito Bundle + Tip

```rust
const JITO_TIP_LAMPORTS: u64 = 10_000; // minimum; adjust dynamically
// Block engine: frankfurt.mainnet.block-engine.jito.wtf
// Max 5 transactions per bundle
// Poll bundle status after 30 s
```

Benefits:
- **MEV protection** – your buy cannot be sandwiched
- **Top-of-block** placement → first buyer in the slot
- Atomic: either the whole bundle lands or none of it does

### 4e. Monitoring & Self-Healing

| Tool        | Port  | Purpose                               |
|-------------|-------|---------------------------------------|
| Telegram    | —     | Instant trade alerts (buy/sell/error) |
| Prometheus  | 9091  | Metrics: latency, PnL, bundle status  |
| Grafana     | 3000  | Live dashboard                        |

The AWS Auto Scaling group uses **Predictive Scaling** based on network latency to spin up
additional capacity before peak activity.

### Trading Parameters

| Parameter          | Default | Description                  |
|--------------------|---------|------------------------------|
| `MAX_SOL_PER_TRADE`| 0.1 SOL | Maximum stake per trade      |
| `SLIPPAGE_BPS`     | 500     | 5 % slippage tolerance       |
| `MIN_LIQUIDITY_SOL`| 5.0 SOL | Minimum pool liquidity       |
| `TAKE_PROFIT_PCT`  | 50 %    | Auto-exit at +50 %           |
| `STOP_LOSS_PCT`    | 20 %    | Auto-exit at −20 %           |

---

## 5. Triton February 2026 News

> **Why it matters for this bot**

Triton published several important changes in February 2026:

| Change | Impact |
|--------|--------|
| **Dragon's Mouth** remains the only recommended gRPC stream for HFT | We use it exclusively; no migration needed |
| **Fumarole** officially flagged as too slow for latency-sensitive bots | Removed from fallback logic |
| **Vixen hosted** deprecated | Not used |
| **YellowStone v2 protocol** – `SLOT_FIRST_SHRED` event now stable | Enables even earlier entry signals |
| New **Frankfurt colocation** nodes added (Iron Mountain FRA-2, Equinix FR4) | Our `eu-central-1` AWS instance now has sub-1 ms hop to the validator |

**Recommended settings post-update (already applied):**

```toml
keepalive_time_ms   = 30_000   # 30 s
keepalive_timeout   = 5_000    # 5 s
gzip_compression    = false    # latency > bandwidth
max_message_size    = 67_108_864  # 64 MB
```

---

## 6. Deploy on AWS (Terraform)

> Infrastructure as code lives in the `infra/` directory (coming soon – see
> [Further Enhancements](#9-further-enhancements)).

Manual setup reference:

```bash
# Region: eu-central-1 (Frankfurt)
# Instance: t4g.small – ARM Graviton2 (~$13/month 24/7)
# AMI: Ubuntu 22.04 ARM64
# Security Group:
#   - SSH (22)         → your IP only
#   - Prometheus (9091)→ your IP only
#   - Grafana (3000)   → your IP only

# IAM Role permissions:
#   - secretsmanager:GetSecretValue
#   - cloudwatch:PutMetricData

# Store secrets in AWS Secrets Manager:
#   TRITON_X_TOKEN, WALLET_PRIVATE_KEY, XAI_API_KEY
#   HELIUS_API_KEY, TELEGRAM_BOT_TOKEN, TELEGRAM_CHAT_ID
```

Required GitHub repository secrets for CI/CD:

| Secret               | Used for                              |
|----------------------|---------------------------------------|
| `TRITON_X_TOKEN`     | gRPC authentication                   |
| `WALLET_PRIVATE_KEY` | On-chain transaction signing          |
| `XAI_API_KEY`        | Groq / xAI AI bridge                  |
| `HELIUS_API_KEY`     | Fallback RPC                          |
| `TELEGRAM_BOT_TOKEN` | Trade notifications                   |
| `TELEGRAM_CHAT_ID`   | Trade notifications                   |

---

## 7. Local Development (Docker)

```bash
# Build the Docker image
docker build -t solana-hft:v4.2 .

# Run with your .env file
docker run --env-file .env solana-hft:v4.2

# Or with docker-compose (includes Prometheus + Grafana)
docker-compose up -d
```

> A `Dockerfile` and `docker-compose.yml` are planned – see
> [Further Enhancements](#9-further-enhancements).

---

## 8. CI / CD (GitHub Actions)

The workflow at `.github/workflows/main.yaml` runs on every push / PR to `main`:

1. **Checkout** the repository
2. **Set up Rust** (stable toolchain)
3. **`cargo build --release`** – fails the pipeline on compile errors
4. **`cargo test`** – runs the full test suite

All secrets are injected from GitHub repository secrets and never logged or committed.

---

## 9. Further Enhancements

| Enhancement | Status | Notes |
|-------------|--------|-------|
| Terraform `infra/` module | 🔜 Planned | Full IaC for one-click AWS deploy |
| Dockerfile + docker-compose | 🔜 Planned | Local dev & staging environment |
| Kelly Criterion position sizing | 🔜 Planned | Dynamic `MAX_SOL_PER_TRADE` based on win-rate |
| Multi-relay Jito routing | 🔜 Planned | Round-robin across Frankfurt, Amsterdam, NY |
| Grafana dashboard JSON | 🔜 Planned | Pre-built panels for latency, PnL, bundle hit-rate |
| WebSocket fallback | 🔜 Planned | Helius WS as Dragon's Mouth backup |
| Stop-loss trailing | 🔜 Planned | Dynamic stop-loss that follows price up |

---

## 10. License & Contributing

This project is for **educational and personal use only**. Trading crypto carries significant
financial risk. Use at your own risk.

**Security rules:**
- Never commit `.env`, private keys, or tokens to Git
- All secrets must go through **AWS Secrets Manager** or **GitHub Secrets**
- Security Group: restrict all ports to your own IP

Pull requests and issues are welcome. Please open an issue before submitting large changes.

---

*Created: March 2026 | Düsseldorf | Timmy*
