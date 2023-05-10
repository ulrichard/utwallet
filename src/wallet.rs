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

use crate::constants::ESPLORA_SERVERS;

use bdk::{
    bitcoin::{Address, Network},
    blockchain::{Blockchain, EsploraBlockchain},
    database::MemoryDatabase,
    keys::{
        bip39::{Language, Mnemonic, WordCount},
        DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
    },
    miniscript,
    wallet::{AddressIndex, AddressInfo},
    FeeRate, SignOptions, SyncOptions, Wallet,
};

use std::sync::Mutex;
use std::{fs, fs::create_dir_all, fs::File, io::Write, path::PathBuf};

pub struct BdkWallet {
    wallet: Wallet<MemoryDatabase>,
    runtime: tokio::runtime::Runtime,
}

static UTWALLET: Mutex<Option<BdkWallet>> = Mutex::new(None);

/// A facade for bdk::Wallet with a singleton instance
impl BdkWallet {
    pub fn init_wallet() -> Result<(), String> {
        *UTWALLET.lock().unwrap() = Some(Self::create_wallet()?);
        Ok(())
    }

    pub fn payto(recipient: Address, amount: u64, fee_rate: FeeRate) -> Result<(), String> {
        let wallet_m = UTWALLET
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let wallet = wallet_m.as_ref().ok_or("The wallet was not initialized")?;
        let blockchain = Self::get_esplora_blockchain()?;

        wallet.runtime.block_on(async {
            wallet
                .wallet
                .sync(&blockchain, SyncOptions::default())
                .await
                .map_err(|e| format!("Failed to synchronize: {:?}", e))?;

            // construct the tx
            let mut builder = wallet.wallet.build_tx();
            builder
                .add_recipient(recipient.script_pubkey(), amount)
                .enable_rbf()
                .fee_rate(fee_rate);
            let (mut psbt, _) = builder
                .finish()
                .map_err(|e| format!("Failed to finish the transaction: {}", e))?;

            // sign
            let signopt = SignOptions {
                ..Default::default()
            };
            let finalized = wallet
                .wallet
                .sign(&mut psbt, signopt)
                .map_err(|e| format!("Failed to sign the transaction: {}", e))?;
            if !finalized {
                println!("The tx is not finalized after signing");
            }

            // broadcast
            let tx = psbt.extract_tx();
            blockchain
                .broadcast(&tx)
                .await
                .map_err(|e| format!("Failed to broadcast the transaction: {}", e))?;
            Ok(())
        })
    }

    pub fn get_address(address_index: AddressIndex) -> Result<AddressInfo, String> {
        let wallet_m = UTWALLET
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let wallet = wallet_m.as_ref().ok_or("The wallet was not initialized")?;
        wallet
            .wallet
            .get_address(address_index)
            .map_err(|e| format!("Failed to get an daddress: {:?}", e))
    }

    pub fn get_balance() -> Result<String, String> {
        let wallet_m = UTWALLET
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let wallet = wallet_m.as_ref().ok_or("The wallet was not initialized")?;
        let blockchain = Self::get_esplora_blockchain()?;

        let bal = wallet.runtime.block_on(async {
            wallet
                .wallet
                .sync(&blockchain, SyncOptions::default())
                .await
                .map_err(|e| format!("Failed to synchronize: {:?}", e))?;

            wallet
                .wallet
                .get_balance()
                .map_err(|e| format!("Unable to determine the balance: {:?}", e))
        })?;
        println!("{:?}", bal);
        Ok(format!(
            "Balance: {} (+{}) BTC",
            bal.confirmed as f32 / 100_000_000.0,
            (bal.immature + bal.trusted_pending + bal.untrusted_pending) as f32 / 100_000_000.0
        ))
    }

    pub fn get_transactions() -> Result<Vec<(u64, f32)>, String> {
        let wallet_m = UTWALLET
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let wallet = wallet_m.as_ref().ok_or("The wallet was not initialized")?;
        let blockchain = Self::get_esplora_blockchain()?;

        let transactions: Result<_, String> = wallet.runtime.block_on(async {
            wallet
                .wallet
                .sync(&blockchain, SyncOptions::default())
                .await
                .map_err(|e| format!("Failed to synchronize: {:?}", e))?;

            let mut transactions = wallet
                .wallet
                .list_transactions(false)
                .map_err(|e| format!("Unable to get transactions: {:?}", e))?;
            transactions.sort_by(|a, b| {
                b.confirmation_time
                    .as_ref()
                    .map(|t| t.height)
                    .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
            });
            let transactions: Vec<_> = transactions
                .iter()
                .map(|td| {
                    (
                        match &td.confirmation_time {
                            Some(ct) => ct.timestamp,
                            None => 0,
                        },
                        (td.received as f32 - td.sent as f32) / 100_000_000.0,
                    )
                })
                .collect();
            Ok(transactions)
        });
        let transactions = transactions?;
        println!("{:?}", transactions);

        Ok(transactions)
    }

    pub fn get_fee_rate(blocks: usize) -> Result<f32, String> {
        let wallet_m = UTWALLET
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let wallet = wallet_m.as_ref().ok_or("The wallet was not initialized")?;
        let blockchain = Self::get_esplora_blockchain()?;

        wallet.runtime.block_on(async {
            let fee_rate = blockchain
                .estimate_fee(blocks)
                .await
                .map_err(|e| format!("Failed to get fee estimation from electrum: {:?}", e))?;

            Ok(fee_rate.as_sat_per_vb())
        })
    }

    fn get_esplora_blockchain() -> Result<EsploraBlockchain, String> {
        let wallet_m = UTWALLET
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let wallet = wallet_m.as_ref().ok_or("The wallet was not initialized")?;

        wallet.runtime.block_on(async {
            for url in ESPLORA_SERVERS {
                let blockchain = EsploraBlockchain::new(&url, 20);
                if let Err(err) = blockchain.get_height().await {
                    eprintln!("esplora server error {} : {:?}", url, err);
                    continue;
                }
                return Ok(blockchain);
            }
            Err("None of the esplora servers from the list could be reached. {}".to_string())
        })
    }

    fn create_wallet() -> Result<BdkWallet, String> {
        let network = Network::Bitcoin;
        let app_data_path =
            unsafe { QStandardPaths::writable_location(StandardLocation::AppDataLocation) };
        let wallet_file = PathBuf::from(app_data_path.to_std_string()).join("wallet.descriptor");

        let descriptors: (String, String) = if wallet_file.exists() {
            let json = fs::read_to_string(&wallet_file)
                .map_err(|e| format!("Failed to read the wallet file {:?}: {}", wallet_file, e))?;
            serde_json::from_str(&json).unwrap()
        } else {
            // Generate fresh mnemonic
            let mnemonic: GeneratedKey<_, miniscript::Segwitv0> =
                Mnemonic::generate((WordCount::Words12, Language::English))
                    .map_err(|e| format!("Failed to generate mnemonic: {:?}", e))?;
            // Convert mnemonic to string
            let mnemonic_words = mnemonic.to_string();
            // Parse a mnemonic
            let mnemonic = Mnemonic::parse(&mnemonic_words)
                .map_err(|e| format!("Failed to parse mnemonic: {}", e))?;
            // Generate the extended key
            let xkey: ExtendedKey = mnemonic
                .into_extended_key()
                .map_err(|e| format!("Failed to convert mnemonic to xprv: {}", e))?;
            // Get xprv from the extended key
            let xprv = xkey
                .into_xprv(network)
                .ok_or("Failed to convert xprv".to_string())?;

            (format!("wpkh({}/0/*)", xprv), format!("wpkh({}/1/*)", xprv))
        };

        let wallet = Wallet::new(
            &descriptors.0,
            Some(&descriptors.1),
            network,
            MemoryDatabase::default(),
        )
        .map_err(|e| format!("Failed to construct wallet: {}", e))?;

        let prefix = wallet_file
            .parent()
            .ok_or("Failed to get parent path".to_string())?;
        create_dir_all(prefix).map_err(|e| format!("Failed to create directory: {}", e))?;
        let mut output = File::create(wallet_file)
            .map_err(|e| format!("Failed to create wallet file: {}", e))?;
        let json = serde_json::to_string_pretty(&(&descriptors.0, &descriptors.1))
            .map_err(|e| format!("Failed to format wallet file: {}", e))?;
        write!(output, "{}", json).map_err(|e| format!("Failed to write wallet file: {}", e))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to construct async runtime: {}", e))?;

        let bdkwallet = BdkWallet { wallet, runtime };

        Ok(bdkwallet)
    }
}
