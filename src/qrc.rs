qrc!(qml_resources,
    "/" {
        "qml/utwallet.qml",
        "qml/MainPage.qml",
        "qml/ScanPage.qml",
        "qml/ErrorDialog.qml"
    },
);

pub fn load() {
    qml_resources();
}
