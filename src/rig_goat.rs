//! AI decision engine – xAI Grok via the Rig framework pattern.
//!
//! Calls the xAI chat completions API with a YES / NO prompt describing the
//! token launch.  Only tokens that receive a **YES** verdict are sniped.
//!
//! Environment variables:
//! - `XAI_API_KEY` (required for live decisions; if absent, defaults to YES).
//! - `GOAT_MODEL`  (optional, default `grok-beta`).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use crate::triton::LaunchEvent;

// ── Constants ─────────────────────────────────────────────────────────────────

/// xAI API chat completions endpoint.
const XAI_API_URL: &str = "https://api.x.ai/v1/chat/completions";

/// Default model – overridable via `GOAT_MODEL` env var.
const DEFAULT_MODEL: &str = "grok-beta";

// ── Engine ────────────────────────────────────────────────────────────────────

/// AI-powered snipe filter backed by xAI Grok.
///
/// Usage matches the Rig framework pattern from CLAUDE.md:
/// ```rust,ignore
/// let client = rig::providers::xai::Client::from_env(); // reads XAI_API_KEY
/// ```
pub struct RigGoat {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl RigGoat {
    /// Construct from environment variables.
    ///
    /// Reads `XAI_API_KEY` (required for live decisions) and optionally
    /// `GOAT_MODEL` to override the default model.
    pub fn from_env() -> Self {
        let api_key = std::env::var("XAI_API_KEY").unwrap_or_default();
        let model =
            std::env::var("GOAT_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }

    /// Return `true` if xAI Grok approves sniping this launch.
    ///
    /// If `XAI_API_KEY` is not configured the gate is disabled and every
    /// launch is approved (useful for testing).  On API errors the bot
    /// defaults to **NO** to avoid buying into unknown conditions.
    #[instrument(skip(self), fields(mint = %event.mint, name = %event.name))]
    pub async fn should_snipe(&self, event: &LaunchEvent) -> bool {
        if self.api_key.is_empty() {
            warn!("XAI_API_KEY not set – AI gate disabled, defaulting to YES");
            return true;
        }

        match self.query_grok(event).await {
            Ok(verdict) => {
                let snipe = verdict.contains("YES");
                debug!(verdict = %verdict, snipe, "AI verdict received");
                snipe
            }
            Err(e) => {
                warn!("AI query failed: {e:#} – defaulting to NO (safe)");
                false
            }
        }
    }

    /// Send the Grok chat completion request and return the uppercased reply.
    async fn query_grok(&self, event: &LaunchEvent) -> Result<String> {
        let prompt = format!(
            "You are a Solana HFT trading bot. Analyse this Pump.fun token launch \
             and decide if it is worth sniping for a quick flip (< 60 s hold).\n\n\
             Mint:    {mint}\n\
             Creator: {creator}\n\
             Name:    {name}\n\
             Symbol:  {symbol}\n\
             URI:     {uri}\n\n\
             Answer with exactly one word: YES or NO.",
            mint = event.mint,
            creator = event.creator,
            name = event.name,
            symbol = event.symbol,
            uri = event.uri,
        );

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: 10,
            temperature: 0.0,
        };

        let response = self
            .client
            .post(XAI_API_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await?;

        let body: ChatResponse = response.json().await?;

        let content = body
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default()
            .to_uppercase();

        Ok(content)
    }
}

// ── JSON types for the xAI API ────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_is_not_empty() {
        assert!(!DEFAULT_MODEL.is_empty());
    }

    #[test]
    fn from_env_uses_default_model_when_unset() {
        std::env::remove_var("GOAT_MODEL");
        let engine = RigGoat::from_env();
        assert_eq!(engine.model, DEFAULT_MODEL);
    }

    #[test]
    fn from_env_respects_goat_model_override() {
        std::env::set_var("GOAT_MODEL", "grok-2-latest");
        let engine = RigGoat::from_env();
        assert_eq!(engine.model, "grok-2-latest");
        std::env::remove_var("GOAT_MODEL");
    }

    #[test]
    fn missing_api_key_means_no_key() {
        std::env::remove_var("XAI_API_KEY");
        let engine = RigGoat::from_env();
        assert!(engine.api_key.is_empty());
    }
}
