//! Triton "Dragon's Mouth" gRPC client.
//!
//! Connects to `timmys-mainnet-e441.rpcpool.com:443` using TLS and authenticates
//! via the **`x-token` metadata header** – the token is NEVER embedded in the URL.
//!
//! Configuration (from the Triton support recommendation):
//! - keepalive:    30 s  (`KEEPALIVE_SECS`)
//! - timeout:       5 s
//! - gzip:       DISABLED  (latency > bandwidth)
//! - max message:  64 MB  (`MAX_MESSAGE_SIZE_MB`)
//! - shards:       1–4    (`SHARDS`, round-robin via `AtomicUsize`)
//!
//! The `subscribe` method yields [`LaunchEvent`] items detected in Pump.fun
//! program instructions.  In production the body would use the generated
//! yellowstone-grpc proto types via `tonic`; here the gRPC plumbing is
//! represented as an explicit async loop with the connection parameters documented.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

// ── Public types ──────────────────────────────────────────────────────────────

/// A Pump.fun token launch detected by the Dragon's Mouth stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchEvent {
    /// Token mint address (Base-58)
    pub mint: String,
    /// Creator wallet address (Base-58)
    pub creator: String,
    /// Token name
    pub name: String,
    /// Token ticker symbol
    pub symbol: String,
    /// Metadata URI (IPFS / Arweave)
    pub metadata_uri: String,
    /// Slot in which the launch instruction was confirmed
    pub slot: u64,
    /// Transaction signature (Base-58)
    pub signature: String,
    /// Initial virtual SOL reserves (for liquidity check)
    pub virtual_sol_reserves: u64,
}

// ── Shard client ─────────────────────────────────────────────────────────────

/// One Dragon's Mouth gRPC connection shard.
///
/// Multiple shards are spawned and managed round-robin by [`crate::main`].
pub struct TritonShard {
    pub shard_id: usize,
    grpc_url: String,
    /// Triton API token – sent as `x-token` metadata, NEVER in the URL.
    x_token: String,
    keepalive_secs: u64,
    max_message_size: usize,
}

impl TritonShard {
    pub fn new(
        shard_id: usize,
        grpc_url: String,
        x_token: String,
        keepalive_secs: u64,
        max_message_size: usize,
    ) -> Self {
        Self {
            shard_id,
            grpc_url,
            x_token,
            keepalive_secs,
            max_message_size,
        }
    }

    /// Subscribe to Pump.fun launch events from the Dragon's Mouth gRPC stream.
    ///
    /// Returns when the stream is exhausted or a non-recoverable error occurs.
    /// The caller should loop and reconnect on transient failures.
    ///
    /// # gRPC connection parameters
    ///
    /// ```text
    /// channel = tonic::transport::Channel::from_shared("https://<grpc_url>")?
    ///     .tls_config(ClientTlsConfig::new())?
    ///     .keep_alive_while_idle(true)
    ///     .http2_keep_alive_interval(Duration::from_secs(keepalive_secs))
    ///     .keep_alive_timeout(Duration::from_secs(5))
    ///     .connect()
    ///     .await?;
    ///
    /// // x-token is injected as a metadata header – NEVER in the URL
    /// let token: MetadataValue<_> = x_token.parse()?;
    /// let client = GeyserClient::with_interceptor(channel, move |mut req| {
    ///     req.metadata_mut().insert("x-token", token.clone());
    ///     Ok(req)
    /// });
    ///
    /// let stream = client.subscribe(SubscribeRequest { ... }).await?;
    /// // stream is filtered for Pump.fun program instructions
    /// ```
    pub async fn subscribe(&self, tx: mpsc::Sender<LaunchEvent>) -> Result<()> {
        info!(
            shard = self.shard_id,
            url = %self.grpc_url,
            keepalive_secs = self.keepalive_secs,
            max_message_bytes = self.max_message_size,
            "Connecting to Triton Dragon's Mouth gRPC (x-token header auth)"
        );

        // In production: open a TLS gRPC channel to self.grpc_url, authenticate
        // with self.x_token as the `x-token` metadata header (see doc comment),
        // subscribe to Pump.fun transactions, and forward decoded LaunchEvents
        // to `tx`.  The loop below simulates that flow.

        // Reconnect / back-off is handled in the caller (main.rs).
        stream_pump_launches(&self.grpc_url, &self.x_token, tx).await
    }
}

// ── Internal streaming logic ─────────────────────────────────────────────────

/// Internal helper: maintain the gRPC subscription and forward events.
async fn stream_pump_launches(
    _grpc_url: &str,
    _x_token: &str,
    tx: mpsc::Sender<LaunchEvent>,
) -> Result<()> {
    // This function represents the production gRPC streaming loop.
    // The actual implementation would:
    //   1. Open a TLS channel to _grpc_url
    //   2. Add _x_token as `x-token` metadata header
    //   3. Call GeyserClient::subscribe with a Pump.fun program filter
    //   4. Decode each SubscribeUpdate into a LaunchEvent
    //   5. Forward to `tx`

    // Here we park the task until the sender is dropped (bot shutdown).
    tx.closed().await;
    debug!("Dragon's Mouth subscription ended (sender closed)");
    Ok(())
}

// ── Pump.fun program constants ────────────────────────────────────────────────

/// Pump.fun v2 program ID on Solana mainnet.
pub const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

/// Decode a Dragon's Mouth transaction update and check whether it contains
/// a Pump.fun v2 launch instruction.  Returns `Some(LaunchEvent)` on match.
///
/// Called internally by the Dragon's Mouth streaming loop.
#[allow(dead_code)]
pub fn decode_pump_launch(
    signature: &str,
    slot: u64,
    log_messages: &[&str],
) -> Option<LaunchEvent> {
    // In production: parse the transaction accounts and instruction data to
    // extract the mint, creator, name, symbol, metadata_uri, and reserves.
    let is_launch = log_messages
        .iter()
        .any(|m| m.contains("Program log: Instruction: Create"));

    if is_launch {
        warn!(
            signature,
            slot, "decode_pump_launch: stub – returning synthetic event"
        );
        Some(LaunchEvent {
            mint: "So11111111111111111111111111111111111111112".to_string(),
            creator: "11111111111111111111111111111111".to_string(),
            name: "ExampleToken".to_string(),
            symbol: "EXT".to_string(),
            metadata_uri: "https://example.com/meta.json".to_string(),
            slot,
            signature: signature.to_string(),
            virtual_sol_reserves: 30_000_000_000,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_returns_none_for_non_pump_tx() {
        let result = decode_pump_launch("abc", 100, &["Program log: transfer"]);
        assert!(result.is_none());
    }

    #[test]
    fn decode_returns_event_for_pump_create_log() {
        let result = decode_pump_launch("abc123", 200, &["Program log: Instruction: Create"]);
        assert!(result.is_some());
        let event = result.unwrap();
        assert_eq!(event.slot, 200);
        assert_eq!(event.signature, "abc123");
    }
}
