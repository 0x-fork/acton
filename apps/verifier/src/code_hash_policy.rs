use std::collections::BTreeSet;

use thiserror::Error;

use crate::blockchain::{is_valid_code_hash, normalize_code_hash};

/// Code hashes blocked from verification, source/ABI reads, listings, and statistics.
/// Verification status remains available.
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
                let code_hash = normalize_code_hash(entry.trim());
                if !is_valid_code_hash(&code_hash) {
                    return Err(CodeHashPolicyError {
                        entry: entry.clone(),
                    });
                }
                Ok(code_hash)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { blocked })
    }

    #[must_use]
    pub fn is_blocked(&self, code_hash: &str) -> bool {
        self.blocked
            .contains(&normalize_code_hash(code_hash.trim()))
    }

    pub(crate) const fn blocked(&self) -> &BTreeSet<String> {
        &self.blocked
    }
}

#[derive(Debug, Error)]
#[error(
    "invalid server.blocked_code_hashes entry {entry:?}: expected a 256-bit code hash in hexadecimal or base64"
)]
pub struct CodeHashPolicyError {
    pub entry: String,
}
