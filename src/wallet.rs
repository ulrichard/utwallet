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

use crate::constants::{ESPLORA_SERVERS, LN_ULR, RAPID_GOSSIP_SYNC_URL};
use crate::input_eval::PrivateKeys;

use bdk_esplora::{esplora_client, EsploraAsyncExt};
use ldk_node::bip39::Mnemonic;
use ldk_node::bitcoin::{secp256k1::PublicKey, Address, Network, Txid};
use ldk_node::lightning::offers::offer::{Amount, Offer};
use ldk_node::lightning_invoice::Bolt11Invoice;
use ldk_node::{Builder, /*Event,*/ Node};
use lnurl::{api::LnUrlResponse, Builder as LnUrlBuilder};
use rand_core::{OsRng, RngCore};
use std::{
    fs,
    fs::create_dir_all,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Mutex,
};

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
            .onchain_payment()
            .send_to_address(&recipient, amount)
            .map_err(|e| format!("Failed to send on-chain: {:?}", e))?;

        println!("on-chain payment sent: {}", txid);

        Ok(txid)
    }

    pub fn channel_open(amount: u64, node_id: Option<&str>) -> Result<(), String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let id_addr = node_id.unwrap_or(LN_ULR).split("@").collect::<Vec<_>>();
        assert_eq!(id_addr.len(), 2);
        let node_id = PublicKey::from_str(id_addr[0]).unwrap();
        let node_addr = id_addr[1].parse().unwrap();
        node.open_channel(node_id, node_addr, amount, None, None)
            .map_err(|e| format!("Failed to open a channel: {:?}", e))?;

        Ok(())
    }

    pub fn channel_close() -> Result<(), String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let channels = node.list_channels();
        for c in channels {
            node.close_channel(&c.user_channel_id, c.counterparty_node_id)
                .map_err(|e| format!("Failed to close a channel: {:?}", e))?;
        }

        Ok(())
    }

    pub fn create_invoice(amount: Option<u64>, desc: &str) -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let expiry_secs = 60 * 15;
        let invoice = if let Some(amount) = amount {
            node.bolt11_payment()
                .receive(amount * 1_000, desc, expiry_secs)
        } else {
            node.bolt11_payment()
                .receive_variable_amount(desc, expiry_secs)
        }
        .map_err(|e| format!("Failed to create an invoice: {:?}", e))?;

        Ok(invoice.to_string())
    }

    pub fn pay_invoice(invoice: &Bolt11Invoice, amount: Option<u64>) -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let ph = match (invoice.amount_milli_satoshis(), amount) {
            (Some(_amount), None) => node
                .bolt11_payment()
                .send(invoice, None)
                .map_err(|e| format!("Unable to pay the invoice: {:?}", e)),
            (Some(amount_inv), Some(amount_field)) => {
                if (amount_inv as i64 - amount_field as i64 * 1_000).abs() > 1_000_000 {
                    Err(format!(
                        "amount of the invoice {} and in the field {} don't match",
                        amount_inv,
                        amount_field * 1_000
                    ))
                } else {
                    node.bolt11_payment()
                        .send(invoice, None)
                        .map_err(|e| format!("Unable to pay the invoice: {:?}", e))
                }
            }
            (None, Some(amount)) => node
                .bolt11_payment()
                .send_using_amount(invoice, amount * 1_000, None)
                .map_err(|e| format!("Unable to pay the invoice with {} sats: {:?}", amount, e)),
            (None, None) => Err("No amount to pay the invoice!".to_string()),
        }?;

        let ph = format!("{:?}", ph);
        println!("lightning payment sent: {}", ph);

        Ok(ph)
    }

    pub fn pay_offer(offer: &Offer, amount: Option<u64>, desc: &str) -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let msats_min = match offer.amount() {
            Some(Amount::Bitcoin { amount_msats }) => Some(amount_msats),
            Some(Amount::Currency { .. }) => {
                return Err("For BOLT12 we only support BTC at the moment".to_string());
            }
            None => None,
        };

        let desc = if desc.is_empty() {
            None
        } else {
            Some(desc.to_string())
        };

        let ph = match (msats_min, amount) {
            (Some(_amount), None) => node
                .bolt12_payment()
                .send(offer, None, desc)
                .map_err(|e| format!("Unable to pay the invoice: {:?}", e)),
            (Some(amount_inv), Some(amount_field)) => {
                if (amount_inv as i64 - amount_field as i64 * 1_000).abs() > 1_000_000 {
                    Err(format!(
                        "amount of the invoice {} and in the field {} don't match",
                        amount_inv,
                        amount_field * 1_000
                    ))
                } else {
                    node.bolt12_payment()
                        .send(offer, None, desc)
                        .map_err(|e| format!("Unable to pay the invoice: {:?}", e))
                }
            }
            (None, Some(amount)) => node
                .bolt12_payment()
                .send_using_amount(offer, amount * 1_000, None, desc)
                .map_err(|e| format!("Unable to pay the invoice with {} sats: {:?}", amount, e)),
            (None, None) => Err("No amount to pay the invoice!".to_string()),
        }?;

        let ph = format!("{:?}", ph);
        println!("lightning payment sent: {}", ph);

        Ok(ph)
    }

    pub fn withdraw(url: &str, satoshis: Option<u64>) -> Result<String, String> {
        let url = url.replace("lnurlw://", "https://");
        let client = LnUrlBuilder::default()
            .build_blocking()
            .map_err(|e| e.to_string())?;
        let resp = client
            .make_request(&url)
            .map_err(|e| format!("Failed to query lnurl: {}", e))?;
        if let LnUrlResponse::LnUrlWithdrawResponse(lnurlw) = resp {
            println!("{:?}", lnurlw);
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
            let invoice = Self::create_invoice(Some(msats / 1_000), &lnurlw.default_description)?;
            let url = format!(
                "{}&num_satoshis={}&k1={}&pr={}",
                lnurlw.callback,
                msats / 1_000,
                lnurlw.k1,
                invoice
            );
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create a tokio runtime: {}", e))?;

            let resp = rt
                .block_on(reqwest::get(url))
                .map_err(|e| format!("failed to request lnurl payment: {}", e))?;
            let body = rt
                .block_on(resp.text())
                .map_err(|e| format!("failed to receive lnurl payment response: {}", e))?;
            println!("lnurl response: {}", body); // k1 is required?

            Ok(body)
        } else {
            Err("invalid response to lnurl".to_string())
        }
    }

    pub fn sweep(privkeys: &PrivateKeys) -> Result<String, String> {
        let sw = crate::sweeper::Sweeper {
            esplora_url: find_working_esplora_server()?,
            network: Network::Bitcoin,
        };
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create a tokio runtime: {}", e))?;

        rt.block_on(sw.sweep(privkeys, &Self::get_address()?))
    }

    pub fn handle_ldk_event() -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        if let Some(event) = node.next_event() {
            //match event {
            //    Event::PaymentSuccessful => println!("payment "),
            //}
            let descr = format!("{:?}", event);
            println!("ldk event: {}", descr);

            node.event_handled();

            Ok(descr)
        } else {
            Ok("".to_string())
        }
    }

    pub fn get_address() -> Result<Address, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        node.onchain_payment()
            .new_address()
            .map_err(|e| format!("Unable to get an address: {:?}", e))
    }

    pub fn get_balance() -> Result<(f32, f32), String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        println!("getting balances");
        let ocbal = node.list_balances().spendable_onchain_balance_sats;

        let lnbal = node.list_balances().total_lightning_balance_sats;

        Ok((ocbal as f32 / 100_000_000.0, lnbal as f32 / 100_000_000.0))
    }

    pub fn get_channel_status() -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let mut channels = node.list_channels();
        if let Some(channel) = channels.pop() {
            let mut our_share = channel.outbound_capacity_msat as f32
                / (channel.outbound_capacity_msat as f32 + channel.inbound_capacity_msat as f32);
            if !channel.is_usable {
                our_share = -our_share;
            }
            println!("channel status: {}", our_share);
            Ok(format!("{}", our_share))
        } else {
            Ok("".to_string())
        }
    }

    fn create_node() -> Result<Node, String> {
        let app_data_path =
            unsafe { QStandardPaths::writable_location(StandardLocation::AppDataLocation) };
        let mnemonic_file = PathBuf::from(app_data_path.to_std_string()).join("mnemonic.txt");
        let mnemonic = read_or_generate_mnemonic(&mnemonic_file)?;
        let ldk_dir = PathBuf::from(app_data_path.to_std_string()).join("ldk");

        println!("building the ldk-node");
        let mut builder = Builder::new();
        builder.set_network(Network::Bitcoin);
        builder.set_chain_source_esplora(find_working_esplora_server()?, None);
        builder.set_entropy_bip39_mnemonic(mnemonic, None);
        builder.set_storage_dir_path(ldk_dir.to_str().unwrap().to_string());
        builder.set_gossip_source_rgs(RAPID_GOSSIP_SYNC_URL.to_string());
        let node = builder
            .build()
            .map_err(|e| format!("Failed to build ldk-node: {:?}", e))?;

        println!("starting the ldk-node");
        node.start().unwrap();
        println!("ldk-node started");

        Ok(node)
    }
}

fn find_working_esplora_server() -> Result<String, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create a tokio runtime: {}", e))?;
    for srv in ESPLORA_SERVERS {
        if let Ok(client) = esplora_client::Builder::new(srv).build_async() {
            if rt.block_on(client.get_height()).is_ok() {
                return Ok(srv.to_string());
            }
        }
    }

    Err("No working esplora server found".to_string())
}

fn read_or_generate_mnemonic(mnemonic_file: &Path) -> Result<Mnemonic, String> {
    let mnemonic_words = if mnemonic_file.exists() {
        fs::read_to_string(&mnemonic_file).map_err(|e| {
            format!(
                "Failed to read the mnemonic file {:?}: {}",
                mnemonic_file, e
            )
        })?
    } else {
        // Generate fresh mnemonic
        let mut entropy = [0u8; 16];
        OsRng.fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| format!("Failed to generate mnemonic: {:?}", e))?;
        mnemonic.to_string()
    };

    let mnemonic =
        Mnemonic::parse(&mnemonic_words).map_err(|e| format!("Failed to parse mnemonic: {}", e))?;

    // persist the mnemonic
    let prefix = mnemonic_file
        .parent()
        .ok_or("Failed to get parent path".to_string())?;
    create_dir_all(prefix).map_err(|e| format!("Failed to create directory: {}", e))?;
    let mut output = File::create(mnemonic_file)
        .map_err(|e| format!("Failed to create mnemonic file: {}", e))?;
    write!(output, "{}", mnemonic_words)
        .map_err(|e| format!("Failed to write mnemonic file: {}", e))?;

    Ok(mnemonic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use electrsd::{
        bitcoind::{self, bitcoincore_rpc::RpcApi, BitcoinD},
        electrum_client::ElectrumApi,
        ElectrsD,
    };
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
        thread::sleep,
        time::Duration,
    };

    struct RegTestEnv {
        /// Instance of the bitcoin core daemon
        bitcoind: BitcoinD,
        /// Instance of the electrs electrum server
        electrsd: ElectrsD,
        /// ldk-node instances
        ldk_nodes: Vec<Node>,
    }

    impl RegTestEnv {
        /// set up local bitcoind and electrs instances in regtest mode, and connect a number of ldk-nodes to it.
        pub fn new(num_nodes: u8) -> Self {
            let bitcoind_exe =
                bitcoind::downloaded_exe_path().expect("bitcoind version feature must be enabled");
            let mut btc_conf = bitcoind::Conf::default();
            btc_conf.network = "regtest";
            let bitcoind = BitcoinD::with_conf(bitcoind_exe, &btc_conf).unwrap();
            let electrs_exe =
                electrsd::downloaded_exe_path().expect("electrs version feature must be enabled");
            let mut elect_conf = electrsd::Conf::default();
            elect_conf.http_enabled = true;
            elect_conf.network = "regtest";
            let electrsd = ElectrsD::with_conf(electrs_exe, &bitcoind, &elect_conf).unwrap();

            // start the ldk-nodes
            let ldk_nodes = (0..num_nodes)
                .map(|i| {
                    let listen = SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        Self::get_available_port(),
                    );
                    let mut builder = Builder::new();
                    builder.set_network(Network::Regtest);
                    builder.set_chain_source_esplora(electrsd.esplora_url.clone().unwrap(), None);
                    let node = builder.build().unwrap();
                    node.start().unwrap();
                    println!("{:?} starting at {:?}", i, listen);
                    node
                })
                .collect::<Vec<_>>();

            RegTestEnv {
                bitcoind,
                electrsd,
                ldk_nodes,
            }
        }

        /// fund on-chain wallets
        pub fn fund_on_chain_wallets(&self, num_blocks: &[usize], retries: u8) {
            // generate coins to the node addresses
            num_blocks
                .iter()
                .zip(self.ldk_nodes.iter())
                .enumerate()
                .for_each(|(i, (num_blocks, node))| {
                    let addr = node.onchain_payment().new_address().unwrap();
                    println!("{} Generating {} blocks to {}", i, num_blocks, addr);
                    self.generate_to_address(*num_blocks, &addr);
                });

            // generate another 100 blocks to make the funds available
            let addr = self
                .ldk_nodes
                .last()
                .unwrap()
                .onchain_payment()
                .new_address()
                .unwrap();
            println!("Generating {} blocks to {}", 100, addr);
            self.generate_to_address(100, &addr);

            num_blocks
                .iter()
                .zip(self.ldk_nodes.iter())
                .enumerate()
                .for_each(|(i, (num_blocks, node))| {
                    // synchronizing the nodes
                    let _success = (0..retries)
                        .map(|i| (i, node.sync_wallets()))
                        .find(|(i, r)| {
                            if let Err(e) = r {
                                println!("{:?} sync : {:?}", i, e);
                                sleep(Duration::from_secs(1));
                            }
                            r.is_ok()
                        });
                    // assert!(success.is_some());

                    // checking the on-chain balance of the nodes
                    (0..5).find(|_| {
                        let bal = node.list_balances().spendable_onchain_balance_sats;
                        if bal == 0 {
                            sleep(Duration::from_secs(1));
                        }
                        bal > 0
                    });
                    let bal = node.list_balances().spendable_onchain_balance_sats;
                    println!("{:?}", bal);
                    let expected = *num_blocks as u64 * 5_000_000_000;
                    assert_eq!(bal, expected, "node {} has a balance of {}", i, bal);
                });
            assert_eq!(self.get_height(), num_blocks.iter().sum::<usize>() + 101);
        }

        /// open channels
        pub fn open_channels(&self, channels: &[(usize, usize, u64)]) {
            channels.iter().for_each(|(n1, n2, sats)| {
                let n1 = &self.ldk_nodes[*n1];
                let n2 = &self.ldk_nodes[*n2];
                n1.open_channel(
                    n2.node_id(),
                    n2.listening_addresses().unwrap()[0].clone(),
                    sats * 1_000,
                    None,
                    None,
                )
                .unwrap();

                //sleep(Duration::from_secs(1));
                //let event = node.next_event();
                //println!("ldk event: {:?}", event);
                //node.event_handled();

                let addr1 = n1.onchain_payment().new_address().unwrap();
                self.generate_to_address(3, &addr1);
                let channels = n1.list_channels();
                let chan = channels.last().unwrap();
                println!("new channel: {:?}", chan);
            });
        }

        fn get_height(&self) -> usize {
            self.electrsd
                .client
                .block_headers_subscribe()
                .unwrap()
                .height
        }

        pub fn generate_to_address(&self, blocks: usize, address: &Address) {
            let old_height = self.get_height();

            self.bitcoind
                .client
                .generate_to_address(blocks as u64, address)
                .unwrap();

            let new_height = loop {
                sleep(Duration::from_secs(1));
                let new_height = self.get_height();
                if new_height >= old_height + blocks {
                    break new_height;
                }
            };

            assert_eq!(new_height, old_height + blocks);
        }

        /// Returns a non-used local port if available.
        /// Note there is a race condition during the time the method check availability and the caller
        fn get_available_port() -> u16 {
            // using 0 as port let the system assign a port available
            let t = TcpListener::bind(("127.0.0.1", 0)).unwrap(); // 0 means the OS choose a free port
            t.local_addr().map(|s| s.port()).unwrap()
        }
    }

    #[test]
    /// Open only one channel between two nodes
    ///      0 --------> 1
    fn test_regtest_two_nodes() {
        let regtest_env = RegTestEnv::new(2);
        regtest_env.fund_on_chain_wallets(&[1, 1], 10);
        regtest_env.open_channels(&[(0, 1, 1_000_000)]);
    }

    #[test]
    /// Open channels for the following graph:
    ///      0 --------> 1
    ///        \         / \
    ///         \       /    >  4 ---> 5
    ///          \     /      >
    ///           \   <       /
    ///            > 2 ---> 3
    fn test_regtest_six_nodes() {
        let regtest_env = RegTestEnv::new(6);
        regtest_env.fund_on_chain_wallets(&[2, 2, 2, 2, 2, 2], 10);
        regtest_env.open_channels(&[
            (0, 1, 1_000_000_000),
            (0, 2, 1_000_000_000),
            (1, 2, 9_000_000_000),
            (2, 3, 9_000_000_000),
            (1, 4, 1_000_000_000),
            (3, 4, 1_000_000_000),
            (4, 5, 2_000_000_000),
        ]);
    }

    #[test]
    fn test_regtest_sweep() {
        let regtest_env = RegTestEnv::new(1);
        regtest_env.fund_on_chain_wallets(&[1], 10);
    }
}
