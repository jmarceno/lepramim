use cxx_qt_build::QmlFile;
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("app.lepramim").qml_files([
        QmlFile::from("qml/Main.qml"),
        QmlFile::from("qml/Theme.qml").singleton(true),
        QmlFile::from("qml/ControlWindow.qml"),
        QmlFile::from("qml/OverlayWindow.qml"),
        QmlFile::from("qml/OnboardingWindow.qml"),
        QmlFile::from("qml/WarningWindow.qml"),
        QmlFile::from("qml/sidebar/Sidebar.qml"),
        QmlFile::from("qml/pages/VoicePage.qml"),
        QmlFile::from("qml/pages/PreprocessorPage.qml"),
        QmlFile::from("qml/pages/AdvancedPage.qml"),
        QmlFile::from("qml/pages/ModelsPage.qml"),
        QmlFile::from("qml/components/Card.qml"),
        QmlFile::from("qml/components/TealButton.qml"),
        QmlFile::from("qml/components/NavItem.qml"),
        QmlFile::from("qml/components/StatusDot.qml"),
        QmlFile::from("qml/components/LabeledCombo.qml"),
        QmlFile::from("qml/components/TealSlider.qml"),
        QmlFile::from("qml/components/TealSwitch.qml"),
    ]))
    .files(["src/ui/controller.rs"])
    .qt_module("Network")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qt_module("Svg")
    .qrc_resources(["src/lepramim/icons/lepramim.svg"])
    .build();
}
