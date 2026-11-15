import Mixxx 1.0 as Mixxx
import QtQuick

// Content inset for camera cutout (notch/punch hole). Window is edge-to-edge;
// skins keep backgrounds full size and move only controls. Only cutouts whose
// height overlaps `target` are considered.
QtObject {
    id: root

    // The item (laid out in window coordinates) whose rows share the cutout
    required property Item target
    // Extra room between a cutout and the content, in pixels
    property real margin: 6

    // Read by insetOf() so that the insets follow the item, as mapToItem()
    // alone is not tracked
    readonly property var span: [root.target.y, root.target.height]
    readonly property real left: root.insetOf(true)
    readonly property real right: root.insetOf(false)

    function insetOf(onLeft) {
        const cutouts = Mixxx.MobileApp.cutouts;
        const item = root.target;
        const window = item ? item.Window.window : null;
        if (!cutouts || cutouts.length === 0 || !window) {
            return 0;
        }
        const width = window.width;
        const top = item.mapToItem(null, 0, 0).y;
        const bottom = top + root.span[1];
        let inset = 0;
        for (let i = 0; i < cutouts.length; ++i) {
            const c = cutouts[i];
            if (c.y >= bottom || c.y + c.height <= top) {
                continue;
            }
            const centerX = c.x + c.width / 2;
            if (onLeft && centerX < width * 0.33) {
                inset = Math.max(inset, c.x + c.width + root.margin);
            } else if (!onLeft && centerX > width * 0.67) {
                inset = Math.max(inset, width - c.x + root.margin);
            }
        }
        return inset;
    }
}
