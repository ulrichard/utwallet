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
            visible: false
            onClicked: {
                main_timer.stop();

                mainPage.scanCode();

                main_timer.start();
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
                main_timer.stop();

                greeter.send(send_address.text, send_amount.text, desc_txt.text);
                send_address.text = "";

                main_timer.interval = 1000;
                main_timer.start();
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
            text: i18n.tr('Create Invoice')
            onClicked: {
                main_timer.stop();

                receive_qr_code.visible = false
                receive_qr_code.source = greeter.request(send_amount.text, desc_txt.text);
                receive_qr_code.visible = true;
                label_receive_addr.text = greeter.receiving_address;

                main_timer.interval = 10000;
                main_timer.start();
            }
        }

	ProgressBar {
	    id: channel1
	    value: 0.5
	    visible: false
	}

	TextArea {
	    id: eventlog
            Layout.fillWidth: true
            enabled: false
	    text: "node is starting\n\n\n\n\n"
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
        
        Button {
            id: btn_channel_open;
            text: i18n.tr('Channel Open')
            onClicked: {
                main_timer.stop();
                greeter.channel_open(send_amount.text, send_address.text);
                main_timer.start();
            }
        }

        Button {
            id: btn_channel_close;
            text: i18n.tr('Channel Close')
            enabled: false
            onClicked: {
                main_timer.stop();
                greeter.channel_close();
                main_timer.start();
            }
        }

        Timer {
            id: main_timer;
            interval: 2000;
            running: true;
            repeat: true
            
            onTriggered: {
                console.log("main timer enter");
                main_timer.stop();
                eventlog.color = "steelblue"
                
                header.title = greeter.update_balance();
                receive_qr_code.source = greeter.address_qr();
                label_receive_addr.text = greeter.receiving_address;

                var chan = greeter.update_channel();
                if (chan == "") {
                    channel1.visible = false;
                    btn_channel_open.enabled = true;
                    btn_channel_close.enabled = false;
                } else {
                    channel1.visible = true;
                    btn_channel_open.enabled = false;
                    btn_channel_close.enabled = true;
                    channel1.value = Math.abs(parseFloat(chan));
                    if (chan.startsWith("-")) {
                    	// channel1.color = "red";
                    } else {
                    	// channel1.color = "green";
                    }
                }

                eventlog.color = "black"
                main_timer.interval = 20000;
                main_timer.start();
                console.log("main timer leave");
            }
        }

        Timer {
            id: exchange_timer;
            interval: 600000;
            running: true;
            repeat: true

            onTriggered: {
                console.log("exchange timer enter");
                eventlog.color = "steelblue"

                var rate = greeter.update_exchange_rate();

                eventlog.color = "black"
                console.log("exchange timer leave");
            }
        }
        Timer {
            id: event_timer;
            interval: 2000;
            running: true;
            repeat: true

            onTriggered: {
                // console.log("event timer enter");
                event_timer.stop();
                eventlog.color = "steelblue"

                eventlog.text = greeter.ldk_events();

                eventlog.color = "black"
                event_timer.start();
                // console.log("event timer leave");
            }
        }
    }
}
