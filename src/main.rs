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
use qmetaobject::*;
use qt_core::{q_standard_paths::StandardLocation, QStandardPaths};

use crate::wallet::create_wallet;

use bdk::{
    bitcoin::Address,
    blockchain::ElectrumBlockchain,
    database::MemoryDatabase,
    electrum_client::{Client, ElectrumApi},
    wallet::AddressIndex,
    SignOptions, SyncOptions, Wallet,
};
use qrcode_png::{Color, QrCode, QrCodeEcc};
use std::{env, fs::create_dir_all, path::PathBuf, str::FromStr};

use gettextrs::{bindtextdomain, textdomain};

mod qrc;
mod transactions;
mod wallet;

const ELECTRUM_SERVER: &str = "ssl://ulrichard.ch:50002";

#[derive(QObject, Default)]
struct Greeter {
    base: qt_base_class!(trait QObject),
    receiving_address: qt_property!(QString),
    wallet: Option<Wallet<MemoryDatabase>>,

    construct_wallet: qt_method!(
        fn construct_wallet(&mut self) {
            self.wallet = Some(log_err(create_wallet()));
        }
    ),
    update_balance: qt_method!(
        fn update_balance(&mut self) -> QString {
            if self.wallet.is_none() {
                self.wallet = Some(log_err(create_wallet()));
            }
            log_err_or(self.get_balance(), "balance unavailable".to_string()).into()
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
                self.wallet = Some(log_err(create_wallet()));
            }
            if addr.is_empty() || amount.is_empty() || fee_rate.is_empty() {
				eprintln!("all the fields need to be filled");
			} else {
				log_err(self.payto(&addr, &amount, &fee_rate));
			}
        }
    ),
    address: qt_method!(
        fn address(&mut self) -> QString {
            if self.wallet.is_none() {
                self.wallet = Some(log_err(create_wallet()));
            }
            let addr = log_err(self.get_receiving_address());
            self.receiving_address = addr.clone().into();
            addr.into()
        }
    ),
    address_qr: qt_method!(
        fn address_qr(&mut self) -> QString {
            if self.wallet.is_none() {
                self.wallet = Some(log_err(create_wallet()));
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
        let app_data_path =
            unsafe { QStandardPaths::writable_location(StandardLocation::AppDataLocation) };
        let app_data_path = PathBuf::from(app_data_path.to_std_string());
        create_dir_all(&app_data_path).unwrap();
        let qr_file = app_data_path.join("receiving.png");

        let mut qrcode = QrCode::new(addr, QrCodeEcc::Medium)
            .map_err(|e| format!("Failed to construct a QR code: {}", e))?;

        qrcode.margin(2);
        qrcode.zoom(8);

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
            .map_err(|e| format!("Failed to synchronize: {:?}", e))?;

        let bal = self
            .wallet
            .as_ref()
            .unwrap()
            .get_balance()
            .map_err(|e| format!("Unable to determine the balance: {:?}", e))?;
        println!("{:?}", bal);
        Ok(format!(
            "Balance: {} (+{}) BTC",
            bal.confirmed as f32 / 100_000_000.0,
            (bal.immature + bal.trusted_pending + bal.untrusted_pending) as f32 / 100_000_000.0
        ))
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

fn log_err_or<T>(res: Result<T, String>, fallback: T) -> T {
    match res {
        Ok(d) => d,
        Err(err) => {
            eprintln!("{}", err);
            fallback
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
    qml_register_type::<transactions::TransactionModel>(
        cstr!("TransactionModel"),
        1,
        0,
        cstr!("TransactionModel"),
    );
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
