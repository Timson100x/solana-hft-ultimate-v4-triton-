# CLAUDE.md - Solana HFT Ultimate v4.2 Triton One

> Regeln und Kontext für Claude / AI-Assistenten bei der Arbeit in diesem Repo.

## Owner
- **Name:** Timmy (Düsseldorf, Deutschland)
- **Triton Account:** Tier 3, timmys-mainnet-e441.rpcpool.com
- **AWS Region:** eu-central-1 (Frankfurt)
- **Ziel:** Hochperformantes Pump.fun Sniping via Rust HFT + Jito Bundles

---

## Architektur

```
Triton Dragon's Mouth gRPC
    |
    v
Rust HFT Bot (4 Shards, Round-Robin)
    |
    +-- pump_tx.rs         (Pump.fun v2 Transaktionen)
    +-- jito_optimized.rs  (Bundle Builder + Tip)
    +-- rig_goat.rs        (AI Decision Engine)
    +-- monitor.rs         (Telegram Alerts)
    |
    v
Jito Block Engine (Frankfurt)
    |
    v
Solana Mainnet
```

---

## Triton Dragon's Mouth - Strikte Regeln

### MUSS so sein:
- **gRPC URL:** `timmys-mainnet-e441.rpcpool.com:443`
- **Auth:** IMMER als `x-token` Metadata-Header - NIEMALS in der URL!
- **Keepalive:** 30 Sekunden (`keepalive_time_ms: 30000`)
- **Keepalive Timeout:** 5 Sekunden
- **Gzip Komprimierung:** DEAKTIVIERT (Latenz > Bandbreite)
- **Max Message Size:** 64MB (`max_decoding_message_size: 67108864`)
- **Sharding:** 1-4 Round-Robin via `AtomicUsize`

### NIEMALS:
- Token in die URL einbetten
- Fumarole verwenden (zu langsam für HFT)
- Vixen hosted verwenden (deprecated)
- Gzip aktivieren

### Empfehlung vom Triton Support:
> "Dragon's Mouth is the ONLY recommendation for HFT. 
>  Fumarole is too slow. Vixen hosted is deprecated.
>  Use Carbon locally if needed."

---

## Pump.fun v2 PDAs (Feb 2026 Update)

**KRITISCH:** Neue PDAs IMMER ans ENDE der Account-Liste!

```rust
// bonding-curve-v2
seeds = [b"bonding-curve-v2", mint.as_ref()]

// pool-v2  
seeds = [b"pool-v2", base_mint.as_ref()]
```

**Creator Rewards Sharing + Mayhem Mode:**
- 8 neue Fee-Recipient-Adressen einbinden
- Reihenfolge in Account-Liste strikt einhalten

---

## Jito Bundle Optimierung

```rust
// Tip: 10.000 Lamports minimum, dynamisch anpassen
const JITO_TIP_LAMPORTS: u64 = 10_000;

// Block Engine Frankfurt:
// frankfurt.mainnet.block-engine.jito.wtf

// Bundle: max. 5 Transaktionen
// Immer: Status nach 30s pollen
```

---

## Rig / GOAT AI Integration

- **Framework:** Rig (Rust AI Agent Framework)
- **Model:** xAI Grok (XAI_API_KEY)
- **Tool:** GOAT SDK für Solana On-Chain Actions
- **Aufgabe:** Marktanalyse, Entry/Exit Signale, Stop-Loss

```rust
// Immer env-var nutzen:
let client = rig::providers::xai::Client::from_env();
```

---

## AWS Setup (eu-central-1)

- **Instanz:** t4g.small (ARM Graviton, ~$13/Monat)
- **AMI:** Ubuntu 22.04 ARM
- **Security Group:** SSH (22) + Prometheus (9091) + Grafana (3000)
  - NUR eigene IP freigeben!
- **IAM Role:** Secrets Manager + CloudWatch Permissions
- **Secrets in AWS:** Wallet Key, Triton Token, API Keys
- **GitHub Secrets für CI/CD:**
  - `TRITON_X_TOKEN`
  - `WALLET_PRIVATE_KEY`
  - `XAI_API_KEY`
  - `HELIUS_API_KEY`
  - `TELEGRAM_BOT_TOKEN`
  - `TELEGRAM_CHAT_ID`

---

## Trading Parameter (Defaults)

| Parameter | Wert | Beschreibung |
|---|---|---|
| MAX_SOL_PER_TRADE | 0.1 SOL | Max Einsatz pro Trade |
| SLIPPAGE_BPS | 500 | 5% Slippage |
| MIN_LIQUIDITY_SOL | 5.0 | Minimum Pool-Liquidität |
| TAKE_PROFIT_PCT | 50% | Auto-Exit bei +50% |
| STOP_LOSS_PCT | 20% | Auto-Exit bei -20% |

---

## Code-Stil Regeln

- Rust 2021 Edition
- `tokio` für async Runtime
- `anyhow` für Error Handling
- `tracing` für Logs (kein `println!`)
- `dotenvy` für .env Loading
- Alle Secrets NUR über Umgebungsvariablen
- NIEMALS Secrets im Code oder Git committen

---

## Copilot-Bewertung (Stand 04.03.2026)
- **Score: 8.5/10**
- Stärken: Performance, Docker, gRPC, AI
- Verbesserungen: Security, Dashboard, Monitoring
