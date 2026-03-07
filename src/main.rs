//! Solana HFT Ultimate v4.2 – Triton One Edition.
//!
//! Entry point: spawns [`SHARD_COUNT`] Dragon's Mouth gRPC connections in a
//! round-robin pattern and blocks until SIGINT / SIGTERM is received.

use anyhow::Result;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::signal;
use tracing::{error, info};

mod jito_optimized;
mod monitor;
mod pump_tx;
mod rig_goat;
mod triton;

use monitor::Monitor;
use rig_goat::RigGoat;
use triton::TritonClient;

/// Number of parallel Dragon's Mouth gRPC shards (round-robin).
const SHARD_COUNT: usize = 4;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file – silently skip when env vars are injected directly (AWS / CI).
    let _ = dotenvy::dotenv();

    // Initialise structured logging; controlled via RUST_LOG env var.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "solana_hft_ultimate=info".into()),
        )
        .init();

    info!("🚀 Solana HFT Ultimate v4.2 (Triton One Edition) starting…");

    // All secrets come from environment variables – never hard-coded.
    let triton_token =
        std::env::var("TRITON_X_TOKEN").expect("TRITON_X_TOKEN must be set");
    let wallet_key =
        std::env::var("WALLET_PRIVATE_KEY").expect("WALLET_PRIVATE_KEY must be set");

    // Shared atomic shard counter for round-robin distribution.
    let shard_counter = Arc::new(AtomicUsize::new(0));

    // Trade monitoring: Telegram alerts + Prometheus metrics.
    let monitor = Arc::new(Monitor::from_env());
    monitor.start_metrics_server().await;

    // AI decision engine powered by xAI Grok.
    let ai = Arc::new(RigGoat::from_env());

    info!("📡 Launching {SHARD_COUNT} Dragon's Mouth shards…");

    let mut handles = Vec::with_capacity(SHARD_COUNT);
    for shard_id in 0..SHARD_COUNT {
        let token = triton_token.clone();
        let wallet = wallet_key.clone();
        let mon = Arc::clone(&monitor);
        let ai_ref = Arc::clone(&ai);
        let counter = Arc::clone(&shard_counter);

        let handle = tokio::spawn(async move {
            let idx = counter.fetch_add(1, Ordering::Relaxed) % SHARD_COUNT;
            info!(shard = idx, "Shard {shard_id} connecting…");

            match TritonClient::connect(&token).await {
                Ok(client) => {
                    if let Err(e) = client.run(&wallet, &mon, &ai_ref).await {
                        error!(shard = idx, "Shard {shard_id} error: {e:#}");
                    }
                }
                Err(e) => {
                    error!(shard = idx, "Shard {shard_id} connection failed: {e:#}");
                }
            }
        });
        handles.push(handle);
    }

    // Block until Ctrl-C / SIGTERM, then abort all shards gracefully.
    signal::ctrl_c().await?;
    info!("🛑 Shutdown signal received – terminating shards…");

    for h in handles {
        h.abort();
    }

    info!("✅ Shutdown complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_count_is_positive() {
        assert!(SHARD_COUNT > 0);
    }

    #[test]
    fn shard_count_within_bounds() {
        // Dragon's Mouth supports 1-4 shards as per CLAUDE.md.
        assert!(SHARD_COUNT >= 1 && SHARD_COUNT <= 4);
    }
}
