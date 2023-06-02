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
use ldk_node::bitcoin::{secp256k1::PublicKey, Address, Network, Txid};
use ldk_node::io::FilesystemStore;
use ldk_node::lightning_invoice::Invoice;
use ldk_node::{Builder, /*Event,*/ Node};
use rand_core::{OsRng, RngCore};
use std::{
    fs,
    fs::create_dir_all,
    fs::File,
    io::Write,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

pub struct BdkWallet {}

static UTNODE: Mutex<Option<Arc<Node<FilesystemStore>>>> = Mutex::new(None);

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

        println!("on-chain payment sent: {}", txid);

        Ok(txid)
    }

    pub fn channel_open(amount: u64) -> Result<(), String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let id_addr = LN_ULR.split("@").collect::<Vec<_>>();
        assert_eq!(id_addr.len(), 2);
        let node_id = PublicKey::from_str(id_addr[0]).unwrap();
        let node_addr = id_addr[1].parse().unwrap();
        node.connect_open_channel(node_id, node_addr, amount, None, false)
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
            node.close_channel(&c.channel_id, c.counterparty_node_id)
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

        let ph = format!("{:?}", ph);
        println!("lightning payment sent: {}", ph);

        Ok(ph)
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

        node.new_funding_address()
            .map_err(|e| format!("Unable to get an address: {:?}", e))
    }

    pub fn get_balance() -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        let bal = node
            .onchain_balance()
            .map_err(|e| format!("Unable to get on-chain balance: {:?}", e))?;

        println!("on-chain balance: {:?}", bal);

        let channels = node.list_channels();
        println!("channels: {:?}", channels);
        let lnbal = channels
            .iter()
            .fold(0, |sum, c| sum + c.outbound_capacity_msat)
            / 1_000;

        /*
        for c in channels {
            let mut config = c.config.unwrap().clone();
            config.max_dust_htlc_exposure_msat = 27_000_000;
            config.forwarding_fee_base_msat = 0;
            // config.forwarding_fee_proportional_millionths = 0;
            node.update_channel(&c.counterparty_node_id, &[c.channel_id], &config)
                .map_err(|e| format!("Unable to update channel config: {:?}", e))?;
        }
        */

        Ok(format!(
            "Balance: {} (+{}) + {} BTC",
            bal.confirmed as f32 / 100_000_000.0,
            (bal.immature + bal.trusted_pending + bal.untrusted_pending) as f32 / 100_000_000.0,
            lnbal as f32 / 100_000_000.0
        ))
    }

    pub fn get_channel_status() -> Result<String, String> {
        let node_m = UTNODE
            .lock()
            .map_err(|e| format!("Unable to get the mutex for the wallet: {:?}", e))?;
        let node = node_m.as_ref().ok_or("The wallet was not initialized")?;

        /*
        let id_addr = crate::constants::LN_ULR.split("@").collect::<Vec<_>>();
        assert_eq!(id_addr.len(), 2);
        let node_id = PublicKey::from_str(id_addr[0]).unwrap();
        let node_addr = id_addr[1].parse().unwrap();
        node.connect(node_id, node_addr, true).unwrap();
        */

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

    fn create_node() -> Result<Arc<Node<FilesystemStore>>, String> {
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

        let builder = Builder::new();
        builder.set_network(Network::Bitcoin);
        builder.set_esplora_server(ESPLORA_SERVERS[1].to_string());
        builder.set_entropy_bip39_mnemonic(mnemonic, None);
        builder.set_storage_dir_path(ldk_dir.to_str().unwrap().to_string());
        builder.set_gossip_source_rgs("https://rapidsync.lightningdevkit.org/snapshot".to_string());
        let node = builder.build();
        node.start().unwrap();

        Ok(node)
    }
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
        ldk_nodes: Vec<Arc<Node<FilesystemStore>>>,
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
                    let builder = Builder::new();
                    builder.set_network(Network::Regtest);
                    builder.set_esplora_server(electrsd.esplora_url.clone().unwrap());
                    let node = builder.build();
                    node.start().unwrap();
                    println!("node {:?} starting at {:?}", i, listen);
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
                .for_each(|(num_blocks, node)| {
                    let addr = node.new_funding_address().unwrap();
                    println!("Generating {} blocks to {}", num_blocks, addr);
                    self.generate_to_address(*num_blocks, &addr);
                });

            // generate another 100 blocks to make the funds available
            let addr = self
                .ldk_nodes
                .last()
                .unwrap()
                .new_funding_address()
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
                                println!("{:?} : {:?}", i, e);
                                sleep(Duration::from_secs(1));
                            }
                            r.is_ok()
                        });
                    //assert!(success.is_some());

                    // checking the on-chain balance of the nodes
                    (0..5).find(|_| {
                        let bal = node.onchain_balance().unwrap().confirmed;
                        if bal == 0 {
                            sleep(Duration::from_secs(1));
                        }
                        bal > 0
                    });
                    let bal = node.onchain_balance().unwrap();
                    println!("{:?}", bal);
                    let expected = *num_blocks as u64 * 5_000_000_000;
                    assert_eq!(
                        bal.confirmed, expected,
                        "node {} has a confirmed balance of {}",
                        i, bal
                    );
                });
            assert_eq!(self.get_height(), num_blocks.iter().sum::<usize>() + 100);
        }

        /// open channels
        pub fn open_channels(&self, channels: &[(usize, usize, u64)]) {
            channels.iter().for_each(|(n1, n2, sats)| {
                let n1 = &self.ldk_nodes[*n1];
                let n2 = &self.ldk_nodes[*n2];
                n1.connect_open_channel(
                    n2.node_id(),
                    n2.listening_address().unwrap().clone(),
                    sats * 1_000,
                    None,
                    true,
                )
                .unwrap();

                //sleep(Duration::from_secs(1));
                //let event = node.next_event();
                //println!("ldk event: {:?}", event);
                //node.event_handled();

                self.generate_to_address(3, &n1.new_funding_address().unwrap());
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
        regtest_env.fund_on_chain_wallets(&[1, 1], 5);
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
        regtest_env.fund_on_chain_wallets(&[2, 2, 2, 2, 2, 2], 5);
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
}
