#[cfg(test)]
mod tests {
    use super::*;
    use ldk_node::bitcoin::secp256k1::PublicKey;
    use ldk_node::lightning_invoice::Invoice;
    use ldk_node::Builder;
    use std::str::FromStr;

    #[test]
    fn test_init() {
        let node = Builder::new()
            .set_network("testnet")
            .set_esplora_server_url("https://blockstream.info/testnet/api".to_string())
            .build();

        node.start().unwrap();

        let _funding_address = node.new_funding_address();

        // .. fund address ..

        node.sync_wallets().unwrap();

        let node_id = PublicKey::from_str("NODE_ID").unwrap();
        let node_addr = "IP_ADDR:PORT".parse().unwrap();
        node.connect_open_channel(node_id, node_addr, 10000, None, false)
            .unwrap();

        let invoice = Invoice::from_str("INVOICE_STR").unwrap();
        node.send_payment(&invoice).unwrap();

        node.stop().unwrap();
    }
}
