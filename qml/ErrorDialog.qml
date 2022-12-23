/*
 * Copyright © 2020 Rodney Dawes
 *
 * This project is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This project is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 */
//import Ergo 0.0
//import OAth 1.0
import QtQuick 2.7
import QtQuick.Layouts 1.1
import QtQuick.Controls 2.2

Item {
    id: mainRect

    property alias summary: summaryLabel.text
    property alias message: messageLabel.text

    signal closed()

    function open() {
        errorDialog.open();
    }

    function close() {
        errorDialog.close();
    }

    visible: errorDialog && errorDialog.opened

    Popup {
        id: errorDialog

        x: parent.width / 2 - width / 2
        y: parent.height / 2 - height / 2

        modal: true
        closePolicy: Popup.CloseOnEscape

        onClosed: {
            destroy();
        }

        background: Rectangle {
            color: "#111111"
            opacity: 0.99
            radius: 8
        }

        ColumnLayout {
            width: parent.width
            spacing: 12

            Label {
                id: summaryLabel
                Layout.fillWidth: true
                font.pixelSize: 24
                wrapMode: Text.WordWrap
            }

            Label {
                id: messageLabel
                Layout.fillWidth: true
                font.pixelSize: 14
                wrapMode: Text.WordWrap
            }

            Item {
                Layout.fillWidth: true
                height: 16
            }

            Button {
                Layout.alignment: Qt.AlignRight
                font.pixelSize: 16
                text: i18n.tr("Close")
                onClicked: {
                    mainRect.closed();
                    errorDialog.close();
                }
            }
        }
    }
}
