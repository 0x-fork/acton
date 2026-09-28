use rston::Wallet as SigningWallet;
use rston::models::{Base64StdAddrFlags, DisplayBase64StdAddr};
use rston::wallet::{Mnemonic, WALLET_V5R1_ID_DEFAULT_TESTNET, WalletVersion};

#[derive(Clone, Debug)]
pub struct Wallet {
    pub wallet: SigningWallet,
}

impl Wallet {
    pub fn new(mnemonic_str: &str) -> anyhow::Result<Self> {
        let mnemonic = Mnemonic::from_str(mnemonic_str, None)?;

        let wallet = SigningWallet::new_with_params(
            WalletVersion::V5R1,
            mnemonic.to_key_pair()?,
            0,
            WALLET_V5R1_ID_DEFAULT_TESTNET,
        )?;

        Ok(Self { wallet })
    }

    pub fn get_address(&self) -> String {
        DisplayBase64StdAddr {
            addr: &self.wallet.address,
            flags: Base64StdAddrFlags {
                testnet: true,
                bounceable: true,
                base64_url: true,
            },
        }
        .to_string()
    }
}
