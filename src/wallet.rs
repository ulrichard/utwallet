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

use bdk::{
    bitcoin::Network,
    blockchain::ElectrumBlockchain,
    database::MemoryDatabase,
    electrum_client::Client,
    keys::{
        bip39::{Language, Mnemonic, WordCount},
        DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
    },
    miniscript, SyncOptions, Wallet,
};
use std::{fs, fs::create_dir_all, fs::File, io::Write, path::PathBuf};

const ELECTRUM_SERVER: &str = "ssl://ulrichard.ch:50002";

pub fn create_wallet() -> Result<Wallet<MemoryDatabase>, String> {
    // load the wallet
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
    let mut output =
        File::create(wallet_file).map_err(|e| format!("Failed to create wallet file: {}", e))?;
    let json = serde_json::to_string_pretty(&(&descriptors.0, &descriptors.1))
        .map_err(|e| format!("Failed to format wallet file: {}", e))?;
    write!(output, "{}", json).map_err(|e| format!("Failed to write wallet file: {}", e))?;

    let client = Client::new(ELECTRUM_SERVER).unwrap();
    let blockchain = ElectrumBlockchain::from(client);
    if let Err(e) = wallet.sync(&blockchain, SyncOptions::default()) {
        eprintln!("failed to synchronize wallet: {}", e);
    };

    Ok(wallet)
}
