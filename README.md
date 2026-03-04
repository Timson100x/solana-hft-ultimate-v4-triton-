# Solana HFT Ultimate v4.2 - Triton One Edition

> High-Frequency Trading Bot für Pump.fun Sniping | Rust + Jito + Triton Dragon's Mouth

[![Rust CI/CD](https://github.com/Timson100x/solana-hft-ultimate-v4-triton-/actions/workflows/rust-ci-cd.yml/badge.svg)](https://github.com/Timson100x/solana-hft-ultimate-v4-triton-/actions)

---

## Schnellstart (Mac/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/Timson100x/solana-hft-ultimate-v4-triton-/main/setup-v4.2-triton.sh | bash
```

Oder manuell:
```bash
git clone https://github.com/Timson100x/solana-hft-ultimate-v4-triton-.git
cd solana-hft-ultimate-v4-triton-
cp .env.example .env
# .env mit eigenen Werten befüllen!
bash setup-v4.2-triton.sh
```

---

## Architektur

```
[Triton Dragon's Mouth gRPC]
         |
    [4x Shards]
         |
   [Rust HFT Bot]
    /    |    \
 pump_tx  jito  rig_goat
    \    |    /
  [Jito Block Engine]
         |
   [Solana Mainnet]
```

---

## Mein Triton One Setup

| Parameter | Wert |
|---|---|
| **Endpoint** | `timmys-mainnet-e441.rpcpool.com:443` |
| **Tier** | Tier 3 |
| **DC** | Frankfurt (Iron Mountain FRA-2 / Equinix FR4) |
| **Protocol** | Dragon's Mouth gRPC |
| **Keepalive** | 30s |
| **Gzip** | DEAKTIVIERT |
| **Max Msg Size** | 64 MB |
| **Shards** | 1-4 Round-Robin |

### Wichtige Triton Regeln:
- Token IMMER als `x-token` Header - NIEMALS in der URL!
- Fumarole = zu langsam für HFT
- Vixen hosted = deprecated
- Dragon's Mouth = einzige Empfehlung für HFT

---

## Module

| Datei | Funktion |
|---|---|
| `src/main.rs` | Entry Point, Shard-Manager |
| `src/pump_tx.rs` | Pump.fun v2 Transaktionen + PDAs |
| `src/jito_optimized.rs` | Jito Bundle Builder + Tip |
| `src/rig_goat.rs` | AI Decision Engine (Rig + GOAT) |
| `src/monitor.rs` | Telegram Alerts + Prometheus |
| `src/triton.rs` | Dragon's Mouth gRPC Client |

---

## Pump.fun v2 Update (Feb 2026)

NEU: PDAs müssen IMMER am Ende der Account-Liste stehen:
- `bonding-curve-v2` (seeds: `b"bonding-curve-v2"`, mint)
- `pool-v2` (seeds: `b"pool-v2"`, base_mint)
- Creator Rewards Sharing aktiviert
- Mayhem Mode: 8 neue Fee-Recipient-Adressen

---

## AWS Setup (eu-central-1)

```bash
# Instance: t4g.small (ARM Graviton)
# ~13$/Monat bei 24/7 Betrieb
# AMI: Ubuntu 22.04 ARM64

# GitHub Secrets setzen:
# TRITON_X_TOKEN
# WALLET_PRIVATE_KEY
# XAI_API_KEY
# HELIUS_API_KEY
# TELEGRAM_BOT_TOKEN
# TELEGRAM_CHAT_ID
```

---

## Trading Parameter

| Parameter | Default | Beschreibung |
|---|---|---|
| MAX_SOL_PER_TRADE | 0.1 | Max SOL pro Trade |
| SLIPPAGE_BPS | 500 | 5% Slippage |
| MIN_LIQUIDITY_SOL | 5.0 | Min Pool-Liquidität |
| TAKE_PROFIT_PCT | 50 | Auto-Sell bei +50% |
| STOP_LOSS_PCT | 20 | Auto-Sell bei -20% |

---

## Monitoring

- **Telegram:** Sofort-Alerts bei jedem Trade
- **Prometheus:** Metriken auf Port 9091
- **Grafana:** Dashboard auf Port 3000

---

## Sicherheit

- Alle Secrets in AWS Secrets Manager
- GitHub Secrets für CI/CD
- NIEMALS Private Key oder Token committen
- `.env` ist in `.gitignore`
- Security Group: Nur eigene IP

---

## Copilot Bewertung: 8.5/10

**Stärken:** Performance, Docker, gRPC, AI-Integration  
**In Arbeit:** Dashboard, Monitoring, Security hardening

---

*Erstellt: 04. März 2026 | Düsseldorf | Timmy*
