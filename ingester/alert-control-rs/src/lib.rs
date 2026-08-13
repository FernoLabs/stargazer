pub mod api;
pub mod store;
pub mod telegram;

use anyhow::{bail, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use stellar_xdr::MuxedAccount;

pub const PAIRING_TTL_SECONDS: i64 = 15 * 60;
pub const PAIRING_TOKEN_BYTES: usize = 32;
pub const PAIRING_TOKEN_LEN: usize = PAIRING_TOKEN_BYTES * 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BillingMode {
    Unconfigured,
    Pilot,
}

impl BillingMode {
    pub fn policy_state(self) -> PolicyState {
        match self {
            Self::Unconfigured => PolicyState::AwaitingEntitlement,
            Self::Pilot => PolicyState::Active,
        }
    }
}

impl fmt::Display for BillingMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unconfigured => formatter.write_str("unconfigured"),
            Self::Pilot => formatter.write_str("pilot"),
        }
    }
}

impl FromStr for BillingMode {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "unconfigured" => Ok(Self::Unconfigured),
            "pilot" => Ok(Self::Pilot),
            _ => bail!("billing mode must be 'unconfigured' or 'pilot'"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyState {
    PendingTelegram,
    AwaitingEntitlement,
    Active,
    Paused,
}

impl PolicyState {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::PendingTelegram => "pending_telegram",
            Self::AwaitingEntitlement => "awaiting_entitlement",
            Self::Active => "active",
            Self::Paused => "paused",
        }
    }

    pub fn from_db(value: &str) -> Result<Self> {
        match value {
            "pending_telegram" => Ok(Self::PendingTelegram),
            "awaiting_entitlement" => Ok(Self::AwaitingEntitlement),
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            _ => bail!("unknown alert policy state"),
        }
    }
}

pub fn normalize_wallet_address(value: &str) -> Result<String> {
    let trimmed = value.trim();
    let account: MuxedAccount = trimmed
        .parse()
        .map_err(|_| anyhow::anyhow!("enter a checksum-valid Stellar G- or M-address"))?;
    match account {
        MuxedAccount::Ed25519(key) => Ok(MuxedAccount::Ed25519(key).to_string()),
        MuxedAccount::MuxedEd25519(muxed) => Ok(MuxedAccount::Ed25519(muxed.ed25519).to_string()),
    }
}

pub fn validate_pairing_token(token: &str) -> Result<()> {
    if token.len() != PAIRING_TOKEN_LEN
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("invalid pairing token");
    }
    Ok(())
}

pub fn pairing_token_hash(token: &str) -> Result<[u8; 32]> {
    validate_pairing_token(token)?;
    Ok(Sha256::digest(token.as_bytes()).into())
}

pub fn validate_bot_username(value: &str) -> Result<String> {
    let username = value.trim().trim_start_matches('@');
    if !(5..=32).contains(&username.len())
        || !username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || !username.to_ascii_lowercase().ends_with("bot")
    {
        bail!(
            "Telegram bot username must be 5-32 ASCII letters, digits, or underscores and end in 'bot'"
        );
    }
    Ok(username.to_owned())
}

pub fn unix_timestamp() -> Result<i64> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| anyhow::anyhow!("system clock is before Unix epoch"))?
        .as_secs();
    i64::try_from(seconds).map_err(|_| anyhow::anyhow!("Unix timestamp exceeds i64"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::{MuxedAccountMed25519, Uint256};

    #[test]
    fn wallet_validation_normalizes_muxed_accounts() {
        let key = Uint256([17; 32]);
        let base = MuxedAccount::Ed25519(key.clone()).to_string();
        let muxed = MuxedAccount::MuxedEd25519(MuxedAccountMed25519 {
            id: 42,
            ed25519: key,
        })
        .to_string();

        assert_eq!(normalize_wallet_address(&base).unwrap(), base);
        assert_eq!(normalize_wallet_address(&muxed).unwrap(), base);
        assert!(normalize_wallet_address("G-not-valid").is_err());
    }

    #[test]
    fn pairing_tokens_are_fixed_lowercase_hex() {
        let valid = "ab".repeat(PAIRING_TOKEN_BYTES);
        assert!(validate_pairing_token(&valid).is_ok());
        assert_eq!(pairing_token_hash(&valid).unwrap().len(), 32);
        assert!(validate_pairing_token(&"A0".repeat(PAIRING_TOKEN_BYTES)).is_err());
        assert!(validate_pairing_token(&"_-".repeat(PAIRING_TOKEN_BYTES)).is_err());
        assert!(validate_pairing_token(&"a".repeat(PAIRING_TOKEN_LEN - 1)).is_err());
    }

    #[test]
    fn billing_mode_fails_closed() {
        assert_eq!(
            "unconfigured".parse::<BillingMode>().unwrap(),
            BillingMode::Unconfigured
        );
        assert_eq!("pilot".parse::<BillingMode>().unwrap(), BillingMode::Pilot);
        assert!("paid".parse::<BillingMode>().is_err());
    }
}
