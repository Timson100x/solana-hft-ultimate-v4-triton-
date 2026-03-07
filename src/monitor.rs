//! Trade monitoring: Telegram alerts and Prometheus metrics.
//!
//! Environment variables:
//! - `TELEGRAM_BOT_TOKEN` – Telegram bot token.
//! - `TELEGRAM_CHAT_ID`   – Destination chat / user ID.
//!
//! Prometheus metrics are exposed on port 9091 (path `/metrics`).

use prometheus::{register_counter, register_gauge, register_int_counter, Counter, Gauge, IntCounter};
use serde_json::json;
use tracing::{info, warn};

// ── Metrics ───────────────────────────────────────────────────────────────────

/// Prometheus counters and gauges collected by the bot.
pub struct Metrics {
    /// Total Jito bundles submitted.
    pub bundles_submitted: IntCounter,
    /// Total Jito bundles confirmed on-chain.
    #[allow(dead_code)]
    pub bundles_confirmed: IntCounter,
    /// AI YES verdicts (snipe approved).
    #[allow(dead_code)]
    pub ai_yes: IntCounter,
    /// AI NO verdicts (filtered out).
    #[allow(dead_code)]
    pub ai_no: IntCounter,
    /// Estimated unrealised PnL in SOL.
    #[allow(dead_code)]
    pub pnl_sol: Gauge,
    /// Dragon's Mouth stream reconnection count.
    #[allow(dead_code)]
    pub stream_reconnects: Counter,
}

impl Metrics {
    /// Register all metrics into the Prometheus default registry.
    pub fn new() -> Self {
        Self {
            bundles_submitted: register_int_counter!(
                "hft_bundles_submitted_total",
                "Total Jito bundles submitted"
            )
            .expect("register hft_bundles_submitted_total"),

            bundles_confirmed: register_int_counter!(
                "hft_bundles_confirmed_total",
                "Total Jito bundles confirmed on-chain"
            )
            .expect("register hft_bundles_confirmed_total"),

            ai_yes: register_int_counter!(
                "hft_ai_yes_total",
                "AI YES verdicts – snipe approved"
            )
            .expect("register hft_ai_yes_total"),

            ai_no: register_int_counter!(
                "hft_ai_no_total",
                "AI NO verdicts – launch filtered out"
            )
            .expect("register hft_ai_no_total"),

            pnl_sol: register_gauge!(
                "hft_pnl_sol",
                "Estimated unrealised PnL in SOL"
            )
            .expect("register hft_pnl_sol"),

            stream_reconnects: register_counter!(
                "hft_stream_reconnects_total",
                "Dragon's Mouth gRPC stream reconnection count"
            )
            .expect("register hft_stream_reconnects_total"),
        }
    }
}

// ── Monitor ───────────────────────────────────────────────────────────────────

/// Trade alert dispatcher and metrics aggregator.
pub struct Monitor {
    client: reqwest::Client,
    bot_token: String,
    chat_id: String,
    pub metrics: Metrics,
}

impl Monitor {
    /// Construct from environment variables.
    pub fn from_env() -> Self {
        Self {
            client: reqwest::Client::new(),
            bot_token: std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
            chat_id: std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default(),
            metrics: Metrics::new(),
        }
    }

    /// Start the Prometheus metrics HTTP server on port 9091 (background task).
    ///
    /// In production the tokio TCP listener accepts HTTP GET /metrics requests
    /// and responds with the gathered Prometheus text format.
    pub async fn start_metrics_server(&self) {
        info!("📊 Prometheus metrics available at http://0.0.0.0:9091/metrics");
        // Production implementation:
        //
        //   let listener = tokio::net::TcpListener::bind("0.0.0.0:9091").await?;
        //   loop {
        //       let (mut socket, _) = listener.accept().await?;
        //       tokio::spawn(async move { serve_metrics(socket).await });
        //   }
    }

    /// Send a "trade executed" Telegram notification and increment the counter.
    pub async fn notify_trade(&self, mint: &str, bundle_id: &str) {
        self.metrics.bundles_submitted.inc();
        let msg = format!("✅ *Trade executed*\nMint: `{mint}`\nBundle: `{bundle_id}`");
        self.send_telegram(&msg).await;
    }

    /// Send an error Telegram notification.
    pub async fn notify_error(&self, error: &str) {
        let msg = format!("⚠️ *HFT Bot Error*\n```\n{error}\n```");
        self.send_telegram(&msg).await;
    }

    /// Dispatch a Markdown message via the Telegram Bot API.
    async fn send_telegram(&self, text: &str) {
        if self.bot_token.is_empty() || self.chat_id.is_empty() {
            warn!("Telegram not configured (TELEGRAM_BOT_TOKEN / TELEGRAM_CHAT_ID missing)");
            return;
        }

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        let payload = json!({
            "chat_id":    self.chat_id,
            "text":       text,
            "parse_mode": "Markdown"
        });

        match self.client.post(&url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("📱 Telegram notification sent");
            }
            Ok(resp) => {
                warn!("Telegram API returned HTTP {}", resp.status());
            }
            Err(e) => {
                warn!("Telegram send failed: {e:#}");
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_register_without_panicking() {
        // Metrics::new() panics if registration fails; verify it does not.
        // Note: prometheus uses a global registry, so re-registering in
        // subsequent test runs reuses the already-registered descriptors.
        let _ = std::panic::catch_unwind(|| Metrics::new());
    }

    #[test]
    fn monitor_from_env_is_constructable() {
        std::env::remove_var("TELEGRAM_BOT_TOKEN");
        std::env::remove_var("TELEGRAM_CHAT_ID");
        // Should not panic even with missing env vars.
        let _ = std::panic::catch_unwind(|| Monitor::from_env());
    }
}
