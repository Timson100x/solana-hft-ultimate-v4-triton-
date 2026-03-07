//! AI decision engine using the Rig framework and xAI / Grok.
//!
//! Calls `llama-3.3-70b-versatile` (via xAI's OpenAI-compatible API) for each
//! detected Pump.fun launch and returns a **YES** / **NO** verdict.
//!
//! Only tokens that receive **YES** proceed to the buy phase; this filters
//! rug-pulls and low-quality launches before any on-chain transaction is sent.
//!
//! The xAI API key is always read from the `XAI_API_KEY` environment variable
//! – it is NEVER hardcoded.

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::triton::LaunchEvent;

// ── Constants ─────────────────────────────────────────────────────────────────

const XAI_API_URL: &str = "https://api.x.ai/v1/chat/completions";
const AI_MODEL: &str = "grok-3-mini";

// ── Types ─────────────────────────────────────────────────────────────────────

/// Result of the AI analysis for a single launch event.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AiDecision {
    /// `true` = buy, `false` = skip
    pub should_buy: bool,
    /// Short explanation from the model
    pub reason: String,
    /// Raw model output (for logging / debugging)
    pub raw_response: String,
}

// ── xAI request / response types ─────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize, Debug)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize, Debug)]
struct ChatResponseMessage {
    content: String,
}

// ── RigGoat AI engine ─────────────────────────────────────────────────────────

/// AI-powered YES/NO filter for Pump.fun launches.
///
/// Wraps the xAI Grok API via the Rig-compatible interface.
/// The API key is sourced exclusively from the `XAI_API_KEY` env var.
#[derive(Clone)]
pub struct RigGoat {
    http: Client,
    /// xAI API key (from environment, never hardcoded)
    api_key: String,
}

impl RigGoat {
    /// Construct a new [`RigGoat`] with the given API key.
    ///
    /// In `main.rs` this is called as:
    /// ```rust,ignore
    /// let ai = RigGoat::new(config.xai_api_key.clone());
    /// ```
    pub fn new(api_key: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
        }
    }

    /// Analyse a Pump.fun launch event and return a buy/skip decision.
    ///
    /// The model is prompted to reply with exactly `YES` or `NO` followed by
    /// a one-sentence reason.  Any ambiguous response defaults to `NO` for
    /// safety.
    pub async fn analyze_launch(&self, event: &LaunchEvent) -> Result<AiDecision> {
        let prompt = build_prompt(event);

        debug!(
            mint = %event.mint,
            symbol = %event.symbol,
            "Sending launch to xAI for analysis"
        );

        let request = ChatRequest {
            model: AI_MODEL,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: "You are a Solana memecoin sniper AI. \
                        Evaluate each token launch and reply with exactly: \
                        YES or NO, followed by a colon and a one-sentence reason. \
                        Example: YES: Strong community and low initial liquidity. \
                        Avoid tokens with suspicious names, copied metadata, or \
                        very low liquidity. Be conservative – safety first."
                        .to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: prompt,
                },
            ],
            max_tokens: 64,
            temperature: 0.1,
        };

        let response = self
            .http
            .post(XAI_API_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .context("xAI API request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(
                mint = %event.mint,
                %status,
                body = %body,
                "xAI API error – defaulting to NO"
            );
            return Ok(AiDecision {
                should_buy: false,
                reason: format!("xAI API error {status}"),
                raw_response: body,
            });
        }

        let chat: ChatResponse = response
            .json()
            .await
            .context("Failed to parse xAI response")?;

        let raw = chat
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .unwrap_or_default();

        let decision = parse_decision(&raw);
        info!(
            mint = %event.mint,
            symbol = %event.symbol,
            buy = decision.should_buy,
            reason = %decision.reason,
            "🤖 AI decision"
        );
        Ok(decision)
    }
}

// ── Prompt builder ────────────────────────────────────────────────────────────

fn build_prompt(event: &LaunchEvent) -> String {
    format!(
        "Token launch detected:\n\
        Mint:             {}\n\
        Name:             {}\n\
        Symbol:           {}\n\
        Creator:          {}\n\
        Metadata URI:     {}\n\
        Slot:             {}\n\
        Virtual SOL reserves: {} lamports\n\
        \n\
        Should I snipe this token? Reply YES or NO with a brief reason.",
        event.mint,
        event.name,
        event.symbol,
        event.creator,
        event.metadata_uri,
        event.slot,
        event.virtual_sol_reserves,
    )
}

// ── Response parser ───────────────────────────────────────────────────────────

/// Parse the model's text reply into an [`AiDecision`].
///
/// Accepts formats like `"YES: reason"`, `"NO: reason"`, `"YES"`, `"NO"`.
/// Any response that does not start with `YES` (case-insensitive) is treated
/// as a `NO` for safety.
fn parse_decision(raw: &str) -> AiDecision {
    let upper = raw.to_uppercase();
    let should_buy = upper.starts_with("YES");

    let reason = raw
        .split_once(':')
        .map(|x| x.1.trim().to_string())
        .unwrap_or_else(|| raw.to_string());

    AiDecision {
        should_buy,
        reason,
        raw_response: raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_decision_yes_with_reason() {
        let d = parse_decision("YES: Strong community metrics");
        assert!(d.should_buy);
        assert_eq!(d.reason, "Strong community metrics");
    }

    #[test]
    fn parse_decision_no_with_reason() {
        let d = parse_decision("NO: Suspicious metadata");
        assert!(!d.should_buy);
        assert_eq!(d.reason, "Suspicious metadata");
    }

    #[test]
    fn parse_decision_bare_yes() {
        let d = parse_decision("YES");
        assert!(d.should_buy);
    }

    #[test]
    fn parse_decision_bare_no() {
        let d = parse_decision("NO");
        assert!(!d.should_buy);
    }

    #[test]
    fn parse_decision_ambiguous_defaults_to_no() {
        let d = parse_decision("Maybe: unclear signal");
        assert!(!d.should_buy);
    }

    #[test]
    fn build_prompt_contains_mint() {
        let event = LaunchEvent {
            mint: "TestMint123".to_string(),
            creator: "Creator456".to_string(),
            name: "TestToken".to_string(),
            symbol: "TT".to_string(),
            metadata_uri: "https://example.com".to_string(),
            slot: 999,
            signature: "sig".to_string(),
            virtual_sol_reserves: 1_000_000,
        };
        let prompt = build_prompt(&event);
        assert!(prompt.contains("TestMint123"));
        assert!(prompt.contains("TestToken"));
    }
}
