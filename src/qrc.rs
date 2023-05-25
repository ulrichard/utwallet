qrc!(qml_resources,
    "/" {
        "qml/utlnwallet.qml",
        "qml/MainPage.qml",
        "qml/ScanPage.qml",
        "qml/ErrorDialog.qml"
    },
);

pub fn load() {
    qml_resources();
}
