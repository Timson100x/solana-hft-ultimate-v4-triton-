#!/bin/bash

# 🛠 Solana HFT v4.2 - Triton Installation Script
# Verwendung auf einem iPhone (Termius, BlinkShell oder ähnliches Terminal):
# 1. Stelle sicher, dass die iOS-App mit deinem Contabo VPS verbunden ist.
# 2. Kopiere und füge dieses Skript ein oder führe den curl-Befehl unten aus.

# 📥 Starte den Prozess:
# curl -fsSL "https://raw.githubusercontent.com/Timson100x/solana-hft-ultimate-v4-triton-/main/setup-v4.2-triton.sh" | bash

set -e  # Sofort abbrechen bei Fehlern

# Variablen
REPO_URL="https://github.com/Timson100x/solana-hft-ultimate-v4-triton-.git"
TRITON_GRPC_URL="timmys-mainnet-e441.rpcpool.com:443"
KEEPALIVE=30
MAX_MESSAGE_SIZE=67108864  # 64MB
SHARDS="1-4"

# Hilfsfunktion für farbige Ausgabe
function info {
    echo -e "\033[1;32m[INFO]\033[0m $1"
}

# Installiere grundlegende Abhängigkeiten
info "Installiere grundlegende Abhängigkeiten (git, curl, Docker, etc.)"
sudo apt update && sudo apt install -y git curl docker.io docker-compose build-essential

# Klone das Repository
info "Klone das Repository: $REPO_URL"
if [ ! -d "solana-hft-ultimate-v4-triton" ]; then
    git clone $REPO_URL
else
    info "Repository existiert bereits lokal, überspringe Klonen."
fi
cd solana-hft-ultimate-v4-triton

# Überprüfe Rust und installiere falls nötig
if ! command -v cargo &> /dev/null; then
    info "Rust ist nicht installiert. Installiere Rust (via rustup)."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    export PATH="$HOME/.cargo/bin:$PATH"
else
    info "Rust ist bereits installiert."
fi

# Baue Rust-Module
info "Baue Rust-Module (rust-bot/, pump_tx.rs, build_buy_instruction, etc.)."
cargo build --release

# Füge Triton Dragon’s Mouth gRPC Unterstützung hinzu
info "Konfiguriere Triton Dragon’s Mouth gRPC (URL: $TRITON_GRPC_URL)."
echo -e "" > triton-config.yaml
cat <<EOF > triton-config.yaml
grpc:
  url: "$TRITON_GRPC_URL"
  keepalive: $KEEPALIVE
  max_message_size: $MAX_MESSAGE_SIZE
  shards: "$SHARDS"
compression:
  enabled: false
EOF

# .env.example, README.md, etc. generieren
info "Generiere unterstützende Dateien (.env.example, .gitignore, README.md, etc.)."
cat <<EOF > .env.example
# Beispiel-Umgebungsvariablen
gRPC_URL="$TRITON_GRPC_URL"
RUST_BACKTRACE=1
EOF

cat <<EOF > .gitignore
# Ignoriere Logs, Abhängigkeiten und Build-Dateien
*.log
target/
.env
EOF

cat <<EOF > README.md
# Solana HFT v4.2 - Triton Edition

## Einführung
Dieses Repository enthält eine optimierte High-Frequency-Trading-Engine für Solana. Entwickelt für Pump.fun Sniping und maximalen ROI.

## Installation
Führen Sie das Skript `setup-v4.2-triton.sh` aus, um alles vorzubereiten.

## Konfiguration
Bearbeiten Sie die Datei `.env` basierend auf `.env.example`, bevor Sie den Dienst starten.

## Start
```
docker-compose up --build
```
EOF

# Erstelle CI/CD Workflow
info "Erstelle CI/CD Workflow."
mkdir -p .github/workflows
cat <<EOF > .github/workflows/main.yaml
name: CI/CD Pipeline

on:
  push:
    branches:
      - main
  pull_request:
    branches:
      - main

jobs:
  build-and-test:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout Repository
        uses: actions/checkout@v3
      - name: Set up Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build with Cargo
        run: cargo build --release
      - name: Run Tests
        run: cargo test
EOF

# Abschließende Anleitung
info "Setup abgeschlossen! Bitte führen Sie nun folgende Schritte auf einem iPhone aus:"
info "1️⃣ Öffnen Sie die .env-Datei und passen Sie die Variablen an Ihre Anforderungen an."
info "2️⃣ Starten Sie die Engine mit: docker-compose up --build."
info "3️⃣ Besuchen Sie die README.md für weitere Informationen."

echo -e "\033[1;32m[✅ ERFOLGREICH]\033[0m Ihr Solana-HFT-System ist bereit. Schnelles Sniping auf Pump.fun!"