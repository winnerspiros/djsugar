## REMOVED Requirements

### Requirement: HID not supported
**Reason**: HID controllers are supported now.
**Migration**: none.

## ADDED Requirements

### Requirement: HID controllers
DJ Sugar SHALL run Mixxx's HID mappings (`.hid.xml`) for the USB HID controllers they name, picked by vendor and product ID (and USB interface, or usage page and usage), with Mixxx's HID script API. HID controllers SHALL be listed in Settings › Controllers, where they can be turned off, and SHALL NOT be offered for editing, Map mode or Quick map.

#### Scenario: Plug in a Traktor Kontrol S2 MK3
- **WHEN** a Traktor Kontrol S2 MK3 is connected
- **THEN** it runs Mixxx's S2 MK3 HID mapping without setup, is marked Beta in Settings › Controllers, and its lights follow the decks

#### Scenario: Device won't open
- **WHEN** the system refuses to open an HID controller (no udev rule on Linux, another app holding it)
- **THEN** Settings › Controllers says why, and DJ Sugar tries again once it is plugged in again or Refresh is pressed

#### Scenario: Output reports
- **WHEN** a script sends an output report identical to the last one sent with that ID
- **THEN** it is not written to the device again, unless sent with `useNonSkippingFIFO`
