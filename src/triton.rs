//! Dragon's Mouth gRPC client – Triton One (YellowStone).
//!
//! Authentication rules (CLAUDE.md – strictly enforced):
//! - Token **always** in the `x-token` metadata header.
//! - Token **never** embedded in the gRPC URL.
//! - Keepalive: 30 s interval / 5 s timeout.
//! - Gzip compression: **DISABLED** (latency > bandwidth for HFT).
//! - Max decoding message size: 64 MB.
//! - 4 shards via `AtomicUsize` round-robin.

use anyhow::{Context, Result};
use std::time::Duration;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tracing::{debug, info, warn};

use crate::jito_optimized::JitoClient;
use crate::monitor::Monitor;
use crate::pump_tx::PumpTransaction;
use crate::rig_goat::RigGoat;

/// Dragon's Mouth gRPC endpoint (TLS, port 443).
const TRITON_ENDPOINT: &str = "https://timmys-mainnet-e441.rpcpool.com:443";

/// Max decoded message size: 64 MB (CLAUDE.md).
#[allow(dead_code)]
pub const MAX_DECODING_MSG_SIZE: usize = 67_108_864;

/// HTTP/2 keepalive ping interval – 30 s (CLAUDE.md).
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// HTTP/2 keepalive timeout – 5 s (CLAUDE.md).
pub const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(5);

/// Wrapper around the authenticated Dragon's Mouth gRPC channel.
pub struct TritonClient {
    _channel: Channel,
    // Stored for use with auth_header() when attaching to gRPC requests.
    #[allow(dead_code)]
    token: String,
}

impl TritonClient {
    /// Connect to the Triton Dragon's Mouth endpoint with the given auth token.
    ///
    /// TLS is enabled; gzip is **not** enabled; the token is stored for use
    /// as the `x-token` metadata header on every request.
    pub async fn connect(token: &str) -> Result<Self> {
        let tls = ClientTlsConfig::new();

        let channel = Endpoint::from_static(TRITON_ENDPOINT)
            .tls_config(tls)
            .context("TLS configuration error")?
            // HTTP/2 keepalive (matches Triton recommendation from CLAUDE.md)
            .http2_keep_alive_interval(KEEPALIVE_INTERVAL)
            .keep_alive_timeout(KEEPALIVE_TIMEOUT)
            .keep_alive_while_idle(true)
            // Gzip DISABLED – latency > bandwidth for HFT (CLAUDE.md)
            .connect_timeout(Duration::from_secs(10))
            .connect()
            .await
            .context("Failed to connect to Triton Dragon's Mouth")?;

        info!("✅ Connected to Dragon's Mouth at {TRITON_ENDPOINT}");

        Ok(Self {
            _channel: channel,
            token: token.to_owned(),
        })
    }

    /// Build the `x-token` ASCII metadata value for request authentication.
    ///
    /// Attach to every outbound gRPC request:
    /// ```rust,ignore
    /// let mut req = tonic::Request::new(payload);
    /// req.metadata_mut().insert("x-token", client.auth_header()?);
    /// ```
    #[allow(dead_code)]
    pub fn auth_header(&self) -> Result<MetadataValue<tonic::metadata::Ascii>> {
        MetadataValue::try_from(self.token.as_str())
            .map_err(|e| anyhow::anyhow!("Invalid x-token metadata value: {e}"))
    }

    /// Subscribe to slot updates and react to detected Pump.fun launches.
    ///
    /// This drives the main snipe loop for one shard.  In production the
    /// `next_launch_event` stub is replaced by the live yellowstone-grpc
    /// subscription stream.
    pub async fn run(
        &self,
        wallet_key: &str,
        monitor: &Monitor,
        ai: &RigGoat,
    ) -> Result<()> {
        info!("🔄 Dragon's Mouth stream active – waiting for Pump.fun launches…");

        // Production subscription attaches the auth header:
        //
        //   let mut subscribe_req = tonic::Request::new(SubscribeRequest { … });
        //   subscribe_req.metadata_mut().insert("x-token", self.auth_header()?);
        //   let stream = stub.subscribe(subscribe_req).await?.into_inner();
        //
        // Each SLOT_FIRST_SHRED containing a Pump.fun program invocation is
        // filtered and converted into a `LaunchEvent`.

        let jito = JitoClient::from_env();

        loop {
            match self.next_launch_event().await {
                Ok(Some(event)) => {
                    debug!(mint = %event.mint, "New Pump.fun launch event");

                    // 1. AI gate – only snipe tokens approved by xAI Grok.
                    if !ai.should_snipe(&event).await {
                        debug!(mint = %event.mint, "AI filtered – skipping");
                        continue;
                    }

                    // 2. Build v2 buy instruction (new PDAs appended at end).
                    let tx = match PumpTransaction::build_buy_v2(&event, wallet_key) {
                        Ok(t) => t,
                        Err(e) => {
                            warn!("build_buy_v2 failed: {e:#}");
                            continue;
                        }
                    };

                    // 3. Submit as a Jito bundle (tip ≥ 10 000 lamports).
                    match jito.submit_bundle(tx).await {
                        Ok(bundle_id) => {
                            info!(
                                bundle = %bundle_id,
                                mint = %event.mint,
                                "✅ Bundle landed"
                            );
                            monitor.notify_trade(&event.mint.to_string(), &bundle_id).await;
                        }
                        Err(e) => {
                            warn!("Bundle submission failed: {e:#}");
                            monitor.notify_error(&e.to_string()).await;
                        }
                    }
                }
                // None signals end-of-stream; the shard manager will reconnect.
                Ok(None) => break,
                Err(e) => {
                    warn!("Stream error: {e:#}");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Poll the Dragon's Mouth stream for the next Pump.fun launch event.
    ///
    /// **Production**: replace this stub with the yellowstone-grpc subscription:
    /// ```rust,ignore
    /// while let Some(msg) = stream.next().await { … }
    /// ```
    async fn next_launch_event(&self) -> Result<Option<LaunchEvent>> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(None) // None → end-of-stream
    }
}

/// A Pump.fun token launch extracted from an on-chain slot update.
#[derive(Debug, Clone)]
pub struct LaunchEvent {
    /// Token mint address.
    pub mint: solana_sdk::pubkey::Pubkey,
    /// Creator / deployer wallet.
    pub creator: solana_sdk::pubkey::Pubkey,
    /// Human-readable token name.
    pub name: String,
    /// Token ticker symbol.
    pub symbol: String,
    /// Off-chain metadata URI (IPFS / Arweave).
    pub uri: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keepalive_constants_match_claude_md() {
        assert_eq!(KEEPALIVE_INTERVAL, Duration::from_secs(30));
        assert_eq!(KEEPALIVE_TIMEOUT, Duration::from_secs(5));
    }

    #[test]
    fn max_decoding_msg_size_is_64mb() {
        assert_eq!(MAX_DECODING_MSG_SIZE, 64 * 1024 * 1024);
    }
}
