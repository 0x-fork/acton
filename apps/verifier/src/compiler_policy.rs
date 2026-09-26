use thiserror::Error;

/// Parsed compiler restrictions from `compiler.disabled`.
///
/// An empty policy disables nothing. Rules match either every version of a
/// compiler or one exact version, independently of the installed compilers.
#[derive(Clone, Debug, Default)]
pub struct CompilerPolicy {
    disabled: Vec<DisabledCompiler>,
}

impl CompilerPolicy {
    /// Parses compiler names and exact version rules.
    ///
    /// # Errors
    ///
    /// Returns an error identifying the first malformed `name` or `name@version` rule.
    pub fn from_disabled(entries: &[String]) -> Result<Self, CompilerPolicyError> {
        let disabled = entries
            .iter()
            .map(|entry| {
                DisabledCompiler::parse(entry).map_err(|reason| CompilerPolicyError {
                    entry: entry.clone(),
                    reason,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { disabled })
    }

    /// Returns whether any configured rule disables the given compiler version.
    ///
    /// Compiler names are trimmed and matched without ASCII case sensitivity.
    /// Versions are matched verbatim, including prerelease and build suffixes.
    /// This only checks the policy; it does not check compiler availability.
    #[must_use]
    pub fn is_disabled(&self, name: &str, version: &str) -> bool {
        let name = name.trim();
        self.disabled.iter().any(|rule| {
            rule.name.eq_ignore_ascii_case(name)
                && rule.version.as_deref().is_none_or(|exact| exact == version)
        })
    }
}

#[derive(Debug, Error)]
#[error("invalid compiler.disabled entry {entry:?}: {reason}")]
pub struct CompilerPolicyError {
    pub entry: String,
    pub reason: &'static str,
}

#[derive(Clone, Debug)]
struct DisabledCompiler {
    name: String,
    version: Option<String>,
}

impl DisabledCompiler {
    fn parse(entry: &str) -> Result<Self, &'static str> {
        let (name, version) = entry
            .split_once('@')
            .map_or((entry, None), |(name, version)| {
                (name, Some(version.trim()))
            });
        let name = name.trim();
        if name.is_empty() {
            return Err("expected a non-empty compiler name");
        }
        if version.is_some_and(|version| version.is_empty() || version.contains('@')) {
            return Err("expected name or name@version with a non-empty version");
        }
        Ok(Self {
            name: name.to_owned(),
            version: version.map(str::to_owned),
        })
    }
}
