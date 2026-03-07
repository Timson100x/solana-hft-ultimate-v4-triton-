//! Configuration loaded from environment variables.
//!
//! Copy `.env.example` to `.env`, fill in your values, then run the bot.
//! **NEVER** commit a `.env` file containing real secrets.

use anyhow::{Context, Result};

/// All runtime configuration for the HFT bot.
// Fields not yet consumed at runtime are part of the public API and will
// be used by future sell/stop-loss logic.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Config {
    // ── Triton One (Dragon's Mouth gRPC) ─────────────────────────────────────
    /// gRPC endpoint without scheme, e.g. `timmys-mainnet-e441.rpcpool.com:443`
    pub triton_grpc_url: String,
    /// API token – sent as the `x-token` gRPC metadata header, NEVER in the URL
    pub triton_x_token: String,

    // ── Jito MEV ─────────────────────────────────────────────────────────────
    /// Block-engine hostname, e.g. `frankfurt.mainnet.block-engine.jito.wtf`
    pub jito_block_engine_url: String,
    /// Minimum tip in Lamports (10 000 = 0.00001 SOL)
    pub jito_tip_lamports: u64,
    /// Maximum transactions per bundle (Jito limit: 5)
    pub jito_max_bundle_size: usize,

    // ── Wallet ────────────────────────────────────────────────────────────────
    /// Base-58 encoded private key (64 bytes)
    pub wallet_private_key: String,
    /// Base-58 encoded public key (32 bytes)
    pub wallet_public_key: String,

    // ── Helius (fallback RPC) ─────────────────────────────────────────────────
    pub helius_rpc_url: String,

    // ── xAI / Grok AI bridge ──────────────────────────────────────────────────
    pub xai_api_key: String,

    // ── Telegram alerts ───────────────────────────────────────────────────────
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,

    // ── Trading parameters ────────────────────────────────────────────────────
    /// Maximum SOL to risk per trade (default: 0.1)
    pub max_sol_per_trade: f64,
    /// Slippage tolerance in basis points (default: 500 = 5 %)
    pub slippage_bps: u64,
    /// Minimum pool liquidity in SOL before entering (default: 5.0)
    pub min_liquidity_sol: f64,
    /// Take-profit threshold in percent (default: 50 %)
    pub take_profit_pct: f64,
    /// Stop-loss threshold in percent (default: 20 %)
    pub stop_loss_pct: f64,

    // ── Sharding ──────────────────────────────────────────────────────────────
    /// Number of parallel Dragon's Mouth shard connections (1–4)
    pub shards: usize,
    /// gRPC keepalive interval in seconds (30 s per Triton recommendation)
    pub keepalive_secs: u64,
    /// gRPC max decoding message size in bytes (64 MB)
    pub max_message_size: usize,
}

impl Config {
    /// Build a [`Config`] by reading environment variables.
    ///
    /// Required variables: `TRITON_X_TOKEN`, `WALLET_PRIVATE_KEY`,
    /// `WALLET_PUBLIC_KEY`, `XAI_API_KEY`, `TELEGRAM_BOT_TOKEN`,
    /// `TELEGRAM_CHAT_ID`.  All others have sensible defaults.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            triton_grpc_url: std::env::var("TRITON_GRPC_URL")
                .unwrap_or_else(|_| "timmys-mainnet-e441.rpcpool.com:443".to_string()),
            triton_x_token: std::env::var("TRITON_X_TOKEN")
                .context("TRITON_X_TOKEN is required")?,

            jito_block_engine_url: std::env::var("JITO_BLOCK_ENGINE_URL")
                .unwrap_or_else(|_| "frankfurt.mainnet.block-engine.jito.wtf".to_string()),
            jito_tip_lamports: parse_env("JITO_TIP_LAMPORTS", 10_000)?,
            jito_max_bundle_size: parse_env("JITO_MAX_BUNDLE_SIZE", 5)?,

            wallet_private_key: std::env::var("WALLET_PRIVATE_KEY")
                .context("WALLET_PRIVATE_KEY is required")?,
            wallet_public_key: std::env::var("WALLET_PUBLIC_KEY")
                .context("WALLET_PUBLIC_KEY is required")?,

            helius_rpc_url: std::env::var("HELIUS_RPC_URL").unwrap_or_default(),

            xai_api_key: std::env::var("XAI_API_KEY").context("XAI_API_KEY is required")?,

            telegram_bot_token: std::env::var("TELEGRAM_BOT_TOKEN")
                .context("TELEGRAM_BOT_TOKEN is required")?,
            telegram_chat_id: std::env::var("TELEGRAM_CHAT_ID")
                .context("TELEGRAM_CHAT_ID is required")?,

            max_sol_per_trade: parse_env("MAX_SOL_PER_TRADE", 0.1_f64)?,
            slippage_bps: parse_env("SLIPPAGE_BPS", 500_u64)?,
            min_liquidity_sol: parse_env("MIN_LIQUIDITY_SOL", 5.0_f64)?,
            take_profit_pct: parse_env("TAKE_PROFIT_PCT", 50.0_f64)?,
            stop_loss_pct: parse_env("STOP_LOSS_PCT", 20.0_f64)?,

            shards: parse_env("SHARDS", 4_usize)?,
            keepalive_secs: parse_env("KEEPALIVE_SECS", 30_u64)?,
            max_message_size: parse_env("MAX_MESSAGE_SIZE_MB", 64_usize)? * 1024 * 1024,
        })
    }
}

/// Parse an environment variable into type `T`, returning `default` when the
/// variable is unset.
fn parse_env<T>(name: &str, default: T) -> Result<T>
where
    T: std::str::FromStr + std::fmt::Debug,
    T::Err: std::fmt::Display,
{
    match std::env::var(name) {
        Ok(val) => val
            .parse::<T>()
            .map_err(|e| anyhow::anyhow!("{name}={val:?} is not valid: {e}")),
        Err(_) => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_returns_default_when_unset() {
        std::env::remove_var("__HFT_TEST_VAR");
        let v: u64 = parse_env("__HFT_TEST_VAR", 42).unwrap();
        assert_eq!(v, 42);
    }

    #[test]
    fn parse_env_reads_set_value() {
        std::env::set_var("__HFT_TEST_VAR2", "99");
        let v: u64 = parse_env("__HFT_TEST_VAR2", 0).unwrap();
        assert_eq!(v, 99);
        std::env::remove_var("__HFT_TEST_VAR2");
    }
}
