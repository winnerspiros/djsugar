# Controller mappings

These are Mixxx's MIDI controller mappings (`*.midi.xml`) and their
JavaScript, copied unchanged from Mixxx's `res/controllers` at commit
0fffd3d0d07b755c34c1730f7b87afadd8f2be19 (2026-10-11), plus its HID
mappings (`*.hid.xml`) for Native Instruments' Traktor controllers with
their scripts and `common-hid-packet-parser.js`, from the same commit. Other
HID mappings, bulk mappings, TypeScript sources and screen (QML) files are
left out. `crates/controllers/tests/mappings.rs` and `tests/hid.rs` check
that every mapping here loads.

Each file keeps its authors' copyright notices and licence. Mixxx and its
mappings are licensed under the GNU General Public License, version 2 or
later; `lodash.mixxx.js` is a build of Lodash, licensed under the MIT
licence (see its header). The authors of each mapping are listed in its
`<info><author>` element.

mixxx-rs looks for this folder next to its executable (inside the app
bundle on macOS), in the repository, or wherever `MIXXX_RS_CONTROLLERS`
points, and picks a mapping for each connected controller by name.
