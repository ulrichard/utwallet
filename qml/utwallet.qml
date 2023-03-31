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
 *
 */

import QtQuick 2.7
import QtQuick.Layouts 1.3
import QtQuick.Controls 2.2
import QtQuick.Window 2.0
import "."

ApplicationWindow {
    id: root

    width: units.gu(45)
    height: units.gu(95)
    minimumWidth: 360
    minimumHeight: 640
    title: i18n.tr("utwallet")
    visible: true

    Shortcut {
        context: Qt.ApplicationShortcut
        sequence: StandardKey.Quit
        onActivated: {
            Qt.quit();
        }
    }

    StackView {
        id: pageStack
        anchors.fill: parent
        visible: true
        anchors.margins: 0
        focus: true
        Component.onCompleted: {
            pageStack.push(mainPageComponent);
        }
        Keys.onBackPressed: {
            if (depth > 1) {
                pop();
            } else {
                Qt.quit();
            }
        }
    }

    Component {
        id: mainPageComponent
        MainPage {
            id: mainPage
            visible: false

            onScanCode: {
                console.assert(root.pageStack !== null, "pageStack must not be empty");
                console.assert(root.scanPageComponent !== null, "scanPageComponent must not be empty");
                root.pageStack.push(root.scanPageComponent);
            }
        }
    }

    Component {
        id: scanPageComponent

        ScanPage {
            id: scanPage

            onSave: {
//                AccountModel.save(data);
                pageStack.pop();
            }

            onClose: {
                pageStack.pop();
            }
        }
    }
}
