<!-- Text written autonomously by an AI agent, reviewed by the maintainer before publishing -->

> From [JaviLabarumDJ/mixxx-android](https://github.com/JaviLabarumDJ/mixxx-android) (`feb9850231`).

# Mixxx community mappings

Controller mappings for Mixxx that are not included with DJ Sugar yet, collected so that users can download the one for their controller.

## How to install one

1. Download the folder of your controller from `github-pull-requests/`, `github-repos/`, or `generated-mappings/`.
2. Copy its controller files (`.xml`, `.js`, and `.qml` when present; copy all files in multi-file folders such as `Allen and Heath Xone K3` or `Ableton Push 2 Display`) to `Mixxx/controllers/` on the internal storage of your tablet or phone.
3. In Mixxx, open *Settings → DJ controllers*, tap *Search USB devices*, choose the mapping for your controller and enable it.

## Status

This collection contains three folders:

- **`github-pull-requests/` (40 mappings):** Proposed to the Mixxx project by their authors (GitHub pull requests) and **not merged into Mixxx yet**. They may be incomplete, need a recent Mixxx version, or change before being merged. **Not tested with DJ Sugar.**
- **`github-repos/` (15 mappings):** Community mappings published in authors' own GitHub repositories under redistributable licenses. Keep each folder's `SOURCE.txt` and `LICENSE` (for `Pioneer DDJ-FLX6` and `Pioneer DDJ-RB`, upstream license headers are documented in `SOURCE.txt`). **Not tested with DJ Sugar.**
- **`generated-mappings/` (13 mappings):** Mixxx 2.5/2.6+ mappings generated from official manufacturer MIDI implementation PDFs / verified USB protocol captures (plus `Pioneer XDJ-RX3`, adapted from `openwave-studio/opendj` for Mixxx 2.6+). **IMPORTANT: None of these generated mappings have been tested on physical DJ hardware or DJ Sugar yet** — treat them as experimental and verify controls carefully with real hardware before live use.

Each folder includes a `SOURCE.txt` with the author, source URL, and license details.

## License

- Mixxx upstream pull requests (`github-pull-requests/`) and spec-generated mappings (`generated-mappings/`) are under the GNU General Public License v2.0 or later (`GPL-2.0-or-later`, except `Pioneer XDJ-RX3` which is `GPL-2.0-only` upstream; see each folder's `LICENSE` / `SOURCE.txt` or https://github.com/mixxxdj/mixxx/blob/main/LICENSE).
- External repositories (`github-repos/`) carry their original author license (`MIT`, `GPL-2.0-only`, or `GPL-2.0-or-later`, as noted in each row and `SOURCE.txt`). When redistributing them, keep the author attribution, license notices, and source links.

## Pull requests from Mixxx (`github-pull-requests/`)

| Controller | Author | Proposal | Files |
|---|---|---|---|
| Ableton - Push | Replayman | [#11101](https://github.com/mixxxdj/mixxx/pull/11101) | `Ableton-Push.midi.xml`, `Ableton-Push-scripts.js` |
| Ableton Push 2 | OpenAI Codex | [#16782](https://github.com/mixxxdj/mixxx/pull/16782) | `Ableton Push 2.midi.xml`, `Ableton-Push-2-scripts.js` |
| Ableton Push 2 Display | OpenAI Codex | [#16782](https://github.com/mixxxdj/mixxx/pull/16782) | `Ableton Push 2 Display.bulk.xml`, `Ableton-Push-2-Display.qml` |
| Akai APC Mini | Valerio Vitali | [#3166](https://github.com/mixxxdj/mixxx/pull/3166) | `Akai-APC-MINI.midi.xml`, `Akai-APC-MINI-scripts.js` |
| Akai MPK Mini mkII | Valerio Vitali | [#3166](https://github.com/mixxxdj/mixxx/pull/3166) | `AKAI-MPKmini2.midi.xml`, `AKAI-MPKmini2-scripts.js` |
| Denon HC1000S mapping | le3d, based on many existing Denon other mappings, re-using bits and pieces | [#3799](https://github.com/mixxxdj/mixxx/pull/3799) | `Denon_DN_HC1000S.midi.xml`, `Denon-HC1000S-scripts.js` |
| Denon Prime 4 - Jog Wheel Displays | Levi "Whanake" Williams | [#4027](https://github.com/mixxxdj/mixxx/pull/4027) | `Denon-Prime-4-Jog-Wheels.midi.xml`, `Denon-Prime-4-jog-wheel-scripts.js` |
| Denon Prime 4 - Mixxx Mapping | Levi "Whanake" Williams | [#4027](https://github.com/mixxxdj/mixxx/pull/4027) | `Denon-Prime-4.midi.xml`, `Denon-Prime-4-scripts.js` |
| Denon Prime GO - Mixxx Mapping | Levi "Whanake" Williams | [#12047](https://github.com/mixxxdj/mixxx/pull/12047) | `Denon-Prime-GO.midi.xml`, `Denon-Prime-GO.scripts.js` |
| Denon SC Live 4 | Jammes Luckett (pka Sister Transistor) // Art Mechanix | [#17201](https://github.com/mixxxdj/mixxx/pull/17201) | `Denon-SC-Live-4.midi.xml`, `Denon-SC-Live-4-scripts.js` |
| Gemini G4V | Javier Vilarroig | [#4323](https://github.com/mixxxdj/mixxx/pull/4323) | `Gemini G4V.midi.xml`, `Gemini-G4V-scripts.js` |
| Hercules DJ Control Instinct P8 | Piff+Hubert+STi+Piega | [#1851](https://github.com/mixxxdj/mixxx/pull/1851) | `Hercules DJ Control Instinct P8.midi.xml`, `Hercules-DJ-Control-Instinct-P8-scripts.js` |
| Hercules DJControl AirPlus | DJ Phatso for Hercules Technical Support | [#17363](https://github.com/mixxxdj/mixxx/pull/17363) | `DJControl_AirPlus.midi.xml`, `DJControl_AirPlus.script.js` |
| Hercules DJControl Mix Ultra | Aayush Kapoor (xeoneux) | [#17400](https://github.com/mixxxdj/mixxx/pull/17400) | `Hercules DJControl Mix Ultra.midi.xml`, `Hercules-DJControl-Mix-Ultra-scripts.js` |
| Kruger Matz DJ-001 | Jairo Master Zion | [#16933](https://github.com/mixxxdj/mixxx/pull/16933) | `KrugerMatz-DJ-001.midi.xml`, `KrugerMatz-DJ-001-scripts.js` |
| Native Instruments Traktor Kontrol Z2 | Jörg Wartenberg | [#3038](https://github.com/mixxxdj/mixxx/pull/3038) | `Traktor Kontrol Z2.hid.xml`, `Traktor-Kontrol-Z2-hid-scripts.js` |
| Novation Launchkey Mini MK4 25 | theotacconi | [#16653](https://github.com/mixxxdj/mixxx/pull/16653) | `Novation-Launchkey-Mini-MK4-25.midi.xml`, `Novation-Launchkey-Mini-MK4-25.scripts.js` |
| Numark Mixtrack Go | mixo101 | [#16813](https://github.com/mixxxdj/mixxx/pull/16813) | `Numark-Mixtrack-Go.midi.xml`, `Numark-Mixtrack-Go.scripts.js` |
| Numark NS6 | maurojuniorr | [#17264](https://github.com/mixxxdj/mixxx/pull/17264) | `Numark NS6.midi.xml`, `Numark-NS6.scripts.js` |
| Numark Party Mix MKII | rrrapha | [#16155](https://github.com/mixxxdj/mixxx/pull/16155) | `Numark-Party-Mix-MKII.midi.xml`, `Numark-Party-Mix-MKII.scripts.js` |
| Pioneer DDJ-1000 | Serhii O. Pashchenko | [#17174](https://github.com/mixxxdj/mixxx/pull/17174) | `Pioneer-DDJ-1000.midi.xml`, `Pioneer-DDJ-1000-scripts.js` |
| Pioneer DDJ-FLX10 | Marc Zischka & Victor Pineda (Veezuhz) | [#16539](https://github.com/mixxxdj/mixxx/pull/16539) | `Pioneer-DDJ-FLX10.midi.xml`, `Pioneer-DDJ-FLX10-scripts.js` |
| Pioneer DDJ-FLX10 HID | Victor Pineda (Veezuhz) | [#16539](https://github.com/mixxxdj/mixxx/pull/16539) | `Pioneer-DDJ-FLX10-screen.hid.xml`, `Pioneer-DDJ-FLX10-screen.js` |
| Pioneer DDJ-FLX2 | CupoMeridio, RaduTek | [#16567](https://github.com/mixxxdj/mixxx/pull/16567) | `Pioneer-DDJ-FLX2.midi.xml`, `Pioneer-DDJ-FLX2-scripts.js` |
| Pioneer DDJ-GRV6 | Jörg Beyer | [#15801](https://github.com/mixxxdj/mixxx/pull/15801) | `Pioneer-DDJ-GRV6.midi.xml`, `Pioneer-DDJ-GRV6-scripts.js` |
| Pioneer DDJ-REV1 | AKOI, Alexandr, LauraRozier, Mjlyon, 0b1-twan | [#16722](https://github.com/mixxxdj/mixxx/pull/16722) | `Pioneer DDJ-REV1.midi.xml`, `Pioneer-DDJ-REV1-scripts.js` |
| Pioneer DDJ-SR2 | Waylon Robertson | [#13429](https://github.com/mixxxdj/mixxx/pull/13429) | `Pioneer-DDJ-SR2.midi.xml`, `Pioneer-DDJ-SR2-scripts.js` |
| Pioneer DDJ-SX2 | Krafting, hrudham, tildearrow, eXWoLL1 | [#16372](https://github.com/mixxxdj/mixxx/pull/16372) | `PIONEER-DDJ-SX2.midi.xml`, `PIONEER-DDJ-SX2.scripts.js` |
| Pioneer DDJ-WeGO | Samuel Nicholas, Dmytro Hakman | [#17338](https://github.com/mixxxdj/mixxx/pull/17338) | `Pioneer-DDJ-WeGO.midi.xml`, `Pioneer-DDJ-WeGO-scripts.js` |
| Pioneer DDJ-XP2 | Rene (mixxx at absorb.it) | [#15791](https://github.com/mixxxdj/mixxx/pull/15791) | `Pioneer DDJ-XP2.midi.xml`, `Pioneer-DDJ-XP2-scripts.js` |
| Pioneer XDJ-RX | norayr | [#3008](https://github.com/mixxxdj/mixxx/pull/3008) | `Pioneer XDJ-RX.midi.xml`, `Pioneer-XDJ-RX-scripts.js` |
| PIONEER-DDJ-ERGO | Jesús Soto | [#13003](https://github.com/mixxxdj/mixxx/pull/13003) | `Pioneer-DDJ-ERGO.midi.xml`, `Pioneer-DDJ-ERGO-scripts.js` |
| Reloop Beatmix | Eugene Erokhin uncle.eugene@gmail.com | [#12422](https://github.com/mixxxdj/mixxx/pull/12422) | `Reloop-Beatmix.midi.xml`, `Reloop-Beatmix.scripts.js` |
| Reloop Digital Jockey 2 Master Edition | DJ aK (adapted for Master Edition) | [#16929](https://github.com/mixxxdj/mixxx/pull/16929) | `Reloop-Digital-Jockey-2-ME.midi.xml` |
| Reloop Mixtour | Rene (mixxx at absorb.it), András (andras08 at gmail.com), zfhrp | [#15790](https://github.com/mixxxdj/mixxx/pull/15790) | `Reloop Mixtour.midi.xml`, `Reloop-Mixtour-scripts.js` |
| Reloop Ready | Swiftb0y | [#4513](https://github.com/mixxxdj/mixxx/pull/4513) | `Reloop Ready.midi.xml`, `Reloop-Ready-scripts.js` |
| Roland DJ-202 | Silvio Ankermann (Lykos153), Sebastian Reuße (eigengrau) and René Kraus (upCASE), Jhade Williamson (evoixmr), BSDguy389 | [#1595](https://github.com/mixxxdj/mixxx/pull/1595) | `Roland_DJ-202.midi.xml`, `Roland_DJ-202-scripts.js` |
| Roland DJ-808 | Jan Holthuis | [#15063](https://github.com/mixxxdj/mixxx/pull/15063) | `Roland_DJ-808.midi.xml`, `Roland_DJ-808-scripts.js` |
| Traktor Kontrol X1 MK1 | Jan Claußen | [#17361](https://github.com/mixxxdj/mixxx/pull/17361) | `Traktor Kontrol X1 MK1.bulk.xml`, `Traktor Kontrol X1 MK1 bulk scripts.js` |
| Vestax VCI-380 | JM Jacinto | [#17089](https://github.com/mixxxdj/mixxx/pull/17089) | `Vestax VCI-380.midi.xml`, `Vestax-VCI-380-scripts.js` |

## Community mappings from other repositories (`github-repos/`)

Mappings published by their authors in their own GitHub repositories, with a license that allows sharing them. They are in `github-repos/`, each with a `SOURCE.txt` (author, link, license) and upstream `LICENSE` (for `Pioneer DDJ-FLX6` and `Pioneer DDJ-RB`, license notices are recorded in `SOURCE.txt`).

| Controller | Author | Source | License | Files |
|---|---|---|---|---|
| Akai AMX | Prism (rootzoll mod) | [rootzoll/mixxx-akai-amx](https://github.com/rootzoll/mixxx-akai-amx) | MIT | `AMX.midi.xml`, `Akai-AMX-scripts.js` |
| Allen and Heath Xone K3 | myCatnip | [myCatnip/mixxx-k3-myCatnip](https://github.com/myCatnip/mixxx-k3-myCatnip) | GPL-2.0-only | `xone-k3-{cat,colorlab,kitten,onek3,onek3-minifx}.midi.xml` + 22 `.js` files |
| Denon LC6000 Prime | schmic | [schmic/mixxx-denon-lc6000](https://github.com/schmic/mixxx-denon-lc6000) | GPL-2.0-only | `Denon-LC6000-Prime.midi.xml`, `Denon-LC6000-Prime-scripts.js`, `Denon-LC6000-Prime-Display.midi.xml`, `Denon-LC6000-Prime-Display-scripts.js` |
| Hercules DJControl Inpulse 300 MK2 | Adrien Chinour | [adrien-chinour/djing](https://github.com/adrien-chinour/djing/tree/main/mixxx/controllers) | GPL-2.0-only | `Hercules_DJControl_Inpulse_300_MK2.midi.xml`, `Hercules-DJControl-Inpulse-300-MK2-script.js` |
| Hercules DJControl Inpulse T7 | Infinite-Grok | [Infinite-Grok/mixxx-hercules-t7](https://github.com/Infinite-Grok/mixxx-hercules-t7) | GPL-2.0-only | `Hercules_DJControl_Inpulse_T7.midi.xml`, `Hercules-DJControl-Inpulse-T7-platter.js`, `Hercules-DJControl-Inpulse-T7-script.js` |
| Native Instruments Traktor Kontrol Z1 MK2 | Infinifox | [infifox/mixxx-traktor-controllers](https://github.com/infifox/mixxx-traktor-controllers/releases/tag/v1.0.0) | MIT | `Traktor.Z1.MK2.hid.xml`, `Traktor-Z1-MK2.script.js` |
| Pioneer DDJ-800 | RandomByte | [randombyte-developer/ddj-800](https://github.com/randombyte-developer/ddj-800/releases/tag/v1.5.0) | GPL-2.0-only | `pioneer-ddj800.midi.xml`, `pioneer-ddj800.js` |
| Pioneer DDJ-FLX6 | Robert904, fixxJ and Mixxx DDJ-FLX4 authors | [jrasko/DDJ-FLX6-Mixxx-mapping](https://github.com/jrasko/DDJ-FLX6-Mixxx-mapping) | GPL-2.0-or-later | `Pioneer-DDJ-FLX6.midi.xml`, `Pioneer-DDJ-FLX6-script.js` |
| Pioneer DDJ-RB | Trinity Atom, Luis Mita, Be and contributors | [Trinity-Atom/Mixxx-Mapping-for-DDJ-RB](https://github.com/Trinity-Atom/Mixxx-Mapping-for-DDJ-RB) | GPL-2.0-or-later / MIT | `Pioneer-DDJ-RB.midi.xml`, `Pioneer-DDJ-RB-scripts.js` |
| Pioneer DDJ-SR | Hilton Rudham (hrudham) | [hrudham/Mixxx-Pioneer-DDJ-SR](https://github.com/hrudham/Mixxx-Pioneer-DDJ-SR) | MIT | `PIONEER_DDJ-SR.midi.xml`, `PIONEER_DDJ-SR-scripts.js` |
| Pioneer DDJ-SX3 | Maximilian Beiersdorfer (DJMaxergy), ardje | [ardje/Mixxx-Pioneer-DDJ-SX3](https://github.com/ardje/Mixxx-Pioneer-DDJ-SX3) | GPL-2.0-or-later / MIT | `PIONEER_DJ_DDJ-SX3.midi.xml`, `Pioneer-DDJ-SX3-scripts.js` |
| Pioneer DDJ-WeGO3 | Matthew Scott | [matthewryanscott/mixxx-pioneer-ddj-wego3](https://github.com/matthewryanscott/mixxx-pioneer-ddj-wego3) | MIT | `Pioneer-DDJ-WeGO3.midi.xml`, `Pioneer-DDJ-WeGO3-scripts.js` |
| Pioneer DDJ-WeGO4 | Ryan Baxter (rbax) | [rbax/mixxx-mappings](https://github.com/rbax/mixxx-mappings) | MIT | `PIONEER_DDJ-WeGO4.midi.xml`, `Pioneer-DDJ-WeGO4.scripts.js` |
| Reloop Mixon 4 | daTobi1 | [daTobi1/mixxx](https://github.com/daTobi1/mixxx/tree/reloop-mixon-4-mapping) | GPL-2.0-or-later | `Reloop-Mixon-4.midi.xml`, `Reloop-Mixon-4.scripts.js` |
| Reloop Mixtour Pro | Aron Birkir (Aronsky) | [aronbirkir/reloop-mixtour-pro-mixxx](https://github.com/aronbirkir/reloop-mixtour-pro-mixxx) | MIT | `Reloop-Mixtour-Pro.midi.xml`, `Reloop-Mixtour-Pro.scripts.js` |

## Generated from official manufacturer MIDI specs (`generated-mappings/`)

> **Warning — NOT hardware-tested:** All mappings in `generated-mappings/` were generated from official manufacturer MIDI implementation PDFs / verified USB protocol documentation (plus `Pioneer XDJ-RX3` adapted from OpenDJ) and validated for XML/JS syntax and Mixxx 2.5/2.6+ ControlObjects. **None of them have been tested on physical DJ hardware or Mixxx Android.** Test with a real controller before relying on them live or submitting upstream.

| Controller | Target Mixxx | Source / Reference | License | Files |
|---|---|---|---|---|
| AlphaTheta OMNIS-DUO | Mixxx 2.5 / 2.6+ | [Official AlphaTheta MIDI Spec PDF](https://cdn.rekordbox.com/files/20240119175526/OMNIS-DUO_List_of_MIDI_messages_E.pdf) | GPL-2.0-or-later | `AlphaTheta-OMNIS-DUO.midi.xml`, `AlphaTheta-OMNIS-DUO-scripts.js` |
| AlphaTheta XDJ-AZ | Mixxx 2.5 / 2.6+ | [Official AlphaTheta MIDI Spec PDF](https://cdn.rekordbox.com/files/20241016164551/XDJ-AZ_MIDI_message_list_E.pdf) | GPL-2.0-or-later | `AlphaTheta-XDJ-AZ.midi.xml`, `AlphaTheta-XDJ-AZ-scripts.js` |
| Hercules DJControl Inpulse 200 MK2 | Mixxx 2.5 / 2.6+ | [g-dameg78 USB Protocol](https://github.com/g-dameg78/hercules-inpulse-200-mk2-linux/blob/main/docs/PROTOCOL.md) + Mixxx Inpulse 200 | GPL-2.0-or-later | `Hercules_DJControl_Inpulse_200_MK2.midi.xml`, `Hercules-DJControl-Inpulse-200-MK2-scripts.js` |
| Pioneer DDJ-800 *(4-Deck Official Spec)* | Mixxx 2.5 / 2.6+ | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/ddj-800/ddj-800_midi_message_list_e1.pdf) | GPL-2.0-or-later | `Pioneer-DDJ-800-Full.midi.xml`, `Pioneer-DDJ-800-Full-scripts.js` |
| Pioneer DDJ-REV5 | Mixxx 2.6+ (Stems) | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/ddj-rev5/ddj-rev5_midi_message_list_e.pdf) | GPL-2.0-or-later | `Pioneer-DDJ-REV5.midi.xml`, `Pioneer-DDJ-REV5-scripts.js` |
| Pioneer DDJ-REV7 | Mixxx 2.6+ (Stems) | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/ddj-rev7/ddj-rev7_midi_message_list_e.pdf) | GPL-2.0-or-later | `Pioneer-DDJ-REV7.midi.xml`, `Pioneer-DDJ-REV7-scripts.js` |
| Pioneer DDJ-RR | Mixxx 2.5 / 2.6+ | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/ddj-rr/ddj-rr_list_of_midi_message_e.pdf) | GPL-2.0-or-later | `Pioneer-DDJ-RR.midi.xml`, `Pioneer-DDJ-RR-scripts.js` |
| Pioneer DDJ-RZ | Mixxx 2.5 / 2.6+ | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/ddj-rz/ddj-rz_list_of_midi_message_e.pdf) | GPL-2.0-or-later | `Pioneer-DDJ-RZ.midi.xml`, `Pioneer-DDJ-RZ-scripts.js` |
| Pioneer DJ OPUS-QUAD | Mixxx 2.5 / 2.6+ | [Official AlphaTheta MIDI Spec PDF](https://cdn.rekordbox.com/files/20230308164348/OPUS-QUAD_List_of_MIDI_Messages_en.pdf) | GPL-2.0-or-later | `Pioneer-DJ-OPUS-QUAD.midi.xml`, `Pioneer-DJ-OPUS-QUAD-scripts.js` |
| Pioneer XDJ-RR | Mixxx 2.5 / 2.6+ | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/xdj-rr/xdj-rr_list_of_midi_message_e.pdf) | GPL-2.0-or-later | `Pioneer-XDJ-RR.midi.xml`, `Pioneer-XDJ-RR-scripts.js` |
| Pioneer XDJ-RX2 | Mixxx 2.5 / 2.6+ | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/xdj-rx2/xdj-rx2_list_of_midi_message_e3.pdf) | GPL-2.0-or-later | `Pioneer-XDJ-RX2.midi.xml`, `Pioneer-XDJ-RX2-scripts.js` |
| Pioneer XDJ-RX3 | Mixxx 2.6+ | [openwave-studio/opendj](https://github.com/openwave-studio/opendj) *(adapted for Mixxx 2.6+)* | GPL-2.0-only | `Pioneer-XDJ-RX3.midi.xml`, `Pioneer-XDJ-RX3-script.js` |
| Pioneer XDJ-XZ | Mixxx 2.5 / 2.6+ | [Official Pioneer DJ MIDI Spec PDF](https://www.pioneerdj.com/-/media/pioneerdj/downloads/manuals/other/xdj-xz/xdj-xz_midi_message_list_e.pdf) | GPL-2.0-or-later | `Pioneer-XDJ-XZ.midi.xml`, `Pioneer-XDJ-XZ-scripts.js` |

## Links only

These mappings exist externally, but are not copied here because their repositories either do not specify a redistributable license, use `GPL-3.0`/`AGPL-3.0`, or require platform-specific patches/companion software: download them directly from their authors.

| Controller | Source | Notes |
|---|---|---|
| Akai APC40 | [paulusr/Mixxx-APC40-Controller](https://github.com/paulusr/Mixxx-APC40-Controller) | No license file specified |
| Denon DN-S3700 (hybrid) | [HoneyHazard/mixxx-Denon-DN-S3700-hybrid](https://github.com/HoneyHazard/mixxx-Denon-DN-S3700-hybrid) | No license file specified |
| Gemini GMX | [codecat/mixxx-gemini-gmx](https://github.com/codecat/mixxx-gemini-gmx) | No license file specified |
| Hercules DJControl Inpulse 200 MK3 | [dusan-ivanco/Mixxx_Hercules_DJControl_Inpulse_200_MK3](https://github.com/dusan-ivanco/Mixxx_Hercules_DJControl_Inpulse_200_MK3) | No license file specified |
| Hercules DJControl Inpulse 300 MK2 (4-deck fork) | [DCinAC/mixxx-inpulse-300-mk2](https://github.com/DCinAC/mixxx-inpulse-300-mk2) ([Mixxx Discourse](https://mixxx.discourse.group/t/hercules-djcontrol-inpulse-300mk2/27960)) | Forked from DJ Phatso, no license file (2-deck GPL-2.0 mapping in `github-repos/`) |
| Native Instruments Traktor Kontrol S8 | [gusgustavodj/kontrol-s8-mixxx-mappings](https://github.com/gusgustavodj/kontrol-s8-mixxx-mappings) | Requires Windows C++ display patch (`GPL-2.0`) |
| Novation Launchpad Pro MK3 | [mxmilkiib/mixxx-novation-launchpad-pro-mk3-milkii](https://github.com/mxmilkiib/mixxx-novation-launchpad-pro-mk3-milkii) | No license file specified |
| Novation Twitch | [bfreebody-cpu/novation-twitch-modern](https://github.com/bfreebody-cpu/novation-twitch-modern) | `GPL-3.0`, macOS Swift restoration companion |
| Numark Party Mix II / Party Mix Live | [magtomm/MIXXX-Numark-party-mix-2](https://github.com/magtomm/MIXXX-Numark-party-mix-2) | No license file specified (see also Party Mix MKII in `github-pull-requests/`) |
| Pioneer DDJ-1000 / XDJ-XZ toolkit | [ntamas94/pioneered-by-ntamas](https://github.com/ntamas94/pioneered-by-ntamas) | `GPL-3.0`, Linux bridge + skin + mapping |
| Pioneer DDJ-S1 | [Jikstra/mixxx-mappings-pioneer-ddj1-s1](https://github.com/Jikstra/mixxx-mappings-pioneer-ddj1-s1) | No license file specified |
| Pioneer DDJ-SP1 | [theaugy/sp1](https://github.com/theaugy/sp1) | No license file specified |
| Pioneer DDJ-T1 | [kumy/Pioneer-DDJ-T1---mixxx-mapping](https://github.com/kumy/Pioneer-DDJ-T1---mixxx-mapping) | No license file specified, WIP |
| Rane Four | [ElectroDanceWorld-Radio/rane-four-mixxx-map](https://github.com/ElectroDanceWorld-Radio/rane-four-mixxx-map) | `AGPL-3.0`, early reverse-engineered WIP |
| Reloop Buddy, Mixars Primo | [Andymann/mixxx.controller.reloopBuddy](https://github.com/Andymann/mixxx.controller.reloopBuddy) | No license file specified |
| Reloop Mixtour Pro (alternative) | [sayak-brm/ReloopMixxxtourPro](https://github.com/sayak-brm/ReloopMixxxtourPro) | No license file specified |

## Controllers without a mapping yet

No verified public mapping or redistributable official MIDI chart was found for:

- **Denon Prime 2** (no open-source Mixxx mapping or public official MIDI implementation PDF found)
- **Reloop Mixon 8 Pro** (no open-source Mixxx mapping or public official MIDI implementation PDF found)
- **Roland DJ-707M** (Roland hosts its `[English] DJ-707M MIDI Implementation Chart` on [Roland DJ-707M Support](https://www.roland.com/global/support/by_product/dj-707m/owners_manuals/) behind an interactive EULA download wall, and no verified community mapping was found)

With Mixxx Android, you can also create your own basic mapping for any class-compliant MIDI controller via *Settings → DJ controllers → Learn controller*.

<!-- End of the text written autonomously by an AI agent -->
