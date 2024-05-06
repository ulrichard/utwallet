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

use ldk_node::bitcoin::{
    bip32::ExtendedPrivKey, secp256k1::PublicKey, Address, Network, PrivateKey,
};
use ldk_node::lightning::ln::msgs::SocketAddress;
use ldk_node::lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescription};
use libelectrum2descriptors::ElectrumExtendedPrivKey;
use lnurl::{api::LnUrlResponse, lightning_address::LightningAddress, lnurl::LnUrl, Builder};
use miniscript::Descriptor;
use regex::Regex;
use std::{collections::HashMap, str::FromStr};

pub struct InputEval {
    pub network: InputNetwork,
    pub satoshis: Option<u64>,
    pub description: String,
}

pub enum PrivateKeys {
    Pk(PrivateKey),
    Epk(ExtendedPrivKey),
    Desc(Descriptor<String>),
}

impl PrivateKeys {
    pub fn to_string(&self) -> String {
        match self {
            Self::Pk(pk) => pk.to_wif(),
            Self::Epk(epk) => epk.to_string(),
            Self::Desc(desc) => desc.to_string(),
        }
    }
}

pub enum InputNetwork {
    Mainnet(Address),
    Lightning(Bolt11Invoice),
    PrivKey(PrivateKeys),
    LnWithdraw(String),
}

impl InputEval {
    pub fn evaluate(recipient: &str, bitcoins: &str, description: &str) -> Result<Self, String> {
        let descr = description.to_string();
        let satoshis = if bitcoins.is_empty() {
            None
        } else {
            Some(parse_satoshis(bitcoins)?)
        };

        let rgx_btc_addr = r#"(bc1|[13])[a-zA-HJ-NP-Z0-9]{25,39}"#;
        let re = Regex::new(&format!("^{}$", rgx_btc_addr)).map_err(|e| e.to_string())?;
        if re.is_match(recipient) {
            return Self::mainnet(recipient, satoshis, descr);
        }

        // https://developer.bitcoin.org/devguide/payment_processing.html
        let re = Regex::new(&format!(
            "^bitcoin:({})([?&](amount|label|message)=([^&]+))*$",
            rgx_btc_addr
        ))
        .map_err(|e| e.to_string())?;
        if re.is_match(recipient) {
            let caps = re.captures(recipient).unwrap();
            let addr = caps.get(1).unwrap().as_str();

            let re = Regex::new("(?P<key>amount|label|message)=(?P<value>[^&]+)")
                .map_err(|e| e.to_string())?;

            let mut props = HashMap::new();
            for caps in re.captures_iter(recipient) {
                props.insert(caps["key"].to_string(), caps["value"].to_string());
            }
            let satoshis = if let Some(sats) = props.get("amount") {
                Some(parse_satoshis(sats)?)
            } else {
                satoshis
            };
            let descr = if let Some(desc) = props.get("label") {
                desc.clone()
            } else {
                descr
            };

            return Self::mainnet(&addr, satoshis, descr);
        }

        // private key
        if let Ok(pk) = PrivateKey::from_wif(&recipient) {
            return Ok(Self {
                network: InputNetwork::PrivKey(PrivateKeys::Pk(pk)),
                satoshis: None,
                description: "sweep private key".to_string(),
            });
        }

        // extended private key
        if let Ok(xprv) = ExtendedPrivKey::from_str(&recipient) {
            return Ok(Self {
                network: InputNetwork::PrivKey(PrivateKeys::Epk(xprv)),
                satoshis: None,
                description: "sweep private keys".to_string(),
            });
        }

        // electrum extended private key
        if let Ok(exprv) = ElectrumExtendedPrivKey::from_str(recipient) {
            return Ok(Self {
                network: InputNetwork::PrivKey(PrivateKeys::Epk(*exprv.xprv())),
                satoshis: None,
                description: "sweep private keys".to_string(),
            });
        }

        // miniscript descriptor
        if let Ok(desc) = Descriptor::<String>::from_str(&recipient) {
            desc.sanity_check()
                .map_err(|e| format!("Descriptor failed sanity check: {}", e))?;
            return Ok(Self {
                network: InputNetwork::PrivKey(PrivateKeys::Desc(desc)),
                satoshis: None,
                description: "sweep private keys".to_string(),
            });
        }

        // https://www.bolt11.org/
        let rgx_bolt11 = r#"^(?i)(LIGHTNING:)?lnbc[a-z0-9]{100,700}$"#;
        let re = Regex::new(&rgx_bolt11).map_err(|e| e.to_string())?;
        if re.is_match(recipient) {
            let recipient = recipient
                .replace("LIGHTNING:", "")
                .replace("lightning:", "");
            let invoice = str::parse::<Bolt11Invoice>(&recipient).map_err(|e| e.to_string())?;
            let satoshis = if let Some(msat) = invoice.amount_milli_satoshis() {
                Some(msat / 1_000)
            } else {
                satoshis
            };
            return Self::lightning(&recipient, satoshis, descr);
        }

        // https://bolt12.org/
        let rgx_bolt12 = r#"^lno1[a-z0-9]{55,150}$"#;
        let re = Regex::new(&rgx_bolt12).map_err(|e| e.to_string())?;
        if re.is_match(recipient) {
            return Err("BOLT12 is not supported yet".to_string());
        }

        // LNURL https://github.com/lnurl/luds
        if recipient.starts_with("LNURL")
            || recipient.starts_with("lightning:LNURL")
            || recipient.starts_with("LIGHTNING:LNURL")
        {
            let recipient = recipient
                .replace("LIGHTNING:", "")
                .replace("lightning:", "");
            let lnu = LnUrl::from_str(&recipient).map_err(|e| e.to_string())?;
            let url = lnu.url.as_str();
            return Self::ln_url(&url, satoshis, descr);
        }

        // lnurlw
        if recipient.starts_with("lnurlw://") || recipient.contains("api.swiss-bitcoin-pay.ch/card")
        {
            let recipient = recipient.replace("lnurlw://", "https://");
            return Self::ln_url(&recipient, satoshis, descr);
        }

        // LNURL https://github.com/lnurl/luds
        if recipient.starts_with("https://") {
            return Self::ln_url(&recipient, satoshis, descr);
        }

        // https://coincharge.io/lnurl/
        let rgx_lnaddr = r#"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,6}$"#;
        let re = Regex::new(&rgx_lnaddr).map_err(|e| e.to_string())?;
        if re.is_match(recipient) {
            let lnaddr = LightningAddress::from_str(&recipient).map_err(|e| e.to_string())?;
            let url = lnaddr.lnurlp_url().as_str().to_string();
            return Self::ln_url(&url, satoshis, descr);
        }

        Err("Unknown input format".to_string())
    }

    fn mainnet(addr: &str, satoshis: Option<u64>, description: String) -> Result<Self, String> {
        let addr = Address::from_str(addr)
            .map_err(|e| format!("Failed to parse address {} : {}", addr, e))?;
        let addr = addr.require_network(Network::Bitcoin).map_err(|e| {
            format!(
                "The onchain address doesn't look like it is for mainnet: {}",
                e
            )
        })?;
        Ok(Self {
            network: InputNetwork::Mainnet(addr),
            satoshis,
            description,
        })
    }

    fn lightning(
        invoice: &str,
        satoshis: Option<u64>,
        description: String,
    ) -> Result<Self, String> {
        let invoice = Bolt11Invoice::from_str(invoice)
            .map_err(|e| format!("Failed to construct the invoice {} : {}", invoice, e))?;
        let satoshis = if let Some(msats) = invoice.amount_milli_satoshis() {
            Some(msats / 1_000)
        } else {
            satoshis
        };
        let description = if let Bolt11InvoiceDescription::Direct(desc) = invoice.description() {
            desc.clone().into_inner().to_string()
        } else {
            description
        };
        Ok(Self {
            network: InputNetwork::Lightning(invoice),
            satoshis,
            description,
        })
    }

    fn ln_url(url: &str, satoshis: Option<u64>, description: String) -> Result<Self, String> {
        let client = Builder::default()
            .build_blocking()
            .map_err(|e| e.to_string())?;
        let resp = client
            .make_request(url)
            .map_err(|e| format!("Failed to query lnurl: {}", e))?;
        match resp {
            LnUrlResponse::LnUrlPayResponse(pay) => {
                let msats = if let Some(sats) = satoshis {
                    if sats * 1_000 < pay.min_sendable || sats * 1_000 > pay.max_sendable {
                        return Err(format!(
                            "payment {} is not between {} and {}",
                            sats * 1_000,
                            pay.min_sendable,
                            pay.max_sendable
                        ));
                    }
                    sats * 1_000
                } else {
                    pay.min_sendable
                };
                let resp = client
                    .get_invoice(&pay, msats, None, Some(&description))
                    .map_err(|e| e.to_string())?;
                let invoice = resp.invoice();
                Self::lightning(&invoice.to_string(), Some(msats / 1_000), description)
            }
            LnUrlResponse::LnUrlWithdrawResponse(lnurlw) => {
                let msats = if let Some(sats) = satoshis {
                    if sats * 1_000 > lnurlw.max_withdrawable {
                        return Err(format!(
                            "payment {} is above {}",
                            sats * 1_000,
                            lnurlw.max_withdrawable,
                        ));
                    }
                    if let Some(minw) = lnurlw.min_withdrawable {
                        if sats * 1_000 < minw {
                            return Err(format!("payment {} is below {}", sats * 1_000, minw,));
                        }
                    }
                    sats * 1_000
                } else {
                    lnurlw.max_withdrawable
                };

                Ok(Self {
                    network: InputNetwork::LnWithdraw(url.to_string()),
                    satoshis: Some(msats / 1_000),
                    description: lnurlw.default_description,
                })
            }
            LnUrlResponse::LnUrlChannelResponse(_) => {
                Err("LNURL withdraw and channel are not implemented yet".to_string())
            }
        }
    }

    /// generate a comma separated value string to pass to the QML GUI
    pub fn gui_csv(&self) -> Result<String, String> {
        let recipient = match &self.network {
            InputNetwork::Mainnet(addr) => addr.to_string(),
            InputNetwork::Lightning(invoice) => invoice.to_string(),
            InputNetwork::LnWithdraw(ss) => ss.to_string(),
            InputNetwork::PrivKey(ss) => ss.to_string(),
        };
        let sats = match self.satoshis {
            Some(s) => format!("{}", s as f32 / 100_000_000.0),
            None => "".to_string(),
        };
        Ok(format!("{};{};{}", recipient, sats, self.description))
    }
}

/// Convert a string with a value in Bitcoin to Satoshis
pub fn parse_satoshis(amount: &str) -> Result<u64, String> {
    if amount.is_empty() {
        return Ok(0);
    }
    let amount = f64::from_str(amount)
        .map_err(|e| format!("Failed to parse the satoshis from {:?} : {}", amount, e))?;
    Ok((amount * 100_000_000.0) as u64)
}

/// Checks if the input looks like a nodeid that could be used to open a channel
pub fn is_node_id(input: &str) -> bool {
    let id_addr = input.split("@").collect::<Vec<_>>();
    if id_addr.len() != 2 {
        return false;
    }
    if PublicKey::from_str(id_addr[0]).is_err() {
        return false;
    }
    if SocketAddress::from_str(id_addr[1]).is_err() {
        return false;
    }

    return true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "Unknown input format")]
    fn test_empty() {
        let inp = "";
        let _resp = InputEval::evaluate(inp, "", "").unwrap();
    }

    #[test]
    #[should_panic(expected = "Unknown input format")]
    fn test_short_numeric() {
        let inp = "1234567890";
        let _resp = InputEval::evaluate(inp, "", "").unwrap();
    }

    #[test]
    fn test_legacy_address() {
        let inp = "3M5f673Ler6iJbatJNvex7EYANRsydSQXE";
        let resp = InputEval::evaluate(inp, "1", "d").unwrap();
        if let InputNetwork::Mainnet(ref addr) = resp.network {
            assert_eq!(inp, addr.to_string());
        } else {
            panic!("not recognized as regular mainnet address");
        }
        assert_eq!(resp.satoshis, Some(100_000_000));
        assert_eq!(resp.description, "d");
        assert_eq!(
            resp.gui_csv().unwrap(),
            "3M5f673Ler6iJbatJNvex7EYANRsydSQXE;1;d"
        );
    }

    #[test]
    fn test_beech_address() {
        let inp = "bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa";
        let resp = InputEval::evaluate(inp, "0.0000001", "").unwrap();
        if let InputNetwork::Mainnet(ref addr) = resp.network {
            assert_eq!(inp, addr.to_string());
        } else {
            panic!("not recognized as regular mainnet address");
        }
        assert_eq!(resp.satoshis, Some(10));
        assert_eq!(resp.description, "");
        assert_eq!(
            resp.gui_csv().unwrap(),
            "bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa;0.0000001;"
        );
    }

    #[test]
    fn test_uri_amount() {
        let inp = "bitcoin:bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa?amount=100";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Mainnet(ref addr) = resp.network {
            assert_eq!(
                "bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa",
                addr.to_string()
            );
        } else {
            panic!("not recognized as regular mainnet address");
        }
        assert_eq!(resp.satoshis, Some(10_000_000_000));
        assert_eq!(resp.description, "");
        assert_eq!(
            resp.gui_csv().unwrap(),
            "bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa;100;"
        );
    }

    #[test]
    fn test_uri_label_amount() {
        let inp = "bitcoin:bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa?label=test&amount=100";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Mainnet(ref addr) = resp.network {
            assert_eq!(
                "bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa",
                addr.to_string()
            );
        } else {
            panic!("not recognized as regular mainnet address");
        }
        assert_eq!(resp.satoshis, Some(10_000_000_000));
        assert_eq!(resp.description, "test");
        assert_eq!(
            resp.gui_csv().unwrap(),
            "bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa;100;test"
        );
    }

    #[test]
    fn test_priv_key() {
        let inp = "KxWvpvpY9C5weJGWpUMQqHt88Xktt7nZDZPHbpJjEuUaDgeMHJuw";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::PrivKey(ref key) = resp.network {
            assert_eq!(
                "KxWvpvpY9C5weJGWpUMQqHt88Xktt7nZDZPHbpJjEuUaDgeMHJuw",
                key.to_string()
            );
        } else {
            panic!("not recognized as private key address");
        }
    }

    #[test]
    fn test_xprv() {
        let inp = "xprv9z1Nt86QQeoGXTjrvKgbFT924JeV1qmo2QV6m8YYTWkaVVWNc3nmeTTKsoq2PKVMfQLUKchQbazkT5FqLo4BUC2P2rVFmDnE46QBNjiAsLP";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::PrivKey(ref key) = resp.network {
            assert_eq!(
                "xprv9z1Nt86QQeoGXTjrvKgbFT924JeV1qmo2QV6m8YYTWkaVVWNc3nmeTTKsoq2PKVMfQLUKchQbazkT5FqLo4BUC2P2rVFmDnE46QBNjiAsLP",
                key.to_string()
            );
        } else {
            panic!("not recognized as private key address");
        }
    }

    #[test]
    fn test_zprv() {
        let inp = "zprvAZLoT7yPmyP5qVdL4pdmE2tFeGx1BWSbMwkMwXwPE6z3E1qrYCY4HsZPkRXHnziAB8uSpMzjiDM3jbkQsnWWDAkVtsMo1L9sES5xeJMq3YV";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::PrivKey(ref key) = resp.network {
            assert_eq!(
                "xprv9ugGqndZUcJ88uF6Q74WorhFJLf7JGTbXihvNk9cU6EH7pDQ2tCw3kF7i1c7oBQKMrfqKQocntdwy2XHSPgUchPJABxwqWWtgyxfsAdXcKZ",
                key.to_string()
            );
        } else {
            panic!("not recognized as private key address");
        }
    }

    #[test]
    fn test_desc() {
        let inp = "pkh(xprv9z1Nt86QQeoGXTjrvKgbFT924JeV1qmo2QV6m8YYTWkaVVWNc3nmeTTKsoq2PKVMfQLUKchQbazkT5FqLo4BUC2P2rVFmDnE46QBNjiAsLP)";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::PrivKey(ref desc) = resp.network {
            assert_eq!(inp.to_string() + "#smfvl5ay", desc.to_string());
        } else {
            panic!("not recognized as miniscript descriptor");
        }
    }

    #[test]
    #[should_panic(expected = "sanity check")]
    fn test_desc_invalid() {
        let inp = "pkh(pkh(xprv9z1Nt86QQeoGXTjrvKgbFT924JeV1qmE46QBNjiAsLP))";
        InputEval::evaluate(inp, "", "").unwrap();
    }

    #[test]
    fn test_bolt11_short() {
        let inp = "lnbc1pjzg3y4sp5t5pqc4w2re6duurq9smwhd78688rwmg2hwxhypxn0vqgu9vgjxnspp5z7p6kn5fpnr8zefvhdw90gascnae5a9s2flrwjp45a6tf53gwrrqdq9u2d2zxqr3jscqpjrzjqvp62xyytkuen9rc8asxue3fuuzultc89ewwnfxch70zf80yl0gpjzxypyqqxhqqqqqqqqqqqqqqqzqq9q9qx3qysgqcnwt6hdzlz3r5k3vqlwcyjrgmyyxrcq7rv304w32q8s6zqe4r7vjvvqxq8rk0g8j9udljtr9dw908ye7608z945gpa3h0avudrqtcpsp7zd4mp";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(ref invoice) = resp.network {
            assert_eq!(inp, invoice.to_string());
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, None);
        assert_eq!(resp.description, "⚡");
        assert_eq!(resp.gui_csv().unwrap(), "lnbc1pjzg3y4sp5t5pqc4w2re6duurq9smwhd78688rwmg2hwxhypxn0vqgu9vgjxnspp5z7p6kn5fpnr8zefvhdw90gascnae5a9s2flrwjp45a6tf53gwrrqdq9u2d2zxqr3jscqpjrzjqvp62xyytkuen9rc8asxue3fuuzultc89ewwnfxch70zf80yl0gpjzxypyqqxhqqqqqqqqqqqqqqqzqq9q9qx3qysgqcnwt6hdzlz3r5k3vqlwcyjrgmyyxrcq7rv304w32q8s6zqe4r7vjvvqxq8rk0g8j9udljtr9dw908ye7608z945gpa3h0avudrqtcpsp7zd4mp;0;⚡");
    }

    #[test]
    fn test_bolt11_long() {
        let inp = "lnbc3518772650p1pjzg3x2sp59yemkg0cfmsxmugaesm304av4cx4mrp8q7zl65sses7dya7v725spp52ezaxjly2cvdvzlnyakgrq8v3gpnc58rtjepwch74gwgx05snvvqd2qw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqw3jhxapqxqr3jscqpjrzjq032f2wvt88a4lpgxa3nlxuuzd6xmm5azq8np92afzqnsfvv09qk6za0p5qqjdgqqqqqqqqqqqqqqqqqyu9qx3qysgq8v099gx9mlh9fvs3l0n0qlgka7kt0en8kca659maxy3kuww9y4l3utddc3yrx24hs2jwfyx8h0w2t6xltetqzd4a0mlpqwjz2mp5stsqvat45l";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(ref invoice) = resp.network {
            assert_eq!(inp, invoice.to_string());
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, Some(351877));
        let desc = "test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test test ";
        assert_eq!(resp.description, desc);
        let exp = format!("{};{};{}", inp, 0.00351877, desc);
        assert_eq!(resp.gui_csv().unwrap(), exp);
    }

    #[test]
    fn test_bolt11_timecatcher() {
        let inp = "lnbc21u1pjgj7azpp5w9kue4qeexcjv8j7jjpvxhfsut25d07e6lxz9xq5x3ftdjrv8spqdpydpv5z6zndf44jm6zg9xnsarz2dmkww2p2dgqcqzrrxqyp2xqsp5mf6qel6ymkeuue833vnscdwdkyrl5gef225z9f776gn0pgmehsqq9qyyssqfn28qncnutmp9y3wvqxze4xtewqkxv4jtqvndhk4hqwhqr4fl5j80zy6jcwvud85r0v0vpdwqd0d93n53jcnv43ee3dxjww3tcvgc9sph6jczf";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(*"lnbc", invoice.to_string()[..4]);
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, Some(2100));
        assert_eq!(resp.description, "hYAhSjkYoBAM8tbSwg9ASP");
    }

    #[test]
    fn test_bolt11_ulrichard() {
        let inp = "LIGHTNING:LNBC33327780P1PJF2SS6SP50PNHS5H63S0XVJLAJUJM68M6JYQQDLHW0FA4A2HCUTAVRR2N7U4SPP5WVLRGLGX53R5R2FV8DAFK88Q6WXKNN4PPC7S0QCCHHYNMMXDXM5QDQ9U2D2ZXQR3JSCQPJRZJQTZXVFSUXE4L92PF97TT4RCGPY2XALKMLWEXH899WQXF83L8NWV4XZMCSQQQTLQQQYQQQQLGQQQQQWGQVS9QX3QYSGQQZGVXP2RHFQ32DC3RQH2AE2QSMLZJGE9YC2JWQWZ3MDPZFULHPXPXWEVW0QAZN4MDF8593UZFXARP3CTMTGE6W6TEENQW5R7TSE5JHCQK7ZNKT";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(*"lnbc", invoice.to_string()[..4]);
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, Some(3332));
        assert_eq!(resp.description, "⚡");
    }

    #[test]
    #[should_panic(expected = "BOLT12 is not supported yet")]
    fn test_bolt12_short() {
        let inp = "lno1pgqpvggr53478rgx3s4uttelcy76ssrepm2kg0ead5n7tc6dvlkj4mqkeens";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(inp, invoice.to_string());
        } else {
            panic!("not recognized as lightning invoice");
        }
    }

    #[test]
    #[should_panic(expected = "BOLT12 is not supported yet")]
    fn test_bolt12_long() {
        let inp = "lno1pqpzwrc2936x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5yp6x2um5zcss8frtuwxsdrptckhnlsfa4pq8jrk4vsln6mf8uh356eld9tkpdnn8";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(inp, invoice.to_string());
        } else {
            panic!("not recognized as lightning invoice");
        }
    }

    #[test]
    fn test_lnurl_https() {
        let inp = "https://opreturnbot.com/.well-known/lnurlp/ben";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(*"lnbc", invoice.to_string()[..4]);
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, Some(1));
        assert_eq!(resp.description, "");
    }

    #[test]
    fn test_lnurl() {
        let inp = "LNURL1DP68GURN8GHJ7MR9VAJKUEPWD3HXY6T5WVHXXMMD9AKXUATJD3JX2ANFVDJJ7CTSDYHHVV30D3H82UNV9AF5ZMJEWFV82CJ3D4R8G42STP2N272V23K550MSD9HR6VFJYESK6MM4DE6R6VPWX5NXGATJV96XJMMW85CNQVPSV48PVT";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(*"lnbc", invoice.to_string()[..4]);
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert!(
            resp.satoshis.unwrap() > 1_000 && resp.satoshis.unwrap() < 3_000,
            "satoshis: {}",
            resp.satoshis.unwrap()
        );
        assert_eq!(resp.description, "");
    }

    #[test]
    fn test_lnurl_prefix() {
        let inp = "lightning:LNURL1DP68GURN8GHJ7MR9VAJKUEPWD3HXY6T5WVHXXMMD9AKXUATJD3CZ7CTSDYHHVVF0D3H82UNV9UUNGWPCMUCDQF";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(*"lnbc", invoice.to_string()[..4]);
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, Some(2100));
        assert_eq!(resp.description, "");
    }

    #[test]
    fn test_lightning_address_ben() {
        let inp = "ben@opreturnbot.com";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(*"lnbc", invoice.to_string()[..4]);
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, Some(1));
        assert_eq!(resp.description, "");
    }

    // I didn't want to dox my real card id, as otherwise anybody could block it.
    #[test]
    #[should_panic(expected = "HttpResponse(500)")]
    fn test_lightning_address() {
        let inp = "2iwc-vo3m-lsks-zt0z@swiss-bitcoin-pay.ch";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::Lightning(invoice) = resp.network {
            assert_eq!(*"lnbc", invoice.to_string()[..4]);
        } else {
            panic!("not recognized as lightning invoice");
        }
        assert_eq!(resp.satoshis, None);
        assert_eq!(resp.description, "");
    }

    #[test]
    fn test_nodeid_ulrichard() {
        let inp = crate::constants::LN_ULR;
        assert!(is_node_id(inp));
    }

    #[test]
    fn test_nodeid_tor() {
        let inp = "02fb0ba685e8f5be6eb39e5f1f2481b16673aa1019852a727b3140f5b0716cf48a@rquqr26p26lwgnanyjrr4mo33ri76y3a55xge57w52n5qlwp6sixzhad.onion:9735";
        assert!(is_node_id(inp));
    }

    #[test]
    fn test_nodeid_localhost() {
        let inp =
            "02fb0ba685e8f5be6eb39e5f1f2481b16673aa1019852a727b3140f5b0716cf48a@127.0.0.1:9735";
        assert!(is_node_id(inp));
    }

    #[test]
    fn test_nodeid_invalid_pubkey() {
        let inp = "02fb0ba85e8f5beeb39e5f1f2481b1673aa1019852727b3140f5b0716cf48a@127.0.0.1:9735";
        assert!(!is_node_id(inp));
    }

    #[test]
    fn test_nodeid_empty() {
        let inp = "";
        assert!(!is_node_id(inp));
    }

    // I didn't want to dox my real card id, as otherwise anybody could withdraw from it.
    #[test]
    fn test_lnurlw() {
        let inp = "lnurlw://api.swiss-bitcoin-pay.ch/card/AbCdEfGhIjKlMnOpQr?p=123456789ABCDEF&c=123456789ABCDEF";
        let resp = InputEval::evaluate(inp, "", "").unwrap();
        if let InputNetwork::LnWithdraw(invoice) = resp.network {
            assert_eq!(inp.replace("lnurlw://", "https://"), invoice);
        } else {
            panic!("not recognized as lightning withdrawal");
        }
        assert_eq!(resp.satoshis, Some(21000000000));
        assert_eq!(resp.description, "🇨🇭 Swiss Bitcoin Pay Card");
    }
}
