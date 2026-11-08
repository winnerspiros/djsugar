import Mixxx 1.0 as Mixxx
import QtQuick 2.12

/// Usually, this component shouldn't be an (visual) `Item` and use something
/// like `QtObject` instead. However, for some reason using `QtObject` here
/// makes Mixxx crash on load (using Qt 5.15.2+kde+r43-1). We can check if this
/// is fixed upstream once we switch to Qt 6.
Item {
    id: root

    required property string group
    property bool enabled: true
    // Instantiator delegates see index -1 for a moment while being created,
    // which made "[Channel0]": the controls are only looked up for a real deck
    readonly property bool validGroup: /^\[Channel[1-9][0-9]*\]$/.test(group)

    signal loadTrackRequested(bool play)

    Mixxx.ControlProxy {
        group: root.group
        key: root.validGroup ? "LoadSelectedTrack" : ""
        onValueChanged: (value) => {
            if (value == 0 || !root.enabled)
                return ;

            root.loadTrackRequested(false);
        }
    }

    Mixxx.ControlProxy {
        group: root.group
        key: root.validGroup ? "LoadSelectedTrackAndPlay" : ""
        onValueChanged: (value) => {
            if (value == 0 || !root.enabled)
                return ;

            root.loadTrackRequested(true);
        }
    }
}
