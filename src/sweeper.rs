use crate::input_eval::PrivateKeys;
use bdk::{
    bitcoin::{Address, Network},
    blockchain::EsploraBlockchain,
    database::MemoryDatabase,
    SignOptions, SyncOptions, Wallet,
};

pub struct Sweeper {
    pub esplora_url: String,
    pub network: Network,
}

impl Sweeper {
    pub async fn sweep(
        &self,
        privkeys: &PrivateKeys,
        destination: &Address,
    ) -> Result<String, String> {
        let descriptors = Self::descriptors(privkeys)?;

        // note: I tried to use tokio JoinSet here to make it cocurrent, but bdk::wallet is not suitable to pass between threads.
        let mut res = vec![];
        for desc in descriptors {
            res.push(self.sweep_one(&desc, destination).await?);
        }
        let msg = res
            .iter()
            .flatten()
            .fold("".to_string(), |acc, msg| acc + "\n" + &msg)
            .trim()
            .to_string();
        if !msg.is_empty() {
            Ok(msg)
        } else {
            Ok("No balances found to sweep".to_string())
        }
    }

    async fn sweep_one(&self, desc: &str, destination: &Address) -> Result<Option<String>, String> {
        let wallet = Wallet::new(desc, None, self.network, MemoryDatabase::default())
            .map_err(|e| format!("Failed to construct sweep wallet: {}", e))?;
        let blockchain = EsploraBlockchain::new(&self.esplora_url, 20);
        wallet
            .sync(&blockchain, SyncOptions::default())
            .await
            .map_err(|e| format!("Failed to sync sweep wallet: {}", e))?;

        if let Ok(bal) = wallet.get_balance() {
            if bal.get_total() <= 0 {
                return Ok(None);
            }
            println!("sweeping {} to {}", bal, destination.to_string());
            let mut builder = wallet.build_tx();
            builder
                .drain_wallet()
                .drain_to(destination.script_pubkey())
                .enable_rbf();
            let (mut psbt, _) = builder
                .finish()
                .map_err(|e| format!("Failed to construct sweep transaction: {}", e))?;
            let signopt = SignOptions {
                ..Default::default()
            };
            wallet
                .sign(&mut psbt, signopt)
                .map_err(|e| format!("Failed to sign sweep transaction: {}", e))?;
            let tx = psbt.extract_tx();
            blockchain
                .broadcast(&tx)
                .await
                .map_err(|e| format!("Failed to broadcast sweep transaction: {}", e))?;
            Ok(Some(format!("swept {}", bal.get_total())))
        } else {
            Ok(None)
        }
    }

    fn descriptors(privkeys: &PrivateKeys) -> Result<Vec<String>, String> {
        match privkeys {
            PrivateKeys::Desc(desc) => Ok(vec![desc.to_string()]),
            PrivateKeys::Pk(_) | PrivateKeys::Epk(_) => {
                let pref_postf = [
                    ("pkh(", ")"),
                    ("wpkh(", ")"),
                    ("wsh(pk(", "))"),
                    ("sh(wsh(pk(", ")))"),
                ];
                Ok(pref_postf
                    .iter()
                    .map(|(pref, postf)| pref.to_string() + &privkeys.to_string() + postf)
                    .collect())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bdk::wallet::AddressIndex::New;
    use ldk_node::bitcoin::{util::bip32::ExtendedPrivKey, PrivateKey};
    use miniscript::Descriptor;
    use rstest::rstest;
    use std::str::FromStr;

    fn parse_priv(inp: &str) -> PrivateKeys {
        if let Ok(pk) = PrivateKey::from_wif(inp) {
            return PrivateKeys::Pk(pk);
        }

        let xprv = ExtendedPrivKey::from_str(inp).unwrap();
        PrivateKeys::Epk(xprv)
    }

    #[rstest]
    #[case::wif("KxWvpvpY9C5weJGWpUMQqHt88Xktt7nZDZPHbpJjEuUaDgeMHJuw", [
            "174fgNxhD2sPLaY9BjFtLp9Tnf24HESSkh",
            "bc1qg2py53k2rfheluwvqlqhp4867lp3e2kw2jqqmr",
            "bc1qyxyje8qt473cx0tnp8ed2stc2cu5fw8v84m225kphqe5yc8ve46qhnqdzx",
            "3Dtf6RhgusYjRDQyDG5GoUivD4U6aSDRkY"])]
    #[case::xprv("xprv9z1Nt86QQeoGXTjrvKgbFT924JeV1qmo2QV6m8YYTWkaVVWNc3nmeTTKsoq2PKVMfQLUKchQbazkT5FqLo4BUC2P2rVFmDnE46QBNjiAsLP", [
            "182vUeQLsdKqkPt5CWsV7Jz3MRUS6vhXgN",
            "bc1qf5j7l03de8gy6zlf926rms38520h9ngpns40t9",
            "bc1qy8mzjpjnapcsy9fk33jexexk0l46ptz4vmst2p88ly0sxgg4656svv0gvm",
            "32ymS1kXfkd9TNw8a2fKubWBYcyW28LXD8"])]
    fn test_sweep_pk(#[case] pk: &str, #[case] addrs: [&str; 4]) {
        let pk = parse_priv(pk);
        let desc = Sweeper::descriptors(&pk).unwrap();
        assert_eq!(desc.len(), 4);
        let w1 = Wallet::new(&desc[0], None, Network::Bitcoin, MemoryDatabase::default())
            .map_err(|e| format!("{} - {}", desc[0], e))
            .unwrap();
        assert_eq!(w1.get_address(New).unwrap().to_string(), addrs[0]);
        let w2 = Wallet::new(&desc[1], None, Network::Bitcoin, MemoryDatabase::default())
            .map_err(|e| format!("{} - {}", desc[1], e))
            .unwrap();
        assert_eq!(w2.get_address(New).unwrap().to_string(), addrs[1]);
        let w3 = Wallet::new(&desc[2], None, Network::Bitcoin, MemoryDatabase::default())
            .map_err(|e| format!("{} - {}", desc[2], e))
            .unwrap();
        assert_eq!(w3.get_address(New).unwrap().to_string(), addrs[2]);
        let w4 = Wallet::new(&desc[3], None, Network::Bitcoin, MemoryDatabase::default())
            .map_err(|e| format!("{} - {}", desc[3], e))
            .unwrap();
        assert_eq!(w4.get_address(New).unwrap().to_string(), addrs[3]);
    }

    #[test]
    fn test_sweep_desc() {
        let inp = "pkh(xprv9z1Nt86QQeoGXTjrvKgbFT924JeV1qmo2QV6m8YYTWkaVVWNc3nmeTTKsoq2PKVMfQLUKchQbazkT5FqLo4BUC2P2rVFmDnE46QBNjiAsLP)";
        let desc = Descriptor::<String>::from_str(inp).unwrap();
        let desc = Sweeper::descriptors(&PrivateKeys::Desc(desc)).unwrap();
        assert_eq!(desc.len(), 1);
        let w1 = Wallet::new(&desc[0], None, Network::Bitcoin, MemoryDatabase::default())
            .map_err(|e| format!("{} - {}", desc[0], e))
            .unwrap();
        assert_eq!(
            w1.get_address(New).unwrap().to_string(),
            "182vUeQLsdKqkPt5CWsV7Jz3MRUS6vhXgN"
        );
    }
}
