/*
 * Copyright © 2018-2020 Rodney Dawes
 * Copyright: 2013 Michael Zanetti <michael_zanetti@gmx.net>
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
import QtGraphicalEffects 1.0
import QtMultimedia 5.8
import QtQuick 2.7
import QtQuick.Layouts 1.3
import QtQuick.Controls 2.2
import QtQuick.Window 2.0

//import QZXing 2.3

Page {
    id: scanPage

    signal close()
    signal save(string data)

/*
    header: AdaptiveToolbar {
        width: parent.width
        height: 48

        leadingActions: [
            Action {
                iconName: "go-previous-symbolic"
                shortcut: [StandardKey.Back, StandardKey.Cancel]
                onTriggered: {
                    scanPage.close();
                }
            }
        ]
        Label {
            anchors.fill: parent
            horizontalAlignment: Text.AlignLeft
            verticalAlignment: Text.AlignVCenter
            text: i18n.tr("Scan QR code")
            font.pixelSize: 24
        }
    }
*/

    Camera {
        id: camera

        focus.focusMode: Camera.FocusMacro + Camera.FocusContinuous
        focus.focusPointMode: Camera.FocusPointCenter

        exposure.exposureMode: Camera.ExposureBarcode
        exposure.meteringMode: Camera.MeteringSpot

        imageProcessing.sharpeningLevel: 0.5
        imageProcessing.denoisingLevel: 0.25

        viewfinder.minimumFrameRate: 30.0
        viewfinder.maximumFrameRate: 30.0
    }

    Timer {
        id: captureTimer
        interval: 250
        repeat: true
        running: Qt.application.active
        onTriggered: {
            videoOutput.grabToImage(function(result) {
                qrCodeReader.decodeImage(result.image);
            });
        }
    }

    VideoOutput {
        id: videoOutput

        anchors.fill: parent
        fillMode: VideoOutput.PreserveAspectCrop
        source: camera
        focus: true
        orientation: Screen.primaryOrientation == Qt.PortraitOrientation ? -90 : 0
    }

    Rectangle {
        id: zoneOverlay
        anchors.fill: parent
        color: "#000000"
    }

    Item {
        id: zoneMask
        anchors.fill: parent

        Rectangle {
            color: "red"
            width: (parent.width > parent.height ? parent.height : parent.width) * 0.75
            height: width
            anchors.centerIn: parent
        }
    }

    OpacityMask {
        opacity: 0.93
        invert: true
        source: ShaderEffectSource {
            sourceItem: zoneOverlay
            hideSource: true
        }
        maskSource: ShaderEffectSource {
            sourceItem: zoneMask
            hideSource: true
        }
        anchors.fill: parent
    }

/*
    QZXing {
        id: qrCodeReader

        enabledDecoders: QZXing.DecoderFormat_QR_CODE

        onTagFound: {
            captureTimer.stop();
            camera.stop();
            var account = AccountModel.createAccount(tag);
            if (account == null) {
                var popup = errorComponent.createObject(
                    scanPage,
                    {
                        summary: i18n.tr("Invalid Code Scanned"),
                        message: i18n.tr("The scanned QR code is not valid.")
                    }
                );
                popup.closed.connect(function() {
                    camera.start();
                    captureTimer.start();
                });
                popup.open();
            } else {
                scanPage.save(account);
            }
        }

        tryHarder: false
    }
*/

    Label {
        id: scanLabel
        anchors {
            left: parent.left
            top: parent.top
            right: parent.right
            margins: 4
        }

        opacity: 0.93
        width: parent.width
        padding: 4
        y: 4
        text: i18n.tr("Scan a QR Code containing account information")
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        font.pixelSize: 16
    }

    Component {
        id: errorComponent

        ErrorDialog {
            anchors.fill: parent
        }
    }
}
