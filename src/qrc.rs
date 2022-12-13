qrc!(qml_resources,
    "/" {
        "qml/Main.qml",
        "ScanPage.qml",
        "ErrorDialog.qml"
    },
);

pub fn load() {
    qml_resources();
}
