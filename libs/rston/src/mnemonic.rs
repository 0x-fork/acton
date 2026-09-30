//! TON and BIP39 mnemonic validation and Ed25519 wallet key derivation.

use crate::error::MnemonicError;
use ed25519_dalek::{KEYPAIR_LENGTH, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH, SecretKey, SigningKey};
use hmac::{Hmac, Mac};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha512;
use std::borrow::Cow;
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

/// An owned wallet mnemonic whose words and optional password are zeroized on drop.
///
/// Imports 24-word TON phrases or 12-word BIP39 phrases. BIP39 wallet keys use
/// SLIP-0010 Ed25519 derivation at `m/44'/607'/0'`, as described in the
/// [TON wallet guidelines](https://github.com/ton-blockchain/TEPs/blob/master/text/0003-wallets.md).
///
/// Borrowed input passed to [`Mnemonic::new`] or [`Mnemonic::from_str`] remains
/// owned by the caller and cannot be cleared by this type.
pub struct Mnemonic {
    words: MnemonicWords,
    password: Zeroizing<Option<String>>,
}

enum MnemonicWords {
    Ton(Zeroizing<Vec<String>>),
    Bip39(bip39::Mnemonic),
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
    /// Imports 24 English words as a TON mnemonic or 12 as a BIP39 mnemonic.
    ///
    /// Trims each word and converts it to lowercase before validation.
    ///
    /// `password` is the mnemonic password used for key derivation.
    /// `None` and an empty string both mean no password. Nonempty passwords
    /// retain their whitespace and case. BIP39 passphrases are normalized to
    /// Unicode NFKD during key derivation and do not affect checksum validation.
    ///
    /// # Errors
    ///
    /// Returns [`MnemonicError`] for an incorrect word count, an unknown word,
    /// or a phrase that fails TON seed validation or the BIP39 checksum.
    pub fn new(words: Vec<&str>, password: Option<String>) -> Result<Mnemonic, MnemonicError> {
        let password = Zeroizing::new(password);

        if !matches!(words.len(), 12 | 24) {
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

        if normalized_words.len() == 12 {
            let phrase = Zeroizing::new(normalized_words.join(" "));
            let words = bip39::Mnemonic::parse_in_normalized(bip39::Language::English, &phrase)?;
            return Ok(Mnemonic {
                words: MnemonicWords::Bip39(words),
                password,
            });
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
            words: MnemonicWords::Ton(normalized_words),
            password,
        })
    }

    /// Imports a 12-word BIP39 or 24-word TON mnemonic from a space-separated phrase.
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

    /// Derives the Ed25519 wallet key pair for this phrase's scheme.
    ///
    /// TON phrases use the TON default seed parameters. BIP39 phrases use
    /// SLIP-0010 at `m/44'/607'/0'` (the first TON account).
    /// The returned pair owns its secret bytes and clears them on drop.
    pub fn to_key_pair(&self) -> Result<KeyPair, MnemonicError> {
        let seed = match &self.words {
            MnemonicWords::Ton(words) => {
                let entropy = to_entropy(words, (*self.password).as_ref())?;
                pbkdf2_sha512(entropy, "TON default seed", PBKDF_ITERATIONS, 64)?
            }
            MnemonicWords::Bip39(words) => {
                bip39_wallet_seed(words, self.password.as_deref().unwrap_or_default())?
            }
        };

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

/// Derives the first TON account using hardened SLIP-0010 Ed25519 children.
fn bip39_wallet_seed(
    mnemonic: &bip39::Mnemonic,
    password: &str,
) -> Result<Zeroizing<Vec<u8>>, MnemonicError> {
    let mut passphrase = Cow::Borrowed(password);
    bip39::Mnemonic::normalize_utf8_cow(&mut passphrase);
    let passphrase = Zeroizing::new(passphrase.into_owned());
    let seed = Zeroizing::new(mnemonic.to_seed_normalized(&passphrase));

    let mut mac = Hmac::<Sha512>::new_from_slice(b"ed25519 seed")?;
    mac.update(seed.as_slice());
    let mut derived = Zeroizing::new(<[u8; 64]>::from(mac.finalize().into_bytes()));

    // m/44'/607'/0': BIP44 purpose, TON coin type, first account.
    // Each result holds the private key followed by its chain code.
    for index in [44u32, 607, 0] {
        let mut mac = Hmac::<Sha512>::new_from_slice(&derived[32..])?;
        mac.update(&[0]);
        mac.update(&derived[..32]);
        mac.update(&(index | (1 << 31)).to_be_bytes());
        derived = Zeroizing::new(<[u8; 64]>::from(mac.finalize().into_bytes()));
    }

    Ok(Zeroizing::new(derived.to_vec()))
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

    #[test]
    fn bip39_wallet_keys_match_reference() -> anyhow::Result<()> {
        // BIP39 test phrases, with keys derived by @ton/crypto's
        // deriveEd25519Path(seed, [44, 607, 0]) and keyPairFromSeed.
        let cases = [
            (
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                None,
                "b477ef5ed17fb8a2b8faddd7a9835a227243a82c70b190c7af4896155aa7df9f7952e94118f34607c75e23258dd9220d66ccac5a3ee074125c25068e8107bfbf",
            ),
            (
                "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                Some("TREZOR"),
                "d0f0c5522593343e960933db00c1df873c0c1f87771c853c2bf0ef23b2f97b413a2da797ef192099b8d918e55e5e2c54ea228637fdec66d77264ac22e447db4a",
            ),
            (
                "legal winner thank year wave sausage worth useful legal winner thank yellow",
                None,
                "930b30bc355699d639e8b9561c5e650edfc696294f9917cc19a453dcd526cf84434908c4c092113fcced4eec061b09c02b7b8934a9ca555e2032505aab7558ea",
            ),
        ];

        for (phrase, password, expected) in cases {
            let mnemonic = Mnemonic::from_str(phrase, password.map(str::to_owned))?;
            let key_pair = mnemonic.to_key_pair()?;
            assert_eq!(hex::encode(key_pair.secret_key), expected);
            assert_eq!(key_pair.public_key, key_pair.secret_key[32..]);
        }

        Ok(())
    }

    #[test]
    fn bip39_normalizes_words_and_passphrase() -> anyhow::Result<()> {
        let phrase = "  ABANDON abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon  ABOUT ";
        let without_password = Mnemonic::from_str(phrase, None)?.to_key_pair()?;
        let empty_password = Mnemonic::from_str(phrase, Some(String::new()))?.to_key_pair()?;
        assert_eq!(without_password, empty_password);

        for password in ["caf\u{e9}", "cafe\u{301}"] {
            let key_pair = Mnemonic::from_str(phrase, Some(password.to_owned()))?.to_key_pair()?;
            assert_eq!(
                hex::encode(key_pair.public_key),
                "001a3e145602093c24330c1515319b9cf68ed74a3412310008fd6407fc025f0c",
            );
        }

        Ok(())
    }

    #[test]
    fn bip39_validates_checksum() {
        let result = Mnemonic::new(vec!["abandon"; 12], None);
        assert!(matches!(
            result,
            Err(MnemonicError::InvalidBip39(bip39::Error::InvalidChecksum)),
        ));
    }
}
