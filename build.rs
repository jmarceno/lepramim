use cxx_qt_build::QmlFile;
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("app.lepramim").qml_files([
        QmlFile::from("qml/Main.qml"),
        QmlFile::from("qml/LepramimTheme.qml"),
        QmlFile::from("qml/ControlWindow.qml"),
        QmlFile::from("qml/OverlayWindow.qml"),
        QmlFile::from("qml/OnboardingWindow.qml"),
        QmlFile::from("qml/WarningWindow.qml"),
        // NOTE: all files must live directly in qml/ (no subdirectories).
        // cxx-qt 0.10 + Qt 6.2 misresolves module types from subdirectories
        // (random "X is not a type" failures); flat layout loads reliably.
        QmlFile::from("qml/Sidebar.qml"),
        QmlFile::from("qml/VoicePage.qml"),
        QmlFile::from("qml/PreprocessorPage.qml"),
        QmlFile::from("qml/AdvancedPage.qml"),
        QmlFile::from("qml/ModelsPage.qml"),
        QmlFile::from("qml/Card.qml"),
        QmlFile::from("qml/TealButton.qml"),
        QmlFile::from("qml/NavItem.qml"),
        QmlFile::from("qml/StatusDot.qml"),
        QmlFile::from("qml/LabeledCombo.qml"),
        QmlFile::from("qml/TealSlider.qml"),
        QmlFile::from("qml/TealSwitch.qml"),
    ]))
    .files(["src/ui/controller.rs"])
    .qt_module("Network")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Svg")
    .qrc_resources(["src/lepramim/icons/lepramim.svg"])
    .build();
}
