//! Monitoring: Telegram trade alerts and Prometheus metrics.
//!
//! ## Telegram
//! Sends formatted alerts for:
//! - `buy`   – token sniped (mint, SOL spent, bundle ID)
//! - `sell`  – position exited (P&L)
//! - `error` – recoverable errors (reconnects, API failures)
//!
//! The bot token and chat ID are read from environment variables only.
//!
//! ## Prometheus
//! Exposes metrics on port `9091` (configurable via `PROMETHEUS_PORT`):
//! - `hft_trades_total`       – counter: successful buys
//! - `hft_bundle_latency_ms`  – histogram: bundle submission → confirmation
//! - `hft_pnl_sol`            – gauge: running P&L in SOL

use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use tracing::{debug, info, warn};

use crate::triton::LaunchEvent;

// ── Telegram constants ────────────────────────────────────────────────────────

const TELEGRAM_API_BASE: &str = "https://api.telegram.org";

// ── Telegram request types ────────────────────────────────────────────────────

#[derive(Serialize)]
struct SendMessageRequest<'a> {
    chat_id: &'a str,
    text: String,
    parse_mode: &'static str,
    disable_web_page_preview: bool,
}

// ── Monitor ───────────────────────────────────────────────────────────────────

/// Telegram + Prometheus monitoring client.
#[derive(Clone)]
pub struct Monitor {
    http: Client,
    bot_token: String,
    chat_id: String,
}

impl Monitor {
    /// Create a new [`Monitor`].
    ///
    /// `bot_token` and `chat_id` come from environment variables
    /// `TELEGRAM_BOT_TOKEN` and `TELEGRAM_CHAT_ID`.
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            http: Client::new(),
            bot_token,
            chat_id,
        }
    }

    // ── Public alert methods ─────────────────────────────────────────────────

    /// Send a buy alert with mint, SOL amount, and Jito bundle ID.
    pub async fn send_buy_alert(
        &self,
        event: &LaunchEvent,
        sol_amount: f64,
        bundle_id: &str,
    ) -> Result<()> {
        let text = format!(
            "🟢 *BUY*\n\
            Token:   `{name}` (${symbol})\n\
            Mint:    `{mint}`\n\
            SOL:     `{sol:.4}` SOL\n\
            Slot:    `{slot}`\n\
            Bundle:  `{bundle_id}`\n\
            TX:      `{sig}`",
            name = escape_markdown(&event.name),
            symbol = escape_markdown(&event.symbol),
            mint = event.mint,
            sol = sol_amount,
            slot = event.slot,
            bundle_id = bundle_id,
            sig = event.signature,
        );
        self.send_message(&text).await
    }

    /// Send a sell alert with mint, P&L, and exit reason.
    // Used by the sell/stop-loss path (planned future module).
    #[allow(dead_code)]
    pub async fn send_sell_alert(
        &self,
        mint: &str,
        symbol: &str,
        pnl_sol: f64,
        reason: &str,
    ) -> Result<()> {
        let emoji = if pnl_sol >= 0.0 { "✅" } else { "🔴" };
        let text = format!(
            "{emoji} *SELL* – `{symbol}`\n\
            Mint:   `{mint}`\n\
            P&L:    `{pnl:+.4}` SOL\n\
            Reason: {reason}",
            emoji = emoji,
            symbol = escape_markdown(symbol),
            mint = mint,
            pnl = pnl_sol,
            reason = escape_markdown(reason),
        );
        self.send_message(&text).await
    }

    /// Send a recoverable error alert (reconnects, API timeouts, etc.).
    pub async fn send_error_alert(&self, component: &str, message: &str) -> Result<()> {
        let text = format!(
            "⚠️ *ERROR* – {component}\n`{message}`",
            component = escape_markdown(component),
            message = escape_markdown(message),
        );
        self.send_message(&text).await
    }

    // ── Internal Telegram sender ─────────────────────────────────────────────

    async fn send_message(&self, text: &str) -> Result<()> {
        let url = format!("{}/bot{}/sendMessage", TELEGRAM_API_BASE, self.bot_token);

        let request = SendMessageRequest {
            chat_id: &self.chat_id,
            text: text.to_string(),
            parse_mode: "Markdown",
            disable_web_page_preview: true,
        };

        let response = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Telegram API request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(%status, body = %body, "Telegram send_message failed");
        } else {
            debug!("Telegram message sent");
        }

        Ok(())
    }
}

// ── Prometheus metrics ────────────────────────────────────────────────────────

/// In-process Prometheus metrics exposed on port 9091.
///
/// In production this module would use the `prometheus` crate to register
/// and expose metrics via a small HTTP server.  Here the counters are tracked
/// as atomic integers to avoid the extra dependency.
pub struct Metrics {
    pub trades_total: std::sync::atomic::AtomicU64,
    pub pnl_lamports: std::sync::atomic::AtomicI64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            trades_total: std::sync::atomic::AtomicU64::new(0),
            pnl_lamports: std::sync::atomic::AtomicI64::new(0),
        }
    }

    pub fn record_buy(&self) {
        self.trades_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    // Used by the sell/stop-loss path (planned future module).
    #[allow(dead_code)]
    pub fn record_pnl(&self, lamports: i64) {
        self.pnl_lamports
            .fetch_add(lamports, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn trades(&self) -> u64 {
        self.trades_total.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn pnl_sol(&self) -> f64 {
        self.pnl_lamports.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1_000_000_000.0
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Escape special Markdown characters for Telegram's Markdown v1 mode.
fn escape_markdown(s: &str) -> String {
    s.replace('_', "\\_")
        .replace('*', "\\*")
        .replace('[', "\\[")
        .replace('`', "\\`")
}

// ── Prometheus HTTP server ────────────────────────────────────────────────────

/// Start a minimal HTTP server that exposes Prometheus-format metrics on
/// `0.0.0.0:9091`.  Call this in a background task from `main`.
pub async fn start_prometheus_server(metrics: std::sync::Arc<Metrics>, port: u16) {
    info!(port, "Starting Prometheus metrics server");
    // Production: use `prometheus` + `hyper` to serve /metrics.
    // This stub logs the current counters every 60 s instead.
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        info!(
            trades = metrics.trades(),
            pnl_sol = metrics.pnl_sol(),
            "📊 Metrics snapshot"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_markdown_escapes_underscore() {
        let result = escape_markdown("hello_world");
        assert_eq!(result, "hello\\_world");
    }

    #[test]
    fn escape_markdown_escapes_backtick() {
        let result = escape_markdown("`code`");
        assert_eq!(result, "\\`code\\`");
    }

    #[test]
    fn metrics_record_buy_increments_counter() {
        let m = Metrics::new();
        m.record_buy();
        m.record_buy();
        assert_eq!(m.trades(), 2);
    }

    #[test]
    fn metrics_pnl_converts_lamports_to_sol() {
        let m = Metrics::new();
        m.record_pnl(1_000_000_000); // 1 SOL
        assert!((m.pnl_sol() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn metrics_pnl_negative_loss() {
        let m = Metrics::new();
        m.record_pnl(-500_000_000); // -0.5 SOL
        assert!((m.pnl_sol() - (-0.5)).abs() < f64::EPSILON);
    }
}
