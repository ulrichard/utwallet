#[cfg(test)]
mod tests {
    use super::*;
    use ldk_node::bip39::Mnemonic;
    use ldk_node::bitcoin::secp256k1::PublicKey;
    use ldk_node::lightning_invoice::Invoice;
    use ldk_node::Builder;
    use std::str::FromStr;

    #[test]
    fn test_init() {
        let mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        let node = Builder::new()
            .set_network("bitcoin")
            .set_esplora_server_url(crate::constants::ESPLORA_SERVERS[0].to_string())
            .set_entropy_bip39_mnemonic(mnemonic, Some("TREZOR".to_string()))
            .build();
        node.start().unwrap();

        assert_eq!(format!("{:?}", node.node_id()), "PublicKey(9720ef321576ad8d8709809f4a3a44c217fcef447475f712c3b02c0a2a1b4d4936f030becdc20dd920e1bfa4647fbefd7919bd6ea04ecb82e8eb8d926dd294a0)");
        assert_eq!(
            format!("{:?}", node.new_funding_address()),
            "Ok(bc1qv5rmq0kt9yz3pm36wvzct7p3x6mtgehjul0feu)"
        );
        assert_eq!(
            format!("{:?}", node.onchain_balance()),
            "Ok(Balance { immature: 0, trusted_pending: 0, untrusted_pending: 0, confirmed: 0 })"
        );
        assert_eq!(format!("{:?}", node.list_channels()), "[]");
        assert_eq!(
            format!("{:?}", node.listening_address()),
            "Some(0.0.0.0:9735)"
        );

        // .. fund address ..

        if let Err(e) = node.sync_wallets() {
            eprintln!("Failed to sync the node: {}", e);
        }

        let invoice = node
            .receive_variable_amount_payment("test", 60 * 30)
            .unwrap();
        assert_eq!(invoice.amount_milli_satoshis(), None);

        let id_addr = crate::constants::LN_ULR.split("@").collect::<Vec<_>>();
        assert_eq!(id_addr.len(), 2);
        let node_id = PublicKey::from_str(id_addr[0]).unwrap();
        let node_addr = id_addr[1].parse().unwrap();
        node.connect(node_id, node_addr, false).unwrap();
        // node.connect_open_channel(node_id, node_addr, 10000, None, false)
        //    .unwrap();

        // let invoice = Invoice::from_str("INVOICE_STR").unwrap();
        // node.send_payment(&invoice).unwrap();

        node.stop().unwrap();
    }
}
