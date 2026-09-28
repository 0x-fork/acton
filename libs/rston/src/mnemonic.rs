//! TON mnemonic validation and Ed25519 key derivation.

use crate::error::MnemonicError;
use ed25519_dalek::{KEYPAIR_LENGTH, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, SecretKey, SigningKey};
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha512;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::{cmp, convert::TryInto, fmt};
use zeroize::{Zeroize, Zeroizing};

const WORDLIST_EN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/mnemonics/wordlist_en.txt"
));
const PBKDF_ITERATIONS: u32 = 100000;

/// English words accepted by TON mnemonic validation, shared across all phrases.
pub static WORDLIST_EN_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| WORDLIST_EN.split('\n').filter(|w| !w.is_empty()).collect());

/// An owned TON mnemonic whose words and optional password are zeroized on drop.
///
/// Borrowed input passed to [`Mnemonic::new`] or [`Mnemonic::from_str`] remains
/// owned by the caller and cannot be cleared by this type.
pub struct Mnemonic {
    words: Zeroizing<Vec<String>>,
    password: Zeroizing<Option<String>>,
}

/// An Ed25519 key pair whose secret key bytes are zeroized on drop.
///
/// Copies made by reading the public [`KeyPair::secret_key`] field are owned by
/// the caller and cannot be cleared by this type.
#[derive(PartialEq, Eq, Clone, Hash)]
pub struct KeyPair {
    /// Ed25519 public key bytes.
    pub public_key: [u8; PUBLIC_KEY_LENGTH],
    /// Ed25519 key-pair bytes, including the private signing key.
    pub secret_key: [u8; KEYPAIR_LENGTH],
}

impl Drop for KeyPair {
    fn drop(&mut self) {
        self.secret_key.zeroize()
    }
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("public_key", &self.public_key)
            .field("secret_key", &"***REDACTED***")
            .finish()
    }
}

impl Mnemonic {
    /// Imports an existing TON mnemonic from 24 English words.
    ///
    /// Trims each word and converts it to lowercase before validation.
    ///
    /// `password` is the mnemonic password used for key derivation.
    /// `None` and an empty string both mean no password. Nonempty passwords
    /// retain their original whitespace and case.
    ///
    /// # Errors
    ///
    /// Returns [`MnemonicError`] for an incorrect word count, an unknown word,
    /// or a phrase that fails TON seed validation.
    pub fn new(words: Vec<&str>, password: Option<String>) -> Result<Mnemonic, MnemonicError> {
        let password = Zeroizing::new(password);

        if words.len() != 24 {
            return Err(MnemonicError::WordCount(words.len()));
        }

        let normalized_words = Zeroizing::new(
            words
                .into_iter()
                .map(|word| word.trim().to_lowercase())
                .collect::<Vec<_>>(),
        );

        if let Some(word) = normalized_words
            .iter()
            .find(|word| !WORDLIST_EN_SET.contains(word.as_str()))
        {
            return Err(MnemonicError::UnknownWord(word.clone()));
        }

        match &*password {
            Some(s) if !s.is_empty() => {
                let passless_entropy = to_entropy(&normalized_words, None)?;
                let seed = pbkdf2_sha512(passless_entropy, "TON fast seed version", 1, 64)?;
                if seed[0] != 1 {
                    return Err(MnemonicError::InvalidPasswordSeed(seed[0]));
                }
                // Make that this also is not a valid passwordless mnemonic
                let entropy = to_entropy(&normalized_words, (*password).as_ref())?;
                let seed = pbkdf2_sha512(
                    entropy,
                    "TON seed version",
                    cmp::max(1, PBKDF_ITERATIONS / 256),
                    64,
                )?;
                if seed[0] == 0 {
                    return Err(MnemonicError::InvalidPasswordSeed(seed[0]));
                }
            }
            _ => {
                let entropy = to_entropy(&normalized_words, None)?;
                let seed = pbkdf2_sha512(
                    entropy,
                    "TON seed version",
                    cmp::max(1, PBKDF_ITERATIONS / 256),
                    64,
                )?;
                if seed[0] != 0 {
                    return Err(MnemonicError::InvalidPasswordlessSeed(seed[0]));
                }
            }
        }

        Ok(Mnemonic {
            words: normalized_words,
            password,
        })
    }

    /// Imports an existing TON mnemonic from a space-separated phrase.
    ///
    /// Accepts repeated spaces and whitespace around each word.
    /// Tabs and line breaks do not separate words.
    /// Uses [`Self::new`] for word normalization, password handling, and validation.
    pub fn from_str(s: &str, password: Option<String>) -> Result<Mnemonic, MnemonicError> {
        let words: Vec<&str> = s
            .split(' ')
            .map(|w| w.trim())
            .filter(|w| !w.is_empty())
            .collect();
        Mnemonic::new(words, password)
    }

    /// Derives the Ed25519 key pair using the TON default seed parameters.
    /// The returned pair owns its secret bytes and clears them on drop.
    pub fn to_key_pair(&self) -> Result<KeyPair, MnemonicError> {
        let entropy = to_entropy(&self.words, (*self.password).as_ref())?;
        let seed = pbkdf2_sha512(entropy, "TON default seed", PBKDF_ITERATIONS, 64)?;

        let secret_key_bytes: &SecretKey = seed
            .get(..SECRET_KEY_LENGTH)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| MnemonicError::InvalidSecretKeyLength {
                actual: seed.len(),
                expected: SECRET_KEY_LENGTH,
            })?;

        let signing_key = SigningKey::from_bytes(secret_key_bytes);
        Ok(KeyPair {
            public_key: signing_key.verifying_key().to_bytes(),
            secret_key: signing_key.to_keypair_bytes(),
        })
    }
}

fn to_entropy(
    words: &[String],
    password: Option<&String>,
) -> Result<Zeroizing<Vec<u8>>, MnemonicError> {
    let phrase_len = words.iter().map(String::len).sum::<usize>() + words.len().saturating_sub(1);
    let mut phrase = Zeroizing::new(String::with_capacity(phrase_len));
    for (index, word) in words.iter().enumerate() {
        if index > 0 {
            phrase.push(' ');
        }
        phrase.push_str(word);
    }
    let mut mac = Hmac::<Sha512>::new_from_slice(phrase.as_bytes())?;
    if let Some(s) = password {
        mac.update(s.as_bytes());
    }
    let mut code_bytes = mac.finalize().into_bytes();
    let entropy = Zeroizing::new(code_bytes.to_vec());
    code_bytes.zeroize();
    Ok(entropy)
}

fn pbkdf2_sha512(
    key: Zeroizing<Vec<u8>>,
    salt: &str,
    rounds: u32,
    output_len_bytes: usize,
) -> Result<Zeroizing<Vec<u8>>, MnemonicError> {
    let mut output = Zeroizing::new(vec![0; output_len_bytes]);
    pbkdf2_hmac::<Sha512>(
        key.as_slice(),
        salt.as_bytes(),
        rounds,
        output.as_mut_slice(),
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonic_parse_works() -> anyhow::Result<()> {
        let words = "dose ice enrich trigger test dove century still betray gas diet dune use other base gym mad law immense village world example praise game";
        let mnemonic = Mnemonic::from_str(words, None);
        assert!(mnemonic.is_ok());

        let words = " dose ice enrich trigger test dove \
        century still betray gas diet       dune use other base gym mad law \
        immense village world example praise game ";
        let mnemonic = Mnemonic::from_str(words, None);
        assert!(mnemonic.is_ok());
        Ok(())
    }

    #[test]
    fn mnemonic_validate_works() -> anyhow::Result<()> {
        let mnemonic = Mnemonic::new(
            vec![
                "dose", "ice", "enrich", "trigger", "test", "dove", "century", "still", "betray",
                "gas", "diet", "dune",
            ],
            None,
        );
        assert!(mnemonic.is_err());
        let mnemonic = Mnemonic::new(vec!["a"], None);
        assert!(mnemonic.is_err());
        Ok(())
    }

    #[test]
    fn mnemonic_to_private_key_works() -> anyhow::Result<()> {
        let mnemonic = Mnemonic::new(
            vec![
                "dose", "ice", "enrich", "trigger", "test", "dove", "century", "still", "betray",
                "gas", "diet", "dune", "use", "other", "base", "gym", "mad", "law", "immense",
                "village", "world", "example", "praise", "game",
            ],
            None,
        )?;
        let expected = "119dcf2840a3d56521d260b2f125eedc0d4f3795b9e627269a4b5a6dca8257bdc04ad1885c127fe863abb00752fa844e6439bb04f264d70de7cea580b32637ab";

        let kp = mnemonic.to_key_pair()?;
        let expected = Zeroizing::new(hex::decode(expected)?);
        assert_eq!(kp.secret_key.as_slice(), expected.as_slice());

        Ok(())
    }
}
