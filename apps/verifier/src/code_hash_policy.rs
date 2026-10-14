use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::blockchain::parse_code_hash;

/// Code hashes blocked from verification and direct source/ABI reads.
/// Verification status, listings, and statistics reflect the source repository.
#[derive(Clone, Debug, Default)]
pub struct CodeHashPolicy {
    blocked: BTreeSet<String>,
}

impl CodeHashPolicy {
    /// Loads one hexadecimal or base64 code hash per line, ignoring blank lines
    /// and lines starting with `#` after trimming whitespace.
    /// Hashes are normalized and deduplicated. An empty file blocks no hashes.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or any entry is not a 256-bit code hash.
    pub fn from_file(path: &Path) -> Result<Self, CodeHashPolicyError> {
        let read_error = |source| CodeHashPolicyError::Read {
            path: path.to_path_buf(),
            source,
        };
        let file = File::open(path).map_err(read_error)?;
        let mut blocked = BTreeSet::new();
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(read_error)?;
            let entry = line.trim();
            if entry.is_empty() || entry.starts_with('#') {
                continue;
            }
            let hash = parse_code_hash(entry).ok_or_else(|| CodeHashPolicyError::InvalidEntry {
                path: path.to_path_buf(),
                line: index + 1,
                entry: entry.to_owned(),
            })?;
            blocked.insert(hash);
        }
        Ok(Self { blocked })
    }

    /// Returns the number of unique blocked code hashes.
    #[must_use]
    pub(crate) fn blocked_count(&self) -> usize {
        self.blocked.len()
    }

    #[must_use]
    pub fn is_blocked(&self, code_hash: &str) -> bool {
        parse_code_hash(code_hash).is_some_and(|hash| self.blocked.contains(&hash))
    }
}

#[derive(Debug, Error)]
pub enum CodeHashPolicyError {
    #[error("failed to read blocked code hash file {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error(
        "invalid blocked code hash {entry:?} at {path}:{line}: expected a 256-bit code hash in hexadecimal or base64"
    )]
    InvalidEntry {
        path: PathBuf,
        line: usize,
        entry: String,
    },
}
