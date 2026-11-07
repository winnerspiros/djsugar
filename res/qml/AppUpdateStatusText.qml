import QtQuick
import Mixxx 1.0 as Mixxx

// What the app update (Mixxx.AppUpdate) is doing, in a sentence
Text {
    readonly property var update: Mixxx.AppUpdate
    readonly property string sizeText: qsTr("%1 MB").arg(Math.round(update.downloadSize / (1024 * 1024)))

    function errorText() {
        switch (update.error) {
        case Mixxx.AppUpdate.Network:
            return qsTr("Unable to look for updates. Is there an Internet connection?");
        case Mixxx.AppUpdate.NoSpace:
            return qsTr("Not enough free storage for the update (%1).").arg(sizeText);
        case Mixxx.AppUpdate.Download:
            return qsTr("The download failed or was cancelled.");
        case Mixxx.AppUpdate.Damaged:
            return qsTr("The download is damaged, download it again.");
        case Mixxx.AppUpdate.Installer:
            return qsTr("Unable to open the Android installer.");
        }
        return "";
    }

    color: update.state === Mixxx.AppUpdate.Failed ? "#ff8a80" : "#cfd8dc"
    wrapMode: Text.WordWrap
    text: {
        switch (update.state) {
        case Mixxx.AppUpdate.Checking:
            return qsTr("Looking for updates…");
        case Mixxx.AppUpdate.UpToDate:
            return qsTr("Mixxx is up to date.");
        case Mixxx.AppUpdate.Available:
            return qsTr("Mixxx %1 is available (%2).").arg(update.latestVersion).arg(sizeText);
        case Mixxx.AppUpdate.Downloading:
            return qsTr("Downloading Mixxx %1… %2%").arg(update.latestVersion).arg(Math.round(update.progress * 100));
        case Mixxx.AppUpdate.Verifying:
            return qsTr("Checking the download…");
        case Mixxx.AppUpdate.Downloaded:
            return qsTr("Mixxx %1 is ready to install.").arg(update.latestVersion);
        case Mixxx.AppUpdate.Failed:
            return errorText();
        }
        return qsTr("Look for a newer version of Mixxx Android.");
    }
}
