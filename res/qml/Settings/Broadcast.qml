import QtQuick
import Mixxx 1.0 as Mixxx

Category {
    label: qsTr("Broadcast")

    Mixxx.SettingParameter {
        label: qsTr("A yellow square")

        Rectangle {
            color: 'yellow'
            height: 20
            width: 20
        }
    }
}
