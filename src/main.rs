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

mod constants;
mod input_eval;
mod qrc;
mod wallet;

use crate::input_eval::InputEval;
use crate::wallet::BdkWallet;

use ldk_node::bitcoin::Address;
use qrcode_png::{Color, QrCode, QrCodeEcc};
use std::{env, fs::create_dir_all, path::PathBuf, str::FromStr};

use gettextrs::{bindtextdomain, textdomain};

#[derive(QObject, Default)]
struct Greeter {
    base: qt_base_class!(trait QObject),
    receiving_address: qt_property!(QString),

    update_balance: qt_method!(
        fn update_balance(&mut self) -> QString {
            log_err_or(BdkWallet::get_balance(), "balance unavailable".to_string()).into()
        }
    ),
    send: qt_method!(
        fn send(&mut self, addr: String, amount: String) {
            if addr.is_empty() || amount.is_empty() {
                eprintln!("all the fields need to be filled");
            } else {
                log_err(self.payto(&addr, &amount));
            }
        }
    ),
    channel_open: qt_method!(
        fn channel_open(&mut self, amount: String) {
            if amount.is_empty() {
                eprintln!("the amount field needs to be filled");
            } else {
                log_err(self.channel_new(&amount));
            }
        }
    ),
    request: qt_method!(
        fn request(&mut self, amount: String, desc: String) -> QString {
            let invoice = log_err(self.invoice(&amount, &desc));
            self.receiving_address = invoice.clone().into();
            format!(
                "file://{}",
                log_err(self.generate_qr(&invoice)).to_str().unwrap()
            )
            .into()
        }
    ),
    address: qt_method!(
        fn address(&mut self) -> QString {
            let addr = log_err(self.get_receiving_address());
            self.receiving_address = addr.clone().into();
            addr.into()
        }
    ),
    address_qr: qt_method!(
        fn address_qr(&mut self) -> QString {
            let addr = log_err(self.get_receiving_address());
            self.receiving_address = addr.clone().into();
            format!(
                "file://{}",
                log_err(self.generate_qr(&addr)).to_str().unwrap()
            )
            .into()
        }
    ),
    evaluate_address_input: qt_method!(
        fn evaluate_address_input(&mut self, addr: String) {
            println!("qt_method: evaluate_address_input()");
            if !addr.is_empty() {
                log_err(self.evaluate_input(&addr));
            }
        }
    ),
}

impl Greeter {
    fn payto(&self, addr: &str, amount: &str) -> Result<(), String> {
        let recipient = Address::from_str(addr)
            .map_err(|e| format!("Failed to parse address {} : {}", addr, e))?;
        let amount = parse_satoshis(amount)?;

        BdkWallet::payto(recipient, amount)?;
        Ok(())
    }

    fn channel_new(&self, amount: &str) -> Result<(), String> {
        let amount = parse_satoshis(amount)?;
        BdkWallet::channel_open(amount)?;
        Ok(())
    }

    fn invoice(&self, amount: &str, desc: &str) -> Result<String, String> {
        let amount = if amount.is_empty() {
            None
        } else {
            Some(parse_satoshis(amount)?)
        };
        BdkWallet::invoice(amount, desc)
    }

    fn evaluate_input(&self, addr: &str) -> Result<String, String> {
        match InputEval::evaluate(addr)? {
            InputEval::Mainnet(addr) => Ok(addr),
            InputEval::Lightning(invoice) => Ok(invoice),
        }
    }

    fn get_receiving_address(&self) -> Result<String, String> {
        let addr = BdkWallet::get_address()?.to_string();
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
        qrcode.zoom(6);

        let buf = qrcode
            .generate(Color::Grayscale(0, 255))
            .map_err(|e| format!("Failed to generate a QR code: {}", e))?;
        std::fs::write(&qr_file, buf)
            .map_err(|e| format!("Failed to write the QR code to file: {}", e))?;

        Ok(qr_file)
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

    println!("Initializing the node singleton.");
    log_err(BdkWallet::init_node());

    println!("Loading file /qml/utwallet.qml.");
    engine.load_file("qrc:/qml/utwallet.qml".into());
    println!("Entering the QML main loop.");
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
