import QtQuick
import Mixxx 1.0 as Mixxx

Category {
    label: qsTr("Mixer & Effects")

    Mixxx.SettingParameter {
        label: qsTr("A green square")

        Rectangle {
            color: 'green'
            height: 20
            width: 20
        }
    }
}
