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

// for widgets visit:
// https://doc.qt.io/qt-6/qtquick-controls2-qmlmodule.html

Page {
    id: mainPage

    signal scanCode()

    Greeter {
        id: greeter
    }
        
    anchors.fill: parent

    header: PageHeader {
        id: header
        title: i18n.tr('utwallet')
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

        Button {
            text: i18n.tr('Scan')
            onClicked: {
                mainPage.scanCode();
            }
        }

        Label {
            id: label_send_address
            text: i18n.tr('Address or Invoice')
        }
        
        TextField {
            id: send_address
            placeholderText: i18n.tr('Address or Invoice')
            Layout.fillWidth: true
//            onEditingFinished: {
//                greeter.evaluate_address_input(send_address.text, send_amount.text, desc_txt.text);
//            }
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
            id: label_desc_txt
            text: i18n.tr('Description')
        }
        
        TextField {
            id: desc_txt
            placeholderText: i18n.tr('lunch split')
            width: units.gu(20)
        }

        Button {
            text: i18n.tr('Send')
            onClicked: {
                greeter.send(send_address.text, send_amount.text, desc_txt.text);
            }
        }

        Button {
            text: i18n.tr('Evaluate Address or Invoice')
            onClicked: {
                var txt = greeter.evaluate_address_input(send_address.text, send_amount.text, desc_txt.text);
                var words = txt.split(";");
                send_address.text = words[0];
                send_amount.text = words[1];
                desc_txt.text = words[2];
            }
        }

        Button {
            text: i18n.tr('Channel Open')
            onClicked: {
                greeter.channel_open(send_amount.text);
            }
        }

        Button {
            text: i18n.tr('Create Invoice')
            onClicked: {
                main_timer.stop();

                receive_qr_code.source = greeter.request(send_amount.text, desc_txt.text);
                label_receive_addr.text = greeter.receiving_address;

                main_timer.interval = 60000;
                main_timer.start();
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
        
        Timer {
            id: main_timer;
            interval: 2000;
            running: true;
            repeat: true
            
            onTriggered: {
                main_timer.stop();
                
                header.title = greeter.update_balance();
                
                receive_qr_code.source = greeter.address_qr();
                label_receive_addr.text = greeter.receiving_address;

                main_timer.interval = 20000;
                main_timer.start();
            }
        }
    }
}
