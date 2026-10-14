use std::collections::BTreeSet;

use thiserror::Error;

use crate::blockchain::parse_code_hash;

/// Code hashes blocked from verification and direct source/ABI reads.
/// Verification status, listings, and statistics reflect the source repository.
#[derive(Clone, Debug, Default)]
pub struct CodeHashPolicy {
    blocked: BTreeSet<String>,
}

impl CodeHashPolicy {
    /// Parses hexadecimal or base64 code hashes, ignoring case in hexadecimal hashes.
    ///
    /// # Errors
    ///
    /// Returns an error if any entry is not a 256-bit code hash.
    pub fn from_blocked(entries: &[String]) -> Result<Self, CodeHashPolicyError> {
        let blocked = entries
            .iter()
            .map(|entry| {
                parse_code_hash(entry).ok_or_else(|| CodeHashPolicyError {
                    entry: entry.clone(),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { blocked })
    }

    #[must_use]
    pub fn is_blocked(&self, code_hash: &str) -> bool {
        parse_code_hash(code_hash).is_some_and(|hash| self.blocked.contains(&hash))
    }
}

#[derive(Debug, Error)]
#[error(
    "invalid server.blocked_code_hashes entry {entry:?}: expected a 256-bit code hash in hexadecimal or base64"
)]
pub struct CodeHashPolicyError {
    pub entry: String,
}
