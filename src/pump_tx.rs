//! Pump.fun v2 transaction builder.
//!
//! February 2026 update requirements (CLAUDE.md):
//! - New PDAs: `bonding-curve-v2` and `pool-v2` – **always appended at the END**
//!   of the account list.
//! - Creator Rewards Sharing enabled.
//! - Mayhem Mode: 8 fee-recipient addresses (strict order required).

use anyhow::{Context, Result};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};
use tracing::{debug, instrument};

use crate::triton::LaunchEvent;

// ── Well-known program IDs ────────────────────────────────────────────────────

/// Pump.fun AMM program (mainnet).
pub const PUMP_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P");

/// SPL Associated Token Account program.
pub const SPL_ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// SPL Token program.
pub const SPL_TOKEN_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

// ── Trading parameters ────────────────────────────────────────────────────────

/// Default max spend per trade: 0.1 SOL in lamports.
pub const DEFAULT_MAX_SOL_LAMPORTS: u64 = 100_000_000;

/// Default slippage tolerance: 500 bps = 5 %.
pub const DEFAULT_SLIPPAGE_BPS: u64 = 500;

/// 8-byte discriminator for the Pump.fun v2 `buy` instruction.
const BUY_DISCRIMINATOR: [u8; 8] = [0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea];

// ── Types ─────────────────────────────────────────────────────────────────────

/// A signed, serialised Solana transaction ready for Jito bundle submission.
pub struct PumpTransaction {
    /// Raw bincode-serialised transaction bytes.
    pub serialized: Vec<u8>,
    /// The token mint this buy targets.
    pub mint: Pubkey,
}

impl PumpTransaction {
    /// Build a Pump.fun v2 `buy` instruction for the given launch event.
    ///
    /// The v2 PDAs (`bonding-curve-v2` and `pool-v2`) are derived and
    /// **appended to the end** of the account list as required by the
    /// February 2026 Pump.fun update.
    #[instrument(skip(wallet_key), fields(mint = %event.mint))]
    pub fn build_buy_v2(event: &LaunchEvent, wallet_key: &str) -> Result<Self> {
        let payer = parse_keypair(wallet_key).context("Invalid WALLET_PRIVATE_KEY")?;

        let max_sol = std::env::var("MAX_SOL_PER_TRADE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok().map(|s| (s * 1e9) as u64))
            .unwrap_or(DEFAULT_MAX_SOL_LAMPORTS);

        let slippage_bps: u64 = std::env::var("SLIPPAGE_BPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_SLIPPAGE_BPS);

        // ── Derive PDAs ───────────────────────────────────────────────────────

        // v1 PDAs (still required)
        let (global, _) = Pubkey::find_program_address(&[b"global"], &PUMP_PROGRAM_ID);
        let (bonding_curve, _) = Pubkey::find_program_address(
            &[b"bonding-curve", event.mint.as_ref()],
            &PUMP_PROGRAM_ID,
        );

        // Feb 2026 v2 PDAs – APPENDED AT END of account list (CLAUDE.md)
        let (bonding_curve_v2, _) = Pubkey::find_program_address(
            &[b"bonding-curve-v2", event.mint.as_ref()],
            &PUMP_PROGRAM_ID,
        );
        let (pool_v2, _) =
            Pubkey::find_program_address(&[b"pool-v2", event.mint.as_ref()], &PUMP_PROGRAM_ID);

        // Associated token account for the payer
        let (ata, _) = Pubkey::find_program_address(
            &[
                payer.pubkey().as_ref(),
                SPL_TOKEN_PROGRAM_ID.as_ref(),
                event.mint.as_ref(),
            ],
            &SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        );

        // ── Instruction data ──────────────────────────────────────────────────

        // token_amount: placeholder; production derives from bonding curve state.
        let token_amount: u64 = 1_000_000;
        let max_sol_cost = max_sol.saturating_add(max_sol.saturating_mul(slippage_bps) / 10_000);

        let mut data = Vec::with_capacity(24);
        data.extend_from_slice(&BUY_DISCRIMINATOR);
        data.extend_from_slice(&token_amount.to_le_bytes());
        data.extend_from_slice(&max_sol_cost.to_le_bytes());

        // ── Account list ──────────────────────────────────────────────────────
        // v1 accounts first, v2 PDAs MUST be at the END (CLAUDE.md).
        let accounts = vec![
            AccountMeta::new_readonly(global, false),
            AccountMeta::new(event.mint, false),
            AccountMeta::new(bonding_curve, false),
            AccountMeta::new(ata, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SPL_ASSOCIATED_TOKEN_PROGRAM_ID, false),
            // ── v2 PDAs (Feb 2026) – MUST be at the END ──────────────────────
            AccountMeta::new(bonding_curve_v2, false),
            AccountMeta::new(pool_v2, false),
        ];

        let ix = Instruction {
            program_id: PUMP_PROGRAM_ID,
            accounts,
            data,
        };

        // Use a placeholder blockhash; production fetches the latest from RPC.
        let recent_blockhash = solana_sdk::hash::Hash::default();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );

        let serialized = bincode::serialize(&tx).context("Failed to serialise transaction")?;

        debug!(
            mint = %event.mint,
            size = serialized.len(),
            "Built Pump.fun v2 buy transaction"
        );

        Ok(Self {
            serialized,
            mint: event.mint,
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Decode a base-58 wallet private key into a [`Keypair`].
fn parse_keypair(base58_key: &str) -> Result<Keypair> {
    let bytes = bs58::decode(base58_key)
        .into_vec()
        .context("Base58 decode failed")?;
    Keypair::from_bytes(&bytes).context("Invalid keypair bytes")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pump_program_id_is_correct() {
        let expected: Pubkey = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
            .parse()
            .unwrap();
        assert_eq!(PUMP_PROGRAM_ID, expected);
    }

    #[test]
    fn bonding_curve_v2_pda_is_off_curve() {
        let mint = Pubkey::new_unique();
        let (pda, _) =
            Pubkey::find_program_address(&[b"bonding-curve-v2", mint.as_ref()], &PUMP_PROGRAM_ID);
        // PDAs are always off the Ed25519 curve by definition.
        assert!(!pda.is_on_curve());
    }

    #[test]
    fn pool_v2_pda_is_off_curve() {
        let base_mint = Pubkey::new_unique();
        let (pda, _) =
            Pubkey::find_program_address(&[b"pool-v2", base_mint.as_ref()], &PUMP_PROGRAM_ID);
        assert!(!pda.is_on_curve());
    }

    #[test]
    fn default_slippage_is_500_bps() {
        assert_eq!(DEFAULT_SLIPPAGE_BPS, 500);
    }

    #[test]
    fn default_max_sol_is_01_sol() {
        // 0.1 SOL = 100_000_000 lamports
        assert_eq!(DEFAULT_MAX_SOL_LAMPORTS, 100_000_000);
    }

    #[test]
    fn buy_discriminator_length() {
        assert_eq!(BUY_DISCRIMINATOR.len(), 8);
    }
}
