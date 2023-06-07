/*
 * Copyright (C) 2022  Richard Ulrich
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; version 3.
 *
 * utlnwallet is distributed in the hope that it will be useful,
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

use crate::input_eval::{is_node_id, parse_satoshis, InputEval, InputNetwork};
use crate::wallet::BdkWallet;

use qrcode_png::{Color, QrCode, QrCodeEcc};
use std::{env, fs::create_dir_all, path::PathBuf /*, str::FromStr*/};

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
    update_channel: qt_method!(
        fn update_channel(&mut self) -> QString {
            log_err_or(
                BdkWallet::get_channel_status(),
                "channel balance unavailable".to_string(),
            )
            .into()
        }
    ),
    ldk_events: qt_method!(
        fn ldk_events(&mut self) -> QString {
            log_err_or(BdkWallet::handle_ldk_event(), "".to_string()).into()
        }
    ),
    send: qt_method!(
        fn send(&mut self, addr: String, amount: String, desc: String) {
            if addr.is_empty() {
                eprintln!("at least the address field needs to be filled");
            } else {
                log_err(self.payto(&addr, &amount, &desc));
            }
        }
    ),
    channel_open: qt_method!(
        fn channel_open(&mut self, amount: String, node_id: String) {
            if amount.is_empty() {
                eprintln!("the amount field needs to be filled");
            } else {
                log_err(self.channel_new(&amount, &node_id));
            }
        }
    ),
    channel_close: qt_method!(
        fn channel_close(&mut self) {
            log_err(BdkWallet::channel_close());
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
        fn evaluate_address_input(
            &mut self,
            addr: String,
            amount: String,
            desc: String,
        ) -> QString {
            log_err(self.evaluate_input(&addr, &amount, &desc)).into()
        }
    ),
}

impl Greeter {
    fn payto(&self, addr: &str, bitcoins: &str, desc: &str) -> Result<(), String> {
        let satoshis = if bitcoins.is_empty() {
            None
        } else {
            Some(parse_satoshis(bitcoins)?)
        };
        let inpeval = InputEval::evaluate(addr, bitcoins, desc)?;
        match inpeval.network {
            InputNetwork::Mainnet(addr) => {
                if let Some(satoshis) = satoshis {
                    Ok(BdkWallet::payto(addr, satoshis)?.to_string())
                } else {
                    Err("Amount field needs to be filled!".to_string())
                }
            }
            InputNetwork::Lightning(invoice) => BdkWallet::pay_invoice(&invoice, satoshis),
        }?;

        Ok(())
    }

    fn channel_new(&self, amount: &str, node_id: &str) -> Result<(), String> {
        let amount = parse_satoshis(amount)?;
        let node_id = if is_node_id(node_id) {
            Some(node_id)
        } else {
            None
        };
        BdkWallet::channel_open(amount, node_id)?;
        Ok(())
    }

    fn invoice(&self, amount: &str, desc: &str) -> Result<String, String> {
        let amount = if amount.is_empty() {
            None
        } else {
            Some(parse_satoshis(amount)?)
        };
        BdkWallet::create_invoice(amount, desc)
    }

    fn evaluate_input(&self, addr: &str, bitcoins: &str, desc: &str) -> Result<String, String> {
        let inpeval = InputEval::evaluate(addr, bitcoins, desc)?;
        inpeval.gui_csv()
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

fn main() {
    init_gettext();
    unsafe {
        cpp! { {
            #include <QtCore/QCoreApplication>
            #include <QtCore/QString>
        }}
        cpp! {[]{
            QCoreApplication::setApplicationName(QStringLiteral("utlnwallet.ulrichard"));
        }}
    }
    QQuickStyle::set_style("Suru");
    qrc::load();
    qml_register_type::<Greeter>(cstr!("Greeter"), 1, 0, cstr!("Greeter"));
    let mut engine = QmlEngine::new();

    println!("Initializing the node singleton.");
    log_err(BdkWallet::init_node());

    println!("Loading file /qml/utlnwallet.qml.");
    engine.load_file("qrc:/qml/utlnwallet.qml".into());
    println!("Entering the QML main loop.");
    engine.exec();
}

fn init_gettext() {
    let domain = "utlnwallet.ulrichard";
    textdomain(domain).expect("Failed to set gettext domain");

    let app_dir = env::var("APP_DIR").expect("Failed to read the APP_DIR environment variable");

    let mut app_dir_path = PathBuf::from(app_dir);
    if !app_dir_path.is_absolute() {
        app_dir_path = PathBuf::from("/usr");
    }

    let path = app_dir_path.join("share/locale");

    bindtextdomain(domain, path.to_str().unwrap()).expect("Failed to bind gettext domain");
}
