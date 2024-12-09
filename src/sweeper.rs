use crate::input_eval::PrivateKeys;
//use bdk::{
//    blockchain::EsploraBlockchain, database::MemoryDatabase, SignOptions, SyncOptions, Wallet,
//};
use bdk_esplora::{esplora_client, EsploraAsyncExt};
use bdk_wallet::{KeychainKind, SignOptions, Wallet};
use ldk_node::bitcoin::{Address, Network};

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
        let mut wallet = Wallet::create_single(desc.to_string())
            .network(self.network)
            .create_wallet_no_persist()
            .map_err(|e| format!("Failed to construct sweep wallet: {}", e))?;
        let client = esplora_client::Builder::new(&self.esplora_url)
            .build_async()
            .map_err(|e| format!("Failed to synchronize sweep wallet: {}", e))?;
        Self::sync_wallet(&mut wallet, &client)
            .await
            .map_err(|e| format!("Failed to synchronize sweep wallet: {}", e))?;

        let bal = wallet.balance();
        if bal.total().to_sat() <= 0 {
            return Ok(None);
        }
        println!("sweeping {} to {}", bal, destination.to_string());
        let mut builder = wallet.build_tx();
        builder.drain_wallet().drain_to(destination.script_pubkey());
        let mut psbt = builder
            .finish()
            .map_err(|e| format!("Failed to construct sweep transaction: {}", e))?;

        let signopt = SignOptions {
            ..Default::default()
        };
        wallet
            .sign(&mut psbt, signopt)
            .map_err(|e| format!("Failed to sign sweep transaction: {}", e))?;

        let tx = psbt
            .extract_tx()
            .map_err(|e| format!("Failed to extract sweep transaction: {}", e))?;
        client
            .broadcast(&tx)
            .await
            .map_err(|e| format!("Failed to broadcast sweep transaction: {}", e))?;
        Ok(Some(format!("swept {}", bal.total())))
    }

    async fn sync_wallet(
        wallet: &mut Wallet,
        client: &esplora_client::AsyncClient,
    ) -> Result<(), String> {
        const STOP_GAP: usize = 10;
        const BATCH_SIZE: usize = 5;

        let full_scan_request = wallet.start_full_scan();
        let update = client
            .full_scan(full_scan_request, STOP_GAP, BATCH_SIZE)
            .await
            .map_err(|e| format!("Failed to sync sweep wallet: {}", e))?;
        wallet
            .apply_update(update)
            .map_err(|e| format!("Failed to sync sweep wallet: {}", e))?;

        Ok(())
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
    use ldk_node::bitcoin::{bip32::Xpriv, PrivateKey};
    use miniscript::Descriptor;
    use rstest::rstest;
    use std::str::FromStr;

    fn parse_priv(inp: &str) -> PrivateKeys {
        if let Ok(pk) = PrivateKey::from_wif(inp) {
            return PrivateKeys::Pk(pk);
        }

        let xprv = Xpriv::from_str(inp).unwrap();
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
        let w1 = Wallet::create_single(desc[0].to_string())
            .network(Network::Bitcoin)
            .create_wallet_no_persist()
            .map_err(|e| format!("{} - {}", desc[0], e))
            .unwrap();
        assert_eq!(
            w1.peek_address(KeychainKind::External, 0)
                .address
                .to_string(),
            addrs[0]
        );
        let w2 = Wallet::create_single(desc[1].to_string())
            .network(Network::Bitcoin)
            .create_wallet_no_persist()
            .map_err(|e| format!("{} - {}", desc[1], e))
            .unwrap();
        assert_eq!(
            w2.peek_address(KeychainKind::External, 0)
                .address
                .to_string(),
            addrs[1]
        );
        let w3 = Wallet::create_single(desc[2].to_string())
            .network(Network::Bitcoin)
            .create_wallet_no_persist()
            .map_err(|e| format!("{} - {}", desc[2], e))
            .unwrap();
        assert_eq!(
            w3.peek_address(KeychainKind::External, 0)
                .address
                .to_string(),
            addrs[2]
        );
        let w4 = Wallet::create_single(desc[3].to_string())
            .network(Network::Bitcoin)
            .create_wallet_no_persist()
            .map_err(|e| format!("{} - {}", desc[3], e))
            .unwrap();
        assert_eq!(
            w4.peek_address(KeychainKind::External, 0)
                .address
                .to_string(),
            addrs[3]
        );
    }

    #[test]
    fn test_sweep_desc() {
        let inp = "pkh(xprv9z1Nt86QQeoGXTjrvKgbFT924JeV1qmo2QV6m8YYTWkaVVWNc3nmeTTKsoq2PKVMfQLUKchQbazkT5FqLo4BUC2P2rVFmDnE46QBNjiAsLP)";
        let desc = Descriptor::<String>::from_str(inp).unwrap();
        let desc = Sweeper::descriptors(&PrivateKeys::Desc(desc)).unwrap();
        assert_eq!(desc.len(), 1);
        let w1 = Wallet::create_single(&desc[0])
            .network(Network::Bitcoin)
            .create_wallet_no_persist()
            .map_err(|e| format!("{} - {}", desc[0], e))
            .unwrap();
        assert_eq!(
            w1.peek_address(KeychainKind::External, 0)
                .address
                .to_string(),
            "182vUeQLsdKqkPt5CWsV7Jz3MRUS6vhXgN"
        );
    }
}
