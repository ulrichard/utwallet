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

#[macro_use]
extern crate cstr;
extern crate cpp;
#[macro_use]
extern crate qmetaobject;
use qt_core::{QStandardPaths, q_standard_paths::StandardLocation};

use bdk::{
    bitcoin::{Address, Network},
    blockchain::ElectrumBlockchain,
    database::MemoryDatabase,
    electrum_client::{Client, ElectrumApi},
    keys::{
        bip39::{Language, Mnemonic, WordCount},
        DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
    },
    miniscript,
    wallet::AddressIndex,
    KeychainKind, SignOptions, SyncOptions, Wallet,
};
use qrcode_png::{Color, QrCode, QrCodeEcc};
use std::{env, fs, fs::File, fs::create_dir_all, io::Write, path::PathBuf, str::FromStr};

use gettextrs::{bindtextdomain, textdomain};
use qmetaobject::*;

mod qrc;

const ELECTRUM_SERVER: &str = "ssl://ulrichard.ch:50002";

#[derive(QObject, Default)]
struct Greeter {
    base: qt_base_class!(trait QObject),
    receiving_address: qt_property!(QString),
    update_balance: qt_method!(
        fn update_balance(&mut self) -> QString {
			println!("updating balance");
            if self.wallet.is_none() {
                self.wallet = Some(log_err(Greeter::create_wallet()));
            }
            log_err(self.get_balance()).into()
        }
    ),
    estimate_fee: qt_method!(
        fn estimate_fee(&self) -> QString {
            format!("{}", log_err(get_fee_rate(1))).into()
        }
    ),
    send: qt_method!(
        fn send(&mut self, addr: String, amount: String, fee_rate: String) {
            if self.wallet.is_none() {
                self.wallet = Some(log_err(Greeter::create_wallet()));
            }
            log_err(self.payto(&addr, &amount, &fee_rate));
        }
    ),
    address_qr: qt_method!(
        fn address_qr(&mut self) -> QString {
            if self.wallet.is_none() {
                self.wallet = Some(log_err(Greeter::create_wallet()));
            }
            let addr = log_err(self.get_receiving_address());
            self.receiving_address = addr.clone().into();
            format!(
                "file://{}",
                log_err(self.generate_qr(&addr)).to_str().unwrap()
            )
            .into()
        }
    ),
    wallet: Option<Wallet<MemoryDatabase>>,
}

impl Greeter {
    fn payto(&self, addr: &str, amount: &str, fee_rate: &str) -> Result<String, String> {
        let wallet = self.wallet.as_ref().unwrap();
        let recipient = Address::from_str(addr)
            .map_err(|e| format!("Failed to parse address {} : {}", addr, e))?;
        let amount = parse_satoshis(amount)?;
        let fee_rate = bdk::FeeRate::from_sat_per_vb(
            f32::from_str(fee_rate)
                .map_err(|e| format!("Failed to parse fee_rate {} : {}", fee_rate, e))?,
        );

        // construct the tx
        let mut builder = wallet.build_tx();
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
            .sign(&mut psbt, signopt)
            .map_err(|e| format!("Failed to sign the transaction: {}", e))?;
        if !finalized {
            println!("The tx is not finalized after signing");
        }

        // broadcast
        let tx = psbt.extract_tx();
        let client = Client::new(ELECTRUM_SERVER)
            .map_err(|e| format!("Failed to construct an electrum client: {}", e))?;
        let txid = client
            .transaction_broadcast(&tx)
            .map_err(|e| format!("Failed to broadcast the transaction: {}", e))?;

        Ok(txid.to_string())
    }

    fn get_receiving_address(&self) -> Result<String, String> {
        let wallet = self.wallet.as_ref().unwrap();
        let addr = wallet
            .get_address(AddressIndex::New)
            .map_err(|e| format!("Failed to get an address from the wallet: {}", e))?
            .to_string();
        Ok(addr)
    }

    fn generate_qr(&self, addr: &str) -> Result<PathBuf, String> {
		let app_data_path =  unsafe {
			QStandardPaths::writable_location(StandardLocation::AppDataLocation)
		};
	    let app_data_path = PathBuf::from(app_data_path.to_std_string());
        create_dir_all(&app_data_path).unwrap();
        let qr_file = app_data_path.join("receiving.png");

        let mut qrcode = QrCode::new(addr, QrCodeEcc::Medium)
            .map_err(|e| format!("Failed to construct a QR code: {}", e))?;

        qrcode.margin(2);
        qrcode.zoom(6);

        let buf = qrcode
            .generate(Color::Grayscale(0, 255))
            .map_err(|e| format!("Failed to generate a QR code: {}", e))?;
        std::fs::write(&qr_file, buf)
            .map_err(|e| format!("Failed to write the QR code to file: {}", e))?;

        Ok(qr_file)
    }

    pub fn get_balance(&self) -> Result<String, String> {
        let client = Client::new(ELECTRUM_SERVER).unwrap();
        let blockchain = ElectrumBlockchain::from(client);

        self.wallet
            .as_ref()
            .unwrap()
            .sync(&blockchain, SyncOptions::default())
            .unwrap();

        match self.wallet.as_ref().unwrap().get_balance() {
            Ok(bal) => Ok(format!(
                "Balance: {} (+{}) BTC",
                bal.confirmed / 100_000_000,
                (bal.immature + bal.trusted_pending + bal.untrusted_pending) / 100_000_000
            )),
            Err(e) => Err(format!("Unable to determine the balance: {:?}", e)),
        }
    }

    pub fn create_wallet() -> Result<Wallet<MemoryDatabase>, String> {
        // load the wallet
        let network = Network::Bitcoin;
        
        let app_data_path =  unsafe {
			QStandardPaths::writable_location(StandardLocation::AppDataLocation)
		};
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
            let xkey: ExtendedKey = mnemonic.into_extended_key()
                .map_err(|e| format!("Failed to convert mnemonic to xprv: {}", e))?;
            // Get xprv from the extended key
            let xprv = xkey.into_xprv(network)
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

        let prefix = wallet_file.parent()
			.ok_or("Failed to get parent path".to_string())?;
        create_dir_all(prefix)
			.map_err(|e| format!("Failed to create directory: {}", e))?;
        let mut output = File::create(wallet_file)
			.map_err(|e| format!("Failed to create wallet file: {}", e))?;
        let json = serde_json::to_string_pretty(&(
            wallet
                .get_descriptor_for_keychain(KeychainKind::External)
                .to_string(),
            wallet
                .get_descriptor_for_keychain(KeychainKind::Internal)
                .to_string(),
        ))
		.map_err(|e| format!("Failed to format wallet file: {}", e))?;
        write!(output, "{}", json)
		.map_err(|e| format!("Failed to write wallet file: {}", e))?;

        Ok(wallet)
    }
}

fn log_err<T>(res: Result<T, String>) -> T {
    match res {
        Ok(d) => d,
        Err(err) => {
			eprintln!("{}", err);
            panic!("{}", err);
        }
    }
}

fn get_fee_rate(blocks: usize) -> Result<f32, String> {
    let client = Client::new(ELECTRUM_SERVER)
        .map_err(|e| format!("Failed to construct an electrum client: {}", e))?;
    let blockchain = ElectrumBlockchain::from(client);

    let fee_rate = blockchain
        .estimate_fee(blocks)
        .map_err(|e| format!("Failed to get fee estimation from electrum: {:?}", e))?;

    let fee_rate = bdk::FeeRate::from_btc_per_kvb(fee_rate as f32); // according to the documentation, this should not be needed

    Ok(fee_rate.as_sat_per_vb())
}

/// Convert a string with a value in Bitcoin to Satoshis
fn parse_satoshis(amount: &str) -> Result<u64, String> {
    if amount.is_empty() {
        return Ok(0);
    }
    let amount = f64::from_str(amount)
        .map_err(|e| format!("Failed to parse the satoshis from {:?} : {}", amount, e))?;
    Ok((amount * 100_000_000.0) as u64)
}

fn main() {
    init_gettext();
    unsafe {
        cpp! { {
            #include <QtCore/QCoreApplication>
            #include <QtCore/QString>
        }}
        cpp! {[]{
            QCoreApplication::setApplicationName(QStringLiteral("utwallet.ulrichard"));
        }}
    }
    QQuickStyle::set_style("Suru");
    qrc::load();
    qml_register_type::<Greeter>(cstr!("Greeter"), 1, 0, cstr!("Greeter"));
    let mut engine = QmlEngine::new();
    engine.load_file("qrc:/qml/Main.qml".into());
    engine.exec();
}

fn init_gettext() {
    let domain = "utwallet.ulrichard";
    textdomain(domain).expect("Failed to set gettext domain");

    let app_dir = env::var("APP_DIR").expect("Failed to read the APP_DIR environment variable");

    let mut app_dir_path = PathBuf::from(app_dir);
    if !app_dir_path.is_absolute() {
        app_dir_path = PathBuf::from("/usr");
    }

    let path = app_dir_path.join("share/locale");

    bindtextdomain(domain, path.to_str().unwrap()).expect("Failed to bind gettext domain");
}
