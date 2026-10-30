import QtQuick
import Mixxx 1.0 as Mixxx

Category {
    label: qsTr("Recording")

    Mixxx.SettingParameter {
        label: qsTr("A red square")

        Rectangle {
            color: 'red'
            height: 20
            width: 20
        }
    }
}
