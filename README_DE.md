# Solana HFT Sniper – v4.2 (Deutsch)

> **Kurzfassung** – Ein latenzoptimierter Solana HFT-Sniper, aufgebaut auf Triton
> "Dragon's Mouth" gRPC, Groq-KI-Entscheidungslogik, Jito MEV-geschützten Bundles und
> vollständiger AWS Self-Healing-Infrastruktur.

[![CI/CD Pipeline](https://github.com/Timson100x/solana-hft-ultimate-v4-triton-/actions/workflows/main.yaml/badge.svg)](https://github.com/Timson100x/solana-hft-ultimate-v4-triton-/actions)

> 🇬🇧 English version: [README.md](README.md)

---

## Inhaltsverzeichnis

1. [Systemübersicht](#1-systemübersicht)
2. [Schnellstart (One-Click-Deploy)](#2-schnellstart-one-click-deploy)
3. [Architektur & Verzeichnisstruktur](#3-architektur--verzeichnisstruktur)
4. [Kernkomponenten](#4-kernkomponenten)
5. [Triton-News Februar 2026](#5-triton-news-februar-2026)
6. [AWS-Deployment (Terraform)](#6-aws-deployment-terraform)
7. [Lokale Entwicklung (Docker)](#7-lokale-entwicklung-docker)
8. [CI / CD (GitHub Actions)](#8-ci--cd-github-actions)
9. [Geplante Erweiterungen](#9-geplante-erweiterungen)
10. [Lizenz & Mitwirken](#10-lizenz--mitwirken)

---

## 1. Systemübersicht

Der Bot hört auf **Solanas gRPC "Dragon's Mouth"** (Triton YellowStone) und empfängt
*Intra-Slot*-Updates (z. B. `SLOT_FIRST_SHRED`).

Sobald ein Pump.fun-Launch erkannt wird:

1. Die **KI-Brücke** (Groq `llama-3.3-70b-versatile`) wertet die Launch-Metadaten aus und
   antwortet mit **JA** oder **NEIN**.
2. Bei **JA** baut der Bot eine **v2-PDA**-`buy`-Instruktion (seit Feb 2026 Pflicht).
3. Die Instruktion wird in ein **Jito-Bundle** mit einem **10 000 Lamports Tip** verpackt
   → garantierte Top-of-Block-Platzierung.
4. Eine Telegram-Benachrichtigung geht ans Handy des Betreibers.

Das alles läuft auf einer **AWS t4g.small** (Graviton2) Instanz in `eu-central-1`
(Frankfurt), die direkt neben Tritons Iron Mountain FRA-2 / Equinix FR4 Nodes liegt.

### Triton One Setup

| Parameter       | Wert                                          |
|-----------------|-----------------------------------------------|
| **Endpoint**    | `timmys-mainnet-e441.rpcpool.com:443`         |
| **Tier**        | Tier 3                                        |
| **Rechenzentrum** | Frankfurt (Iron Mountain FRA-2 / Equinix FR4)|
| **Protokoll**   | Dragon's Mouth gRPC                           |
| **Auth**        | `x-token` Metadata-Header (NIEMALS in der URL)|
| **Keepalive**   | 30 s                                          |
| **Gzip**        | DEAKTIVIERT (Latenz > Bandbreite)             |
| **Max. Nachricht** | 64 MB                                      |
| **Shards**      | 1–4 Round-Robin (`AtomicUsize`)               |

---

## 2. Schnellstart (One-Click-Deploy)

```bash
# 1. Repository klonen
git clone https://github.com/Timson100x/solana-hft-ultimate-v4-triton-.git
cd solana-hft-ultimate-v4-triton-

# 2. .env kopieren und befüllen (siehe .env.example)
cp .env.example .env
# .env mit deinen Werten befüllen (Triton-Token, Private Key, Groq-Key, Telegram etc.)
chmod 600 .env

# 3. Setup-Script ausführen (installiert Rust, Abhängigkeiten, baut den Bot)
bash setup-v4.2-triton.sh
```

> ⚠️ **Niemals** die `.env`-Datei committen! Sie ist bereits in `.gitignore` eingetragen.

### Benötigte Secrets (`.env`)

| Variable             | Beschreibung                                      |
|----------------------|---------------------------------------------------|
| `TRITON_X_TOKEN`     | Triton One API-Token (x-token-Header)             |
| `WALLET_PRIVATE_KEY` | Base58-kodierter Wallet-Private-Key               |
| `XAI_API_KEY`        | xAI / Grok API-Key für die KI-Brücke             |
| `HELIUS_API_KEY`     | Helius Fallback-RPC-Key                           |
| `TELEGRAM_BOT_TOKEN` | Telegram-Bot-Token für Trade-Alerts               |
| `TELEGRAM_CHAT_ID`   | Deine Telegram-Chat-ID                            |

---

## 3. Architektur & Verzeichnisstruktur

```
Triton Dragon's Mouth gRPC
    │
    ▼
Rust HFT Bot (4 Shards, Round-Robin)
    │
    ├── src/main.rs            Entry Point, Shard-Manager
    ├── src/triton.rs          Dragon's Mouth gRPC Client
    ├── src/pump_tx.rs         Pump.fun v2 Transaktionen + PDAs
    ├── src/jito_optimized.rs  Bundle Builder + Tip-Logik
    ├── src/rig_goat.rs        KI-Entscheidungsengine (Rig + Groq)
    └── src/monitor.rs         Telegram-Alerts + Prometheus-Metriken
    │
    ▼
Jito Block Engine (Frankfurt)
    │
    ▼
Solana Mainnet
```

### Verzeichnisstruktur

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
├── README.md          ← Englische Version
└── README_DE.md       ← Diese Datei
```

---

## 4. Kernkomponenten

### 4a. gRPC-Streaming – Dragon's Mouth

- Verbindung via `tonic` zu `timmys-mainnet-e441.rpcpool.com:443`
- Auth via `x-token` Metadata-Header (Token **niemals** in der URL einbetten)
- Keepalive-Ping alle **30 s**, Timeout **5 s**
- Gzip **deaktiviert** (minimale Latenz)
- Maximale Dekodiernachrichtengröße: **64 MB**
- **4 Shards** verwaltet via `AtomicUsize` Round-Robin

```rust
// Auth-Header – korrekter Weg
let mut metadata = tonic::metadata::MetadataMap::new();
metadata.insert("x-token", token.parse()?);
```

### 4b. Pump.fun v2 PDA-Handling (Feb 2026)

> **Kritisch:** Die neuen PDAs müssen immer ans **Ende** der Account-Liste angehängt werden.

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

Weitere Änderungen im Feb-2026-Update:
- **Creator Rewards Sharing** aktiviert
- **Mayhem Mode**: 8 neue Fee-Recipient-Adressen (strikte Reihenfolge erforderlich)

### 4c. Groq-KI „JA / NEIN"-Filter

Die KI-Brücke (Modul `rig_goat.rs`) ruft `llama-3.3-70b-versatile` über das Rig-Framework ab:

```rust
let client = rig::providers::xai::Client::from_env(); // liest XAI_API_KEY
let decision = client.completion("Should I snipe this token? YES or NO").await?;
```

Nur Tokens, die ein **JA**-Urteil erhalten, werden gekauft. So werden Rug-Pulls und
minderwertige Launches gefiltert, bevor eine On-Chain-Transaktion gesendet wird.

### 4d. Jito-Bundle + Tip

```rust
const JITO_TIP_LAMPORTS: u64 = 10_000; // Minimum; dynamisch anpassen
// Block Engine: frankfurt.mainnet.block-engine.jito.wtf
// Max. 5 Transaktionen pro Bundle
// Bundle-Status nach 30 s pollen
```

Vorteile:
- **MEV-Schutz** – dein Kauf kann nicht gesandwiched werden
- **Top-of-Block**-Platzierung → erster Käufer im Slot
- Atomar: entweder landet das gesamte Bundle oder gar nichts

### 4e. Monitoring & Self-Healing

| Tool        | Port  | Zweck                                        |
|-------------|-------|----------------------------------------------|
| Telegram    | —     | Sofort-Trade-Alerts (Kauf/Verkauf/Fehler)    |
| Prometheus  | 9091  | Metriken: Latenz, PnL, Bundle-Status         |
| Grafana     | 3000  | Live-Dashboard                               |

Die AWS Auto Scaling Group nutzt **Predictive Scaling** auf Basis der Netzwerklatenz, um
vor Spitzenlastzeiten zusätzliche Kapazität hochzufahren.

### Trading-Parameter

| Parameter          | Standard | Beschreibung                    |
|--------------------|----------|---------------------------------|
| `MAX_SOL_PER_TRADE`| 0,1 SOL  | Maximaler Einsatz pro Trade     |
| `SLIPPAGE_BPS`     | 500      | 5 % Slippage-Toleranz           |
| `MIN_LIQUIDITY_SOL`| 5,0 SOL  | Mindest-Pool-Liquidität         |
| `TAKE_PROFIT_PCT`  | 50 %     | Auto-Exit bei +50 %             |
| `STOP_LOSS_PCT`    | 20 %     | Auto-Exit bei −20 %             |

---

## 5. Triton-News Februar 2026

> **Warum das für diesen Bot wichtig ist**

Triton hat im Februar 2026 mehrere wichtige Änderungen veröffentlicht:

| Änderung | Auswirkung |
|----------|------------|
| **Dragon's Mouth** bleibt der einzig empfohlene gRPC-Stream für HFT | Wir nutzen ihn exklusiv; keine Migration nötig |
| **Fumarole** offiziell als zu langsam für latenzempfindliche Bots eingestuft | Aus der Fallback-Logik entfernt |
| **Vixen hosted** deprecated | Wird nicht verwendet |
| **YellowStone v2 Protokoll** – `SLOT_FIRST_SHRED`-Event jetzt stabil | Ermöglicht noch frühzeitigere Einstiegssignale |
| Neue **Frankfurt-Colocation**-Nodes (Iron Mountain FRA-2, Equinix FR4) | Unsere `eu-central-1` AWS-Instanz hat jetzt < 1 ms Hop zum Validator |

**Empfohlene Einstellungen nach dem Update (bereits angewendet):**

```toml
keepalive_time_ms   = 30_000      # 30 s
keepalive_timeout   = 5_000       # 5 s
gzip_compression    = false       # Latenz > Bandbreite
max_message_size    = 67_108_864  # 64 MB
```

---

## 6. AWS-Deployment (Terraform)

> Infrastructure as Code liegt im Verzeichnis `infra/` (kommt bald – siehe
> [Geplante Erweiterungen](#9-geplante-erweiterungen)).

Manuelle Setup-Referenz:

```bash
# Region: eu-central-1 (Frankfurt)
# Instanz: t4g.small – ARM Graviton2 (~13 $/Monat bei 24/7-Betrieb)
# AMI: Ubuntu 22.04 ARM64
# Security Group:
#   - SSH (22)          → nur eigene IP
#   - Prometheus (9091) → nur eigene IP
#   - Grafana (3000)    → nur eigene IP

# IAM-Role-Berechtigungen:
#   - secretsmanager:GetSecretValue
#   - cloudwatch:PutMetricData

# Secrets in AWS Secrets Manager ablegen:
#   TRITON_X_TOKEN, WALLET_PRIVATE_KEY, XAI_API_KEY
#   HELIUS_API_KEY, TELEGRAM_BOT_TOKEN, TELEGRAM_CHAT_ID
```

Benötigte GitHub-Repository-Secrets für CI/CD:

| Secret               | Verwendungszweck                          |
|----------------------|-------------------------------------------|
| `TRITON_X_TOKEN`     | gRPC-Authentifizierung                    |
| `WALLET_PRIVATE_KEY` | On-Chain-Transaktionssignierung           |
| `XAI_API_KEY`        | Groq / xAI KI-Brücke                     |
| `HELIUS_API_KEY`     | Fallback-RPC                              |
| `TELEGRAM_BOT_TOKEN` | Trade-Benachrichtigungen                  |
| `TELEGRAM_CHAT_ID`   | Trade-Benachrichtigungen                  |

---

## 7. Lokale Entwicklung (Docker)

```bash
# Docker-Image bauen
docker build -t solana-hft:v4.2 .

# Mit .env-Datei starten
docker run --env-file .env solana-hft:v4.2

# Oder mit docker-compose (inkl. Prometheus + Grafana)
docker-compose up -d
```

> `Dockerfile` und `docker-compose.yml` sind geplant – siehe
> [Geplante Erweiterungen](#9-geplante-erweiterungen).

---

## 8. CI / CD (GitHub Actions)

Der Workflow unter `.github/workflows/main.yaml` läuft bei jedem Push / PR auf `main`:

1. **Checkout** des Repositories
2. **Rust einrichten** (stabiler Toolchain)
3. **`cargo build --release`** – schlägt die Pipeline bei Kompilierfehlern fehl
4. **`cargo test`** – führt die gesamte Test-Suite aus

Alle Secrets werden aus GitHub-Repository-Secrets injiziert und niemals geloggt oder
committet.

---

## 9. Geplante Erweiterungen

| Erweiterung | Status | Hinweise |
|-------------|--------|----------|
| Terraform `infra/`-Modul | 🔜 Geplant | Vollständiges IaC für One-Click-AWS-Deploy |
| Dockerfile + docker-compose | 🔜 Geplant | Lokale Entwicklungs- & Staging-Umgebung |
| Kelly-Kriterium Positionsgrößen | 🔜 Geplant | Dynamisches `MAX_SOL_PER_TRADE` basierend auf Gewinnrate |
| Multi-Relay Jito-Routing | 🔜 Geplant | Round-Robin über Frankfurt, Amsterdam, New York |
| Grafana-Dashboard JSON | 🔜 Geplant | Vorgefertigte Panels für Latenz, PnL, Bundle-Trefferquote |
| WebSocket-Fallback | 🔜 Geplant | Helius WS als Dragon's Mouth Backup |
| Trailing Stop-Loss | 🔜 Geplant | Dynamischer Stop-Loss, der dem Preis nach oben folgt |

---

## 10. Lizenz & Mitwirken

Dieses Projekt ist ausschließlich für **Bildungs- und persönliche Zwecke** bestimmt.
Krypto-Handel birgt erhebliche finanzielle Risiken. Nutzung auf eigene Gefahr.

**Sicherheitsregeln:**
- Niemals `.env`, Private Keys oder Tokens in Git committen
- Alle Secrets müssen über **AWS Secrets Manager** oder **GitHub Secrets** verwaltet werden
- Security Group: alle Ports auf die eigene IP beschränken

Pull Requests und Issues sind willkommen. Bitte öffne zuerst ein Issue, bevor du größere
Änderungen einreichst.

---

*Erstellt: März 2026 | Düsseldorf | Timmy*
