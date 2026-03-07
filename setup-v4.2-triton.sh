#!/bin/bash

# ==============================================================================
# Solana HFT Ultimate v4.2 - Triton One Edition
# Setup Script for AWS t4g.small (Ubuntu 22.04 ARM64, eu-central-1)
# ==============================================================================
# Features:
#   - Triton One Dragon's Mouth gRPC (4 shards, keepalive 30 s, 64 MB max msg)
#   - High-Speed Pump.fun v2 Sniper (Feb 2026 PDAs)
#   - Jito Bundle Optimisation (Frankfurt block engine, 10 000 Lamports tip)
#   - xAI / Grok AI Decision Engine (Rig-compatible)
#   - Telegram Alerts + Prometheus Metrics
#
# Usage:
#   bash setup-v4.2-triton.sh
#
# Required environment variables (set before running or fill in .env):
#   TRITON_X_TOKEN, WALLET_PRIVATE_KEY, WALLET_PUBLIC_KEY,
#   XAI_API_KEY, TELEGRAM_BOT_TOKEN, TELEGRAM_CHAT_ID
# ==============================================================================

set -euo pipefail

# ── Colours ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()   { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()   { echo -e "${YELLOW}[WARN]${NC} $1"; }
error()  { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }
header() {
    echo -e "\n${BLUE}=====================================================================${NC}"
    echo -e "${BLUE} $1${NC}"
    echo -e "${BLUE}=====================================================================${NC}\n"
}

# ── Configuration ──────────────────────────────────────────────────────────────
REPO_URL="https://github.com/Timson100x/solana-hft-ultimate-v4-triton-.git"
PROJECT_DIR="/opt/solana-hft"
RUST_VERSION="stable"
TRITON_GRPC_URL="timmys-mainnet-e441.rpcpool.com:443"
JITO_URL="frankfurt.mainnet.block-engine.jito.wtf"
KEEPALIVE_SECS=30
MAX_MESSAGE_SIZE_MB=64
SHARDS=4
PROMETHEUS_PORT=9091

header "Solana HFT Ultimate v4.2 – Triton One Setup"
info "Triton endpoint : ${TRITON_GRPC_URL}"
info "Jito engine     : ${JITO_URL}"
info "Keepalive       : ${KEEPALIVE_SECS} s | Max msg: ${MAX_MESSAGE_SIZE_MB} MB | Shards: ${SHARDS}"

# ##############################################################################
# STEP 1: System dependencies
# ##############################################################################
header "Step 1: Install system dependencies"

sudo apt-get update -qq
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
# STEP 2: Rust toolchain
# ##############################################################################
header "Step 2: Set up Rust"

if ! command -v rustc &>/dev/null; then
    info "Installing Rust via rustup…"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain "${RUST_VERSION}"
    # shellcheck source=/dev/null
    source "${HOME}/.cargo/env"
fi

export PATH="${HOME}/.cargo/bin:${PATH}"
rustup update "${RUST_VERSION}"
rustup component add clippy rustfmt
info "Rust: $(rustc --version)"

# ##############################################################################
# STEP 3: Clone / update repository
# ##############################################################################
header "Step 3: Clone repository"

if [[ -d "${PROJECT_DIR}/.git" ]]; then
    info "Repository already exists – pulling latest changes"
    git -C "${PROJECT_DIR}" pull --ff-only
else
    sudo mkdir -p "${PROJECT_DIR}"
    sudo chown "${USER}:${USER}" "${PROJECT_DIR}"
    git clone "${REPO_URL}" "${PROJECT_DIR}"
fi

cd "${PROJECT_DIR}"

# ##############################################################################
# STEP 4: Environment file
# ##############################################################################
header "Step 4: Configure .env"

if [[ ! -f "${PROJECT_DIR}/.env" ]]; then
    cp "${PROJECT_DIR}/.env.example" "${PROJECT_DIR}/.env"
    chmod 600 "${PROJECT_DIR}/.env"
    warn ".env created from .env.example – fill in your secrets before starting the bot!"
    warn "  edit ${PROJECT_DIR}/.env"
else
    info ".env already exists – skipping"
fi

# Fail fast if any critical secret still contains the placeholder value.
check_placeholder() {
    local var_name="$1" value="$2"
    if [[ "${value}" == *"DEIN"* || "${value}" == *"HIER"* ]]; then
        error "${var_name} still contains a placeholder value. Edit ${PROJECT_DIR}/.env first."
    fi
}

# Source the .env (if it exists) so we can validate the values
if [[ -f "${PROJECT_DIR}/.env" ]]; then
    # shellcheck disable=SC1091
    set -a; source "${PROJECT_DIR}/.env"; set +a
    check_placeholder "TRITON_X_TOKEN"     "${TRITON_X_TOKEN:-}"
    check_placeholder "WALLET_PRIVATE_KEY" "${WALLET_PRIVATE_KEY:-}"
    check_placeholder "XAI_API_KEY"        "${XAI_API_KEY:-}"
    check_placeholder "TELEGRAM_BOT_TOKEN" "${TELEGRAM_BOT_TOKEN:-}"
fi

# ##############################################################################
# STEP 5: Build the bot
# ##############################################################################
header "Step 5: Build (release)"

cargo build --release 2>&1
info "Build successful: ${PROJECT_DIR}/target/release/solana-hft-ultimate"

# ##############################################################################
# STEP 6: systemd service
# ##############################################################################
header "Step 6: Install systemd service"

SERVICE_FILE="/etc/systemd/system/solana-hft.service"

sudo tee "${SERVICE_FILE}" > /dev/null << UNIT
[Unit]
Description=Solana HFT Ultimate v4.2 – Triton One Edition
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${USER}
WorkingDirectory=${PROJECT_DIR}
EnvironmentFile=${PROJECT_DIR}/.env
ExecStart=${PROJECT_DIR}/target/release/solana-hft-ultimate
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=solana-hft

[Install]
WantedBy=multi-user.target
UNIT

sudo systemctl daemon-reload
sudo systemctl enable solana-hft.service
info "systemd service installed: solana-hft.service"
info "  Start  : sudo systemctl start  solana-hft"
info "  Logs   : sudo journalctl -fu    solana-hft"
info "  Status : sudo systemctl status  solana-hft"

# ##############################################################################
# STEP 7: Summary
# ##############################################################################
header "Setup complete"

PUBLIC_IP=$(curl -sf https://ifconfig.me || echo "<your-server-ip>")

echo -e "${GREEN}NEXT STEPS:${NC}"
echo "  1. Fill in your secrets:  edit ${PROJECT_DIR}/.env"
echo "  2. Start the bot:         sudo systemctl start solana-hft"
echo "  3. Follow logs:           sudo journalctl -fu solana-hft"
echo ""
echo "  SSH from your phone: ssh ubuntu@${PUBLIC_IP}"
echo ""
echo -e "${GREEN}Prometheus metrics:${NC}  http://${PUBLIC_IP}:${PROMETHEUS_PORT}/metrics"
echo ""
info "Remember: NEVER commit .env or any file containing real secrets."
info "Viel Erfolg beim Trading! 🚀"
