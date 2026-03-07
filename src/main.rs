//! Entry point – shard manager and main trading loop.
//!
//! Starts [`Config::shards`] Dragon's Mouth gRPC connections (round-robin via
//! `AtomicUsize`), feeds detected Pump.fun launch events through the AI filter
//! ([`RigGoat`]), builds a v2 buy instruction ([`pump_tx`]), submits a Jito
//! bundle ([`jito_optimized`]), and sends a Telegram alert ([`monitor`]).

use anyhow::Result;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::mpsc;
use tracing::{error, info};

mod config;
mod jito_optimized;
mod monitor;
mod pump_tx;
mod rig_goat;
mod triton;

use config::Config;
use jito_optimized::{submit_bundle, BundleStatus};
use monitor::{start_prometheus_server, Metrics, Monitor};
use pump_tx::build_buy_instruction;
use rig_goat::RigGoat;
use triton::{LaunchEvent, TritonShard};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file (silently ignored when absent – use env vars directly)
    dotenvy::dotenv().ok();

    // Structured logging via tracing; level from RUST_LOG env var
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("solana_hft_ultimate=info".parse()?),
        )
        .init();

    info!("🚀 Solana HFT Ultimate v4.2 – Triton One Edition");

    let config = Arc::new(Config::from_env()?);
    info!(
        shards = config.shards,
        jito_tip = config.jito_tip_lamports,
        max_sol = config.max_sol_per_trade,
        "⚙️  Configuration loaded"
    );

    // Shared round-robin shard counter
    let shard_counter = Arc::new(AtomicUsize::new(0));

    // Channel: Dragon's Mouth shards → main event loop
    let (event_tx, mut event_rx) = mpsc::channel::<LaunchEvent>(1024);

    // ── Spawn Dragon's Mouth shard tasks ──────────────────────────────────────
    for shard_id in 0..config.shards {
        let cfg = Arc::clone(&config);
        let tx = event_tx.clone();
        tokio::spawn(async move {
            let shard = TritonShard::new(
                shard_id,
                cfg.triton_grpc_url.clone(),
                cfg.triton_x_token.clone(),
                cfg.keepalive_secs,
                cfg.max_message_size,
            );
            loop {
                if let Err(e) = shard.subscribe(tx.clone()).await {
                    error!(
                        shard = shard_id,
                        error = %e,
                        "Dragon's Mouth shard disconnected – reconnecting in 2 s"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        });
    }
    drop(event_tx); // senders now live only in shard tasks

    // ── Initialise AI, monitor, and metrics ───────────────────────────────────
    let ai = RigGoat::new(config.xai_api_key.clone());
    let monitor = Monitor::new(
        config.telegram_bot_token.clone(),
        config.telegram_chat_id.clone(),
    );
    let metrics = Arc::new(Metrics::new());

    // Prometheus metrics server on port 9091
    tokio::spawn(start_prometheus_server(Arc::clone(&metrics), 9091));

    info!(
        "📡 Listening for Pump.fun launches on {} Dragon's Mouth shards…",
        config.shards
    );

    // ── Main event loop ───────────────────────────────────────────────────────
    while let Some(event) = event_rx.recv().await {
        let ai = ai.clone();
        let monitor = monitor.clone();
        let cfg = Arc::clone(&config);
        let metrics = Arc::clone(&metrics);
        let shard_idx = shard_counter.fetch_add(1, Ordering::Relaxed) % cfg.shards;

        tokio::spawn(async move {
            if let Err(e) = process_launch(event, cfg, ai, monitor, metrics, shard_idx).await {
                error!(error = %e, "Error processing launch event");
            }
        });
    }

    info!("All Dragon's Mouth shards closed – shutting down");
    Ok(())
}

// ── Per-event processing ──────────────────────────────────────────────────────

async fn process_launch(
    event: LaunchEvent,
    config: Arc<Config>,
    ai: RigGoat,
    monitor: Monitor,
    metrics: Arc<Metrics>,
    shard_idx: usize,
) -> Result<()> {
    info!(
        mint = %event.mint,
        symbol = %event.symbol,
        shard = shard_idx,
        slot = event.slot,
        "📡 Pump.fun launch detected"
    );

    // ── Liquidity guard ──────────────────────────────────────────────────────
    let liquidity_sol = event.virtual_sol_reserves as f64 / 1_000_000_000.0;
    if liquidity_sol < config.min_liquidity_sol {
        info!(
            mint = %event.mint,
            liquidity = liquidity_sol,
            min = config.min_liquidity_sol,
            "⏩ Skipped – below minimum liquidity"
        );
        return Ok(());
    }

    // ── AI YES/NO filter ──────────────────────────────────────────────────────
    let decision = ai.analyze_launch(&event).await?;
    if !decision.should_buy {
        info!(
            mint = %event.mint,
            reason = %decision.reason,
            "🤖 AI: SKIP"
        );
        return Ok(());
    }
    info!(mint = %event.mint, "🤖 AI: BUY");

    // ── Build Pump.fun v2 buy instruction ────────────────────────────────────
    let instruction = build_buy_instruction(
        &event.mint,
        &config.wallet_public_key,
        config.max_sol_per_trade,
        config.slippage_bps,
    )?;

    // ── Submit Jito bundle ────────────────────────────────────────────────────
    let bundle_result = submit_bundle(
        vec![instruction],
        config.jito_tip_lamports,
        &config.jito_block_engine_url,
        &config.wallet_private_key,
    )
    .await?;

    info!(
        mint = %event.mint,
        bundle_id = %bundle_result.bundle_id,
        "⚡ Jito bundle submitted"
    );

    // ── Telegram alert ────────────────────────────────────────────────────────
    monitor
        .send_buy_alert(&event, config.max_sol_per_trade, &bundle_result.bundle_id)
        .await?;

    // ── Record metric ─────────────────────────────────────────────────────────
    metrics.record_buy();

    // ── Poll bundle status ────────────────────────────────────────────────────
    let jito = jito_optimized::JitoClient::new(&config.jito_block_engine_url);
    match jito.poll_bundle_status(&bundle_result.bundle_id).await {
        Ok(BundleStatus::Landed) => {
            info!(mint = %event.mint, bundle_id = %bundle_result.bundle_id, "✅ Bundle landed");
        }
        Ok(BundleStatus::Failed(reason)) => {
            error!(
                mint = %event.mint,
                bundle_id = %bundle_result.bundle_id,
                reason = %reason,
                "❌ Bundle failed"
            );
            monitor
                .send_error_alert(
                    "Jito",
                    &format!("Bundle {} failed: {}", bundle_result.bundle_id, reason),
                )
                .await?;
        }
        Ok(BundleStatus::Pending) => {
            info!(
                mint = %event.mint,
                bundle_id = %bundle_result.bundle_id,
                "⏳ Bundle still pending after poll window"
            );
        }
        Err(e) => {
            error!(error = %e, "Failed to poll bundle status");
        }
    }

    Ok(())
}
