# utlnwallet
Bitcoin Lightning wallet for ubports

[![OpenStore](https://open-store.io/badges/en_US.svg)](https://open-store.io/app/utlnwallet.ulrichard)

It stores the secret information on the filesystem, protected only by the file system permissions. Hence don't store too much value with this app. Use it only for day to day spending and store your wealth on hardware wallets! Backup of the seed has to be done manually at the moment.

At the moment it is designed to only open a single private channel. It defaults to my ulrichard.ch node.

When you open the app for the first time, it generates a new random seed, and writes it to a file. On the user interface you see the current balance separate as on-chain and in lightning channels. At the bottom of the UI, you will see a qr code with an on-chain address, where you can send the first BTC. If you tap the QR code, it is copied into the clipboard.

The "Address or invoice" field is more versatile than it appears at first sight. It currently supports the following formats:
* a Bitcoin address, can be legacy or Beech32
* a BOLT11 lightning invoice
* a BTC URL that contains an amount, such as: "bitcoin:bc1qa8dn66xn2yq4fcaee4f0gwkkr6e6em643cm8fa?label=test&amount=100"
* a private key for sweeping. Can be either WIF, XPRV or a miniscript descriptor
* an LNURL for paying
* an LNURLW for withdrawing
* a lightning address that looks like an eMail address
* a lightning node id for opening a channel
* soon to come: BOLT12 offers and taproot addresses

So far, I did not integrate a qr scanner into the app. But if you have utlnwallet opened, tagger can automatically send the information over. If it is not already running, it will also start the app, but in this case, the data transfer doesn't work yet.
