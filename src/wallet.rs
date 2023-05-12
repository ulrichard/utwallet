/*
 * Copyright (C) 2022  Richard Ulrich
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; version 3.
 *
 * utwallet is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use qt_core::{q_standard_paths::StandardLocation, QStandardPaths};

use crate::constants::{ESPLORA_SERVERS, LN_ULR};

use ldk_node::bip39::Mnemonic;
use ldk_node::bitcoin::{secp256k1::PublicKey, Address, /*Network,*/ Txid};
use ldk_node::lightning_invoice::Invoice;
use ldk_node::{Builder, Node};
use rand_core::{OsRng, RngCore};
use std::{fs, fs::create_dir_all, fs::File, io::Write, path::PathBuf, str::FromStr, sync::Mutex};

pub struct BdkWallet {}

static UTNODE: Mutex<Option<Node>> = Mutex::new(None);

/// A facade for bdk::Wallet with a singleton instance
impl BdkWallet {
    pub fn init_node() -> Result<(), String> {
        *UTNODE.lock().unwrap() = Some(Self::create_node()?);
        Ok(())
    }

    pub fn payto(recipient: Address, amount: u64) -> Result<Txid, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        //if let Err(e) = node.sync_wallets() {
        //    eprintln!("Failed to sync the wallet: {:?}", e);
        //}

        let txid = node
            .send_to_onchain_address(&recipient, amount)
            .map_err(|e| format!("Failed to send on-chain: {:?}", e))?;

        Ok(txid)
    }

    pub fn channel_open(amount: u64) -> Result<(), String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        //if let Err(e) = node.sync_wallets() {
        //    eprintln!("Failed to sync the wallet: {:?}", e);
        //}

        let id_addr = LN_ULR.split("@").collect::<Vec<_>>();
        assert_eq!(id_addr.len(), 2);
        let node_id = PublicKey::from_str(id_addr[0]).unwrap();
        let node_addr = id_addr[1].parse().unwrap();
        node.connect_open_channel(node_id, node_addr, amount, None, false)
            .map_err(|e| format!("Failed to open a channel: {:?}", e))?;

        Ok(())
    }

    pub fn create_invoice(amount: Option<u64>, desc: &str) -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        //if let Err(e) = node.sync_wallets() {
        //    eprintln!("Failed to sync the wallet: {:?}", e);
        //}

        let expiry_secs = 60 * 15;
        let invoice = if let Some(amount) = amount {
            node.receive_payment(amount * 1_000, desc, expiry_secs)
        } else {
            node.receive_variable_amount_payment(desc, expiry_secs)
        }
        .map_err(|e| format!("Failed to create an invoice: {:?}", e))?;

        Ok(invoice.to_string())
    }

    pub fn pay_invoice(invoice: &Invoice, amount: Option<u64>) -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let ph = match (invoice.amount_milli_satoshis(), amount) {
            (Some(_amount), None) => node
                .send_payment(invoice)
                .map_err(|e| format!("Unable to pay the invoice: {:?}", e)),
            (Some(amount_inv), Some(amount_field)) => {
                if (amount_inv as i64 - amount_field as i64 * 1_000).abs() > 1_000_000 {
                    Err(format!(
                        "amount of the invoice {} and in the field {} don't match",
                        amount_inv,
                        amount_field * 1_000
                    ))
                } else {
                    node.send_payment(invoice)
                        .map_err(|e| format!("Unable to pay the invoice: {:?}", e))
                }
            }
            (None, Some(amount)) => node
                .send_payment_using_amount(invoice, amount * 1_000)
                .map_err(|e| format!("Unable to pay the invoice with {} sats: {:?}", amount, e)),
            (None, None) => Err("No amount to pay the invoice!".to_string()),
        }?;

        Ok(format!("{:?}", ph))
    }

    pub fn get_address() -> Result<Address, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        // if let Err(e) = node.sync_wallets() {
        //    eprintln!("Failed to sync the wallet: {:?}", e);
        //}

        node.new_funding_address()
            .map_err(|e| format!("Unable to get an address: {:?}", e))
    }

    pub fn get_balance() -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        if let Err(e) = node.sync_wallets() {
            eprintln!("Failed to sync the wallet: {:?}", e);
        }

        let bal = node
            .onchain_balance()
            .map_err(|e| format!("Unable to get on-chain balance: {:?}", e))?;

        println!("{:?}", bal);
        Ok(format!(
            "Balance: {} (+{}) BTC",
            bal.confirmed as f32 / 100_000_000.0,
            (bal.immature + bal.trusted_pending + bal.untrusted_pending) as f32 / 100_000_000.0
        ))
    }

    fn create_node() -> Result<Node, String> {
        //let network = Network::Bitcoin;
        let app_data_path =
            unsafe { QStandardPaths::writable_location(StandardLocation::AppDataLocation) };
        let mnemonic_file = PathBuf::from(app_data_path.to_std_string()).join("mnemonic.txt");
        let ldk_dir = PathBuf::from(app_data_path.to_std_string()).join("ldk");
        //let wallet_file = PathBuf::from(app_data_path.to_std_string()).join("wallet.descriptor");

        let mnemonic_words = if mnemonic_file.exists() {
            fs::read_to_string(&mnemonic_file).map_err(|e| {
                format!(
                    "Failed to read the mnemonic file {:?}: {}",
                    mnemonic_file, e
                )
            })?
        /*
                } else if wallet_file.exists() {
                    // older versions stored a pair of descriptors
                    let json = fs::read_to_string(&wallet_file)
                        .map_err(|e| format!("Failed to read the wallet file {:?}: {}", wallet_file, e))?;
                    serde_json::from_str(&json).unwrap()
                    let desc: (String, String) = json;
                    let rgx_descriptor_wpkh_xprv = r#"^wpkh\((xprv[a-zA-Z0-9]+)/[0-9]+/\*\)$"#;
                    let re = Regex::new(rgx_descriptor_wpkh_xprv).unwrap();
                    let capt = re.captures(&desc.0).unwrap();
                    let xpriv = capt.get(1).unwrap().as_str();
                    let xpriv = ExtendedPrivKey::from_str(xpriv).unwrap();
                    // there seems to be no way to get from an ExtendedPrivKey to a mnemonic
        */
        } else {
            // Generate fresh mnemonic
            let mut entropy = [0u8; 16];
            OsRng.fill_bytes(&mut entropy);
            let mnemonic = Mnemonic::from_entropy(&entropy)
                .map_err(|e| format!("Failed to generate mnemonic: {:?}", e))?;
            mnemonic.to_string()
        };

        let mnemonic = Mnemonic::parse(&mnemonic_words)
            .map_err(|e| format!("Failed to parse mnemonic: {}", e))?;

        let prefix = mnemonic_file
            .parent()
            .ok_or("Failed to get parent path".to_string())?;
        create_dir_all(prefix).map_err(|e| format!("Failed to create directory: {}", e))?;
        let mut output = File::create(mnemonic_file)
            .map_err(|e| format!("Failed to create mnemonic file: {}", e))?;
        write!(output, "{}", mnemonic_words)
            .map_err(|e| format!("Failed to write mnemonic file: {}", e))?;

        let node = Builder::new()
            .set_network("bitcoin")
            .set_esplora_server_url(ESPLORA_SERVERS[0].to_string())
            .set_entropy_bip39_mnemonic(mnemonic, None)
            .set_storage_dir_path(ldk_dir.to_str().unwrap().to_string())
            .build();
        node.start().unwrap();

        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ldk_node::bitcoin::{secp256k1::PublicKey, util::bip32::ExtendedPrivKey, Network};
    use regex::Regex;
    use std::str::FromStr;

    #[test]
    fn test_descriptor_to_mnemonic() {
        let mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        let seed_bytes = mnemonic.to_seed("");
        let xprv = ExtendedPrivKey::new_master(Network::Bitcoin, &seed_bytes).unwrap();
        let desc = format!("wpkh({}/0/*)", xprv);
        assert_eq!(desc, "wpkh(xprv9s21ZrQH143K3GJpoapnV8SFfukcVBSfeCficPSGfubmSFDxo1kuHnLisriDvSnRRuL2Qrg5ggqHKNVpxR86QEC8w35uxmGoggxtQTPvfUu/0/*)");
        let rgx_descriptor_wpkh_xprv = r#"^wpkh\((xprv[a-zA-Z0-9]+)/[0-9]+/\*\)$"#;
        let re = Regex::new(rgx_descriptor_wpkh_xprv).unwrap();
        assert!(re.is_match(&desc));
        let capt = re.captures(&desc).unwrap();
        assert_eq!(capt.len(), 2);
        let xpriv = capt.get(1).unwrap().as_str();
        assert_eq!(xpriv, "xprv9s21ZrQH143K3GJpoapnV8SFfukcVBSfeCficPSGfubmSFDxo1kuHnLisriDvSnRRuL2Qrg5ggqHKNVpxR86QEC8w35uxmGoggxtQTPvfUu");
        let xpriv = ExtendedPrivKey::from_str(xpriv).unwrap();
        assert_eq!(xprv, xpriv);
    }

    #[test]
    fn test_init_node() {
        let mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        let node = Builder::new()
            .set_network("bitcoin")
            .set_esplora_server_url(crate::constants::ESPLORA_SERVERS[0].to_string())
            .set_entropy_bip39_mnemonic(mnemonic, Some("TREZOR".to_string()))
            .build();
        node.start().unwrap();

        assert_eq!(format!("{:?}", node.node_id()), "PublicKey(9720ef321576ad8d8709809f4a3a44c217fcef447475f712c3b02c0a2a1b4d4936f030becdc20dd920e1bfa4647fbefd7919bd6ea04ecb82e8eb8d926dd294a0)");
        assert_eq!(
            format!("{:?}", node.new_funding_address()),
            "Ok(bc1qv5rmq0kt9yz3pm36wvzct7p3x6mtgehjul0feu)"
        );
        assert_eq!(
            format!("{:?}", node.onchain_balance()),
            "Ok(Balance { immature: 0, trusted_pending: 0, untrusted_pending: 0, confirmed: 0 })"
        );
        assert_eq!(format!("{:?}", node.list_channels()), "[]");
        assert_eq!(
            format!("{:?}", node.listening_address()),
            "Some(0.0.0.0:9735)"
        );

        // .. fund address ..

        if let Err(e) = node.sync_wallets() {
            eprintln!("Failed to sync the node: {}", e);
        }

        let invoice = node
            .receive_variable_amount_payment("test", 60 * 30)
            .unwrap();
        assert_eq!(invoice.amount_milli_satoshis(), None);

        let id_addr = crate::constants::LN_ULR.split("@").collect::<Vec<_>>();
        assert_eq!(id_addr.len(), 2);
        let node_id = PublicKey::from_str(id_addr[0]).unwrap();
        let node_addr = id_addr[1].parse().unwrap();
        node.connect(node_id, node_addr, false).unwrap();
        // node.connect_open_channel(node_id, node_addr, 10000, None, false)
        //    .unwrap();

        // let invoice = Invoice::from_str("INVOICE_STR").unwrap();
        // node.send_payment(&invoice).unwrap();

        node.stop().unwrap();
    }
}
