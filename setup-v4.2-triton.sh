#!/bin/bash

# ==============================================================================
# Solana HFT Ultimate v4.2 - Triton One Edition
# ==============================================================================
# Features:
# - Triton One gRPC (Dragons Mouth) Integration
# - High-Speed Pump.fun Sniper (v2 PDAs)
# - Jito Bundle Optimization
# - Rig GOAT Integration
# - Real-time gRPC Streaming
# ==============================================================================

set -e

# Farben für Output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()    { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()    { echo -e "${YELLOW}[WARN]${NC} $1"; }
error()   { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
header()  { echo -e "\n${BLUE}======================================================================${NC}"; echo -e "${BLUE} $1${NC}"; echo -e "${BLUE}======================================================================${NC}\n"; }

# ----- Konfiguration -----
REPO_URL="https://github.com/Timson100x/solana-hft-ultimate-v4-triton-.git"
PROJECT_DIR="/opt/solana-hft"
RUST_VERSION="stable"
TRITON_GRPC_URL="timmys-mainnet-e441.rpcpool.com:443"
KEEPALIVE_SECS=30
MAX_MESSAGE_SIZE=67108864
SHARDS=4
JITO_URL="frankfurt.mainnet.block-engine.jito.wtf"

header "Solana HFT Ultimate v4.2 - Triton One Setup"
info "Triton Endpoint: ${TRITON_GRPC_URL}"
info "Keepalive: ${KEEPALIVE_SECS}s | Max Msg: 64MB | Shards: ${SHARDS}"

# ##############################################################################
# SCHRITT 1: System-Abhängigkeiten
# ##############################################################################
header "Schritt 1: System-Abhängigkeiten installieren"

sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    cmake \
    git \
    curl \
    llvm \
    libudev-dev \
    protobuf-compiler

# ##############################################################################
# SCHRITT 2: Rust Installation
# ##############################################################################
header "Schritt 2: Rust Umgebung einrichten"

if ! command -v rustc &> /dev/null; then
    info "Rust wird installiert..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
fi

export PATH="$HOME/.cargo/bin:$PATH"
rustup update stable
rustup component add clippy rustfmt
info "Rust: $(rustc --version)"

# ##############################################################################
# SCHRITT 3: Repository klonen / aktualisieren
# ##############################################################################
header "Schritt 3: Repository einrichten"

sudo mkdir -p "${PROJECT_DIR}"
sudo chown "${USER}:${USER}" "${PROJECT_DIR}"

if [ -d "${PROJECT_DIR}/.git" ]; then
    info "Repository bereits vorhanden – aktualisiere…"
    cd "${PROJECT_DIR}"
    git pull --ff-only
else
    info "Klone Repository nach ${PROJECT_DIR}…"
    git clone "${REPO_URL}" "${PROJECT_DIR}"
    cd "${PROJECT_DIR}"
fi

# ##############################################################################
# SCHRITT 4: .env konfigurieren
# ##############################################################################
header "Schritt 4: Umgebungsvariablen konfigurieren"

if [ ! -f "${PROJECT_DIR}/.env" ]; then
    cp "${PROJECT_DIR}/.env.example" "${PROJECT_DIR}/.env"
    warn ".env aus .env.example erstellt – bitte Secrets eintragen!"
    warn "NIEMALS echte Tokens in die URL einbetten – nur als x-token Header!"
else
    info ".env bereits vorhanden – keine Änderungen."
fi

# ##############################################################################
# SCHRITT 5: Bauen
# ##############################################################################
header "Schritt 5: Projekt bauen"

cd "${PROJECT_DIR}"
cargo build --release
info "Build erfolgreich: ${PROJECT_DIR}/target/release/solana-hft-ultimate"

# ##############################################################################
# SCHRITT 6: Abschluss
# ##############################################################################
header "Setup Abgeschlossen"

info "Projekt bereit in ${PROJECT_DIR}"
info "Triton gRPC ist vorkonfiguriert: ${TRITON_GRPC_URL}"

echo -e "\n${GREEN}NÄCHSTE SCHRITTE:${NC}"
echo "1. Trage deine Secrets in ${PROJECT_DIR}/.env ein."
echo "2. Starte den Bot: cd ${PROJECT_DIR} && cargo run --release"
echo "   Oder als systemd-Service einrichten."

info "Viel Erfolg beim Trading! 🚀"
