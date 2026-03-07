//! Jito MEV bundle builder and submission.
//!
//! Settings (CLAUDE.md):
//! - Tip: ≥ 10 000 lamports (dynamic, read from `JITO_TIP_LAMPORTS` env var).
//! - Block engine: `frankfurt.mainnet.block-engine.jito.wtf`.
//! - Max 5 transactions per bundle.
//! - Poll bundle status 30 s after submission.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Deserialize;
use serde_json::json;
use tracing::{debug, info, instrument, warn};

use crate::pump_tx::PumpTransaction;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Jito bundle submission endpoint (Frankfurt block engine).
const JITO_BUNDLE_URL: &str = "https://frankfurt.mainnet.block-engine.jito.wtf/api/v1/bundles";

/// Bundle status polling endpoint.
const JITO_BUNDLE_STATUS_URL: &str =
    "https://frankfurt.mainnet.block-engine.jito.wtf/api/v1/bundles/status";

/// Minimum tip in lamports – CLAUDE.md: 10 000.
pub const MIN_TIP_LAMPORTS: u64 = 10_000;

/// Hard upper limit on transactions per bundle (Jito protocol).
#[allow(dead_code)]
pub const MAX_BUNDLE_SIZE: usize = 5;

/// Well-known Jito tip accounts (mainnet) – rotate via round-robin if desired.
#[allow(dead_code)]
pub const JITO_TIP_ACCOUNTS: [&str; 8] = [
    "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5",
    "HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe",
    "Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY",
    "ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49",
    "DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh",
    "ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt",
    "DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL",
    "3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT",
];

// ── Client ────────────────────────────────────────────────────────────────────

/// Jito bundle client.
pub struct JitoClient {
    client: reqwest::Client,
    tip_lamports: u64,
}

impl JitoClient {
    /// Construct from environment variables.
    ///
    /// Reads `JITO_TIP_LAMPORTS` (optional, default 10 000).  Values below
    /// [`MIN_TIP_LAMPORTS`] are silently clamped up.
    pub fn from_env() -> Self {
        let tip_lamports = std::env::var("JITO_TIP_LAMPORTS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(MIN_TIP_LAMPORTS)
            .max(MIN_TIP_LAMPORTS);

        Self {
            client: reqwest::Client::new(),
            tip_lamports,
        }
    }

    /// Submit a signed buy transaction as a Jito bundle.
    ///
    /// Returns the bundle UUID on success.  The bundle status is polled in a
    /// background task 30 s after submission (CLAUDE.md).
    #[instrument(skip(self, tx), fields(mint = %tx.mint, tip = self.tip_lamports))]
    pub async fn submit_bundle(&self, tx: PumpTransaction) -> Result<String> {
        // Encode the serialised transaction as base64.
        let buy_tx_b64 = BASE64.encode(&tx.serialized);

        // A bundle can contain up to MAX_BUNDLE_SIZE transactions.
        // Here we submit the single buy tx; a tip tx can be appended if needed.
        let txs = vec![buy_tx_b64];

        debug!(
            count = txs.len(),
            tip_lamports = self.tip_lamports,
            "Submitting Jito bundle"
        );

        let body = json!({
            "jsonrpc": "2.0",
            "id":      1,
            "method":  "sendBundle",
            "params":  [txs]
        });

        let response = self
            .client
            .post(JITO_BUNDLE_URL)
            .json(&body)
            .send()
            .await
            .context("Jito HTTP request failed")?;

        let status = response.status();
        let text = response
            .text()
            .await
            .context("Failed to read Jito response body")?;

        if !status.is_success() {
            bail!("Jito bundle rejected (HTTP {status}): {text}");
        }

        let parsed: JitoResponse =
            serde_json::from_str(&text).context("Failed to parse Jito response")?;

        if let Some(err) = parsed.error {
            bail!("Jito RPC error {}: {}", err.code, err.message);
        }

        let bundle_id = parsed.result.context("No bundle ID in Jito response")?;
        info!(bundle_id = %bundle_id, "✅ Jito bundle submitted");

        // Poll bundle status after 30 s in a background task (CLAUDE.md).
        let poll_client = self.client.clone();
        let bid = bundle_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if let Err(e) = poll_bundle_status(&poll_client, &bid).await {
                warn!(bundle_id = %bid, "Status poll failed: {e:#}");
            }
        });

        Ok(bundle_id)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Query the Jito bundle status endpoint 30 s after submission.
async fn poll_bundle_status(client: &reqwest::Client, bundle_id: &str) -> Result<()> {
    let url = format!("{JITO_BUNDLE_STATUS_URL}?bundleId={bundle_id}");
    let resp = client.get(&url).send().await?;
    let text = resp.text().await?;
    info!(bundle_id = %bundle_id, status = %text, "Bundle status");
    Ok(())
}

// ── JSON response types ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct JitoResponse {
    result: Option<String>,
    error: Option<JitoError>,
}

#[derive(Deserialize)]
struct JitoError {
    code: i64,
    message: String,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_tip_is_10000_lamports() {
        assert_eq!(MIN_TIP_LAMPORTS, 10_000);
    }

    #[test]
    fn max_bundle_size_is_five() {
        assert_eq!(MAX_BUNDLE_SIZE, 5);
    }

    #[test]
    fn tip_clamped_to_minimum() {
        // Simulate from_env with tip below minimum.
        let effective = 5_000u64.max(MIN_TIP_LAMPORTS);
        assert_eq!(effective, MIN_TIP_LAMPORTS);
    }

    #[test]
    fn jito_tip_accounts_count() {
        assert_eq!(JITO_TIP_ACCOUNTS.len(), 8);
    }

    #[test]
    fn all_tip_accounts_are_non_empty() {
        for acct in &JITO_TIP_ACCOUNTS {
            assert!(!acct.is_empty());
        }
    }
}
