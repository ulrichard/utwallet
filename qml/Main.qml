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

import QtQuick 2.7
import QtQuick.Controls 2.2
import Ubuntu.Components 1.3
import QtQuick.Layouts 1.3
import Qt.labs.settings 1.0

import Greeter 1.0
import TransactionModel 1.0

// for widgets visit:
// https://doc.qt.io/qt-6/qtquick-controls2-qmlmodule.html

ApplicationWindow {
    id: root
    objectName: 'mainView'

    width: units.gu(45)
    height: units.gu(95)
    visible: true

    Greeter {
        id: greeter
    }
        
    TransactionModel {
        id: transactionsModel
    }
                
    Component {
        id: transactionsDelegate
        RowLayout {
            visible: true
            height: units.gu(3)
                Label {
                    id: label1
                    text: date
                    verticalAlignment: Text.AlignVCenter
                    font.pixelSize: 16
                    width: units.gu(16)
                }
                Label {
                    id: label2
                    text: amount
                    verticalAlignment: Text.AlignVCenter
                    font.pixelSize: 16
                }
        }
    }

    Page {
        anchors.fill: parent

        header: PageHeader {
            id: header
            title: i18n.tr('utwallet')
            
            Component.onCompleted: {
				header.title = greeter.update_balance();
			}
        }

        ColumnLayout {
            spacing: units.gu(2)
            anchors {
                margins: units.gu(2)
                top: header.bottom
                left: parent.left
                right: parent.right
                bottom: parent.bottom
            }

            Item {
                Layout.fillHeight: true
            }

            Label {
                id: label_send_address
                text: i18n.tr('Address')
            }
            
            TextField {
				id: send_address
				placeholderText: i18n.tr('Address')
				Layout.fillWidth: true
				
            }
            
            Label {
                id: label_send_amount
                text: i18n.tr('Amount [BTC]')
            }
            
            TextField {
				id: send_amount
				placeholderText: i18n.tr('Amount')
				width: units.gu(20)
            }

            Label {
                id: label_send_fee
                text: i18n.tr('FeeRate [sat/vbyte]')
            }
            
            TextField {
				id: send_fee
				placeholderText: i18n.tr('FeeRate [sat/vbyte]')
				width: units.gu(20)
				
				Component.onCompleted: {
					send_fee.text = greeter.estimate_fee();
				}
            }

            Button {
                text: i18n.tr('Send')
                onClicked: {
                    greeter.send(send_address.text, send_amount.text, send_fee.text);
                }
            }

            Label {
                id: label_receive
                text: i18n.tr('Receive')
            }
            
            Image {
				id: receive_qr_code
				fillMode: Image.Stretch
				
				Component.onCompleted: {
					receive_qr_code.source = greeter.address_qr();
				}
				
				MouseArea {
					anchors.fill: parent
					onClicked: {
						var mimeData = Clipboard.newData();
						mimeData.text = greeter.receiving_address;
						mimeData.color = "green";
						Clipboard.push(mimeData);
					}
				}
            }

            Label {
                id: label_receive_addr
                text: i18n.tr('Address')
                
                Component.onCompleted: {
					label_receive_addr.text = greeter.address();
				}

            }
            
            ListView {
                id: ubuntuListView
                height: units.gu(34)
                model: transactionsModel
                delegate: transactionsDelegate
            }
            
            Item {
                Layout.fillHeight: true
            }
            
            Timer {
				id: main_timer;
				interval: 2000;
				running: true;
				repeat: true
				
				onTriggered: {
					main_timer.stop();
					
					header.title = greeter.update_balance();
					
					transactionsModel.update_transactions();
					
					receive_qr_code.source = greeter.address_qr();

					main_timer.interval = 20000;
					main_timer.start();
				}
			}
        }
    }
}
