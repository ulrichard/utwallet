/*
 * Copyright (C) 2022  Richard Ulrich
 *
 *   Based on an example from rust-qt-binding-generator
 *   Copyright 2017  Jos van den Oever <jos@vandenoever.info>
 *
 *   This program is free software; you can redistribute it and/or
 *   modify it under the terms of the GNU General Public License as
 *   published by the Free Software Foundation; either version 2 of
 *   the License or (at your option) version 3 or any later version
 *   accepted by the membership of KDE e.V. (or its successor approved
 *   by the membership of KDE e.V.), which shall act as a proxy
 *   defined in Section 14 of version 3 of the license.
 *
 *   This program is distributed in the hope that it will be useful,
 *   but WITHOUT ANY WARRANTY; without even the implied warranty of
 *   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *   GNU General Public License for more details.
 *
 *   You should have received a copy of the GNU General Public License
 *   along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
use crate::log_err;
use crate::wallet::create_wallet;

use qmetaobject::*;

use bdk::{
    blockchain::ElectrumBlockchain, database::MemoryDatabase, electrum_client::Client, SyncOptions,
    Wallet,
};

use chrono::prelude::*;
use std::{collections::HashMap};


const ELECTRUM_SERVER: &str = "ssl://ulrichard.ch:50002";

#[derive(Default, Clone)]
struct TransactionItem {
    date: u64,
    amount: f32,
}

#[allow(non_snake_case)]
#[derive(Default, QObject)]
pub struct TransactionModel {
    base: qt_base_class!(trait QAbstractListModel),
    count: qt_property!(i32; READ row_count NOTIFY count_changed),
    count_changed: qt_signal!(),
    list: Vec<TransactionItem>,
    wallet: Option<Wallet<MemoryDatabase>>,

    construct_wallet: qt_method!(
        fn construct_wallet(&mut self) {
            self.wallet = Some(log_err(create_wallet()));
        }
    ),
    insert_rows: qt_method!(fn(&mut self, row: usize, count: usize) -> bool),
    remove_rows: qt_method!(fn(&mut self, row: usize, count: usize) -> bool),
    add: qt_method!(fn(&mut self, date: u64, amount: f32)),
    remove: qt_method!(fn(&mut self, index: u64) -> bool),
    update_transactions: qt_method!(
        fn update_transactions(&mut self) {
            match self.get_transactions() {
                Ok(txs) => {
                    self.clear();
                    for tx in txs {
                        self.add(tx.0, tx.1);
                    }
                }
                Err(e) => {
                    self.clear();
                    eprintln!("{}", e);
                }
            }
        }
    ),
}

impl TransactionModel {
    fn insert_rows(&mut self, row: usize, count: usize) -> bool {
        if count == 0 || row > self.list.len() {
            return false;
        }
        (self as &mut dyn QAbstractListModel)
            .begin_insert_rows(row as i32, (row + count - 1) as i32);
        for i in 0..count {
            self.list.insert(row + i, TransactionItem::default());
        }
        (self as &mut dyn QAbstractListModel).end_insert_rows();
        self.count_changed();
        true
    }

    fn remove_rows(&mut self, row: usize, count: usize) -> bool {
        if count == 0 || row + count > self.list.len() {
            return false;
        }
        (self as &mut dyn QAbstractListModel)
            .begin_remove_rows(row as i32, (row + count - 1) as i32);
        self.list.drain(row..row + count);
        (self as &mut dyn QAbstractListModel).end_remove_rows();
        self.count_changed();
        true
    }

    fn add(&mut self, date: u64, amount: f32) {
        let end = self.list.len();
        (self as &mut dyn QAbstractListModel).begin_insert_rows(end as i32, end as i32);
        self.list.insert(end, TransactionItem { date, amount });
        (self as &mut dyn QAbstractListModel).end_insert_rows();
        self.count_changed();
    }

    fn remove(&mut self, index: u64) -> bool {
        self.remove_rows(index as usize, 1)
    }

    fn clear(&mut self) {
        self.remove_rows(0, self.count as usize);
    }

    pub fn get_transactions(&self) -> Result<Vec<(u64, f32)>, String> {
        let client = Client::new(ELECTRUM_SERVER).unwrap();
        let blockchain = ElectrumBlockchain::from(client);

        self.wallet
            .as_ref()
            .unwrap()
            .sync(&blockchain, SyncOptions::default())
            .map_err(|e| format!("Failed to synchronize: {:?}", e))?;

        let mut transactions = self
            .wallet
            .as_ref()
            .unwrap()
            .list_transactions(false)
            .map_err(|e| format!("Unable to get transactions: {:?}", e))?;
        transactions.sort_by(|a, b| {
            b.confirmation_time
                .as_ref()
                .map(|t| t.height)
                .cmp(&a.confirmation_time.as_ref().map(|t| t.height))
        });
        let transactions: Vec<_> = transactions
            .iter()
            .map(|td| {
                (
                    match &td.confirmation_time {
                        Some(ct) => ct.timestamp,
                        None => 0,
                    },
                    (td.received as f32 - td.sent as f32) / 100_000_000.0,
                )
            })
            .collect();
        println!("{:?}", transactions);

        Ok(transactions)
    }
}

impl QAbstractListModel for TransactionModel {
    fn row_count(&self) -> i32 {
        self.list.len() as i32
    }

    fn data(&self, index: QModelIndex, role: i32) -> QVariant {
        let idx = index.row() as usize;
        if idx < self.list.len() {
            if role == USER_ROLE {
                // Create a NaiveDateTime from the timestamp
				let datestring = if let Some(naive) = NaiveDateTime::from_timestamp_opt(self.list[idx].date as i64, 0) {
    
					// Create a normal DateTime from the NaiveDateTime
					let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    
					// Format the datetime how you want
					let dt = datetime.format("%Y-%m-%d %H:%M");
					
					format!("{}", dt)
				} else {
					"mempool".to_string()
				};
				
				QString::from(datestring).into()
            } else if role == USER_ROLE + 1 {
                QString::from(format!("{:.6}", self.list[idx].amount)).into()
            } else {
                QVariant::default()
            }
        } else {
            QVariant::default()
        }
    }

    fn role_names(&self) -> HashMap<i32, QByteArray> {
        let mut map = HashMap::new();
        map.insert(USER_ROLE, "date".into());
        map.insert(USER_ROLE + 1, "amount".into());
        map
    }
}
