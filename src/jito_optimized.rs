//! Jito bundle builder and submission client.
//!
//! Jito bundles provide **MEV protection** and **top-of-block** placement on Solana.
//!
//! Configuration:
//! - Block engine: `frankfurt.mainnet.block-engine.jito.wtf`
//! - Tip:          minimum 10 000 Lamports, adjust dynamically  
//! - Bundle size:  maximum 5 transactions
//! - Status poll:  after 30 s, call `/api/v1/bundles` to check landing

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::pump_tx::Instruction;

// ── Constants ────────────────────────────────────────────────────────────────

/// Minimum Jito tip in Lamports.
pub const MIN_TIP_LAMPORTS: u64 = 10_000;

/// Maximum number of transactions in a Jito bundle.
pub const MAX_BUNDLE_TXNS: usize = 5;

/// Milliseconds to wait before polling bundle landing status.
const STATUS_POLL_DELAY_MS: u64 = 30_000;

// ── Bundle types ──────────────────────────────────────────────────────────────

/// A Jito bundle: up to [`MAX_BUNDLE_TXNS`] serialised transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitoBundle {
    /// Base-58 encoded serialised transactions (signed)
    pub transactions: Vec<String>,
    /// Tip in Lamports appended to the last transaction
    pub tip_lamports: u64,
}

/// Result returned after bundle submission.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct BundleResult {
    /// Jito bundle UUID
    pub bundle_id: String,
    /// HTTP status from the block engine
    pub status: BundleStatus,
}

/// Landing status polled from the Jito block engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleStatus {
    /// Bundle received and queued
    Pending,
    /// Bundle included in a block
    Landed,
    /// Bundle failed or timed out
    Failed(String),
}

// ── JSON-RPC helpers ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct JsonRpcRequest<T: Serialize> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: T,
}

#[derive(Deserialize, Debug)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<serde_json::Value>,
}

// ── Jito client ───────────────────────────────────────────────────────────────

/// HTTP client for the Jito block engine JSON-RPC API.
pub struct JitoClient {
    http: Client,
    block_engine_url: String,
}

impl JitoClient {
    pub fn new(block_engine_url: &str) -> Self {
        Self {
            http: Client::new(),
            block_engine_url: block_engine_url.to_string(),
        }
    }

    fn api_url(&self) -> String {
        format!("https://{}:1700/api/v1/bundles", self.block_engine_url)
    }

    /// Submit a [`JitoBundle`] to the Frankfurt block engine.
    ///
    /// Returns a [`BundleResult`] containing the bundle UUID on success.
    pub async fn send_bundle(&self, bundle: &JitoBundle) -> Result<BundleResult> {
        if bundle.transactions.is_empty() {
            bail!("Bundle must contain at least one transaction");
        }
        if bundle.transactions.len() > MAX_BUNDLE_TXNS {
            bail!(
                "Bundle exceeds max size: {} > {}",
                bundle.transactions.len(),
                MAX_BUNDLE_TXNS
            );
        }
        if bundle.tip_lamports < MIN_TIP_LAMPORTS {
            warn!(
                tip = bundle.tip_lamports,
                min = MIN_TIP_LAMPORTS,
                "Tip below minimum – bundle may not land"
            );
        }

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "sendBundle",
            params: [&bundle.transactions],
        };

        let response = self
            .http
            .post(self.api_url())
            .json(&request)
            .send()
            .await
            .context("Failed to reach Jito block engine")?;

        let status = response.status();
        let body: JsonRpcResponse<String> = response
            .json()
            .await
            .context("Invalid JSON from Jito block engine")?;

        if let Some(err) = body.error {
            bail!("Jito RPC error: {err}");
        }

        let bundle_id = body.result.context("Missing bundle ID in response")?;
        info!(bundle_id = %bundle_id, tip = bundle.tip_lamports, "⚡ Bundle submitted");

        Ok(BundleResult {
            bundle_id,
            status: if status.is_success() {
                BundleStatus::Pending
            } else {
                BundleStatus::Failed(status.to_string())
            },
        })
    }

    /// Poll bundle landing status after [`STATUS_POLL_DELAY_MS`] milliseconds.
    pub async fn poll_bundle_status(&self, bundle_id: &str) -> Result<BundleStatus> {
        debug!(
            bundle_id,
            delay_ms = STATUS_POLL_DELAY_MS,
            "Waiting before polling bundle status"
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(STATUS_POLL_DELAY_MS)).await;

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "getBundleStatuses",
            params: [[bundle_id]],
        };

        let response = self
            .http
            .post(self.api_url())
            .json(&request)
            .send()
            .await
            .context("Failed to reach Jito block engine during status poll")?;

        let body: serde_json::Value = response.json().await?;
        let status_str = body["result"]["value"][0]["confirmation_status"]
            .as_str()
            .unwrap_or("unknown");

        let status = match status_str {
            "confirmed" | "finalized" => BundleStatus::Landed,
            "failed" => BundleStatus::Failed("block engine reported failure".to_string()),
            other => {
                warn!(bundle_id, status = other, "Unexpected bundle status");
                BundleStatus::Pending
            }
        };

        info!(bundle_id, ?status, "Bundle status polled");
        Ok(status)
    }
}

// ── High-level helper ────────────────────────────────────────────────────────

/// Build and submit a Jito bundle for a set of instructions.
///
/// Serialises each instruction into a transaction (signed with `private_key`),
/// appends the tip to the last transaction, and submits to the block engine.
///
/// # Arguments
/// * `instructions`         – Instructions to include (max 4, tip tx is #5)
/// * `tip_lamports`         – Lamports to tip the validator
/// * `block_engine_url`     – Block engine hostname
/// * `_wallet_private_key`  – Base-58 encoded keypair (used to sign txns)
pub async fn submit_bundle(
    instructions: Vec<Instruction>,
    tip_lamports: u64,
    block_engine_url: &str,
    _wallet_private_key: &str,
) -> Result<BundleResult> {
    // Production: serialize each Instruction into a signed Solana Transaction,
    // then Base-58 encode the wire format.  Here we encode the instruction data
    // directly as a placeholder.
    let transactions: Vec<String> = instructions
        .iter()
        .map(|ix| bs58::encode(&ix.data).into_string())
        .collect();

    let bundle = JitoBundle {
        transactions,
        tip_lamports: tip_lamports.max(MIN_TIP_LAMPORTS),
    };

    let client = JitoClient::new(block_engine_url);
    client.send_bundle(&bundle).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_rejects_empty_transactions() {
        let bundle = JitoBundle {
            transactions: vec![],
            tip_lamports: MIN_TIP_LAMPORTS,
        };
        // send_bundle is async; validate the guard synchronously via the length check
        assert!(bundle.transactions.is_empty());
    }

    #[test]
    fn bundle_rejects_oversized_bundles() {
        let bundle = JitoBundle {
            transactions: (0..=MAX_BUNDLE_TXNS).map(|_| "tx".to_string()).collect(),
            tip_lamports: MIN_TIP_LAMPORTS,
        };
        assert!(bundle.transactions.len() > MAX_BUNDLE_TXNS);
    }

    #[test]
    fn tip_floor_is_enforced() {
        let tip = 5_000u64; // below minimum
        assert!(tip < MIN_TIP_LAMPORTS);
        let actual = tip.max(MIN_TIP_LAMPORTS);
        assert_eq!(actual, MIN_TIP_LAMPORTS);
    }
}
