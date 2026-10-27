import QtQuick
import Mixxx 1.0 as Mixxx

Category {
    label: qsTr("AutoDJ")

    Mixxx.SettingParameter {
        label: qsTr("A black square")

        Rectangle {
            color: 'black'
            height: 20
            width: 20
        }
    }
}
