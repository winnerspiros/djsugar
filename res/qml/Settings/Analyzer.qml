import QtQuick
import Mixxx 1.0 as Mixxx

Category {
    label: qsTr("Analyzer")

    Mixxx.SettingParameter {
        label: qsTr("A grey square")

        Rectangle {
            color: 'grey'
            height: 20
            width: 20
        }
    }
}
