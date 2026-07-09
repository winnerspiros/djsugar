#pragma once

// TODO: unify this with the invalid interfacenumber from the bulkenumerator
constexpr static int kInvalidInterfaceNumber = -1;
constexpr static unsigned short kAnyValue = 0x0;

struct hid_denylist_t {
    unsigned short vendor_id;
    unsigned short product_id;
    unsigned short usage_page;
    unsigned short usage;
    int interface_number = kInvalidInterfaceNumber;
};

/// USB HID device that should not be recognized as controllers
constexpr static hid_denylist_t kHidDenyList[] = {
        {0x1157, 0x300, 0x1, 0x2},                  // EKS Otus mouse pad (OS/X,windows)
        {0x1157, 0x300, kAnyValue, kAnyValue, 0x3}, // EKS Otus mouse pad (linux)
        {0x04f3, 0x2d26, kAnyValue, kAnyValue},     // ELAN2D26:00 Touch screen
        {0x046d, 0xc539, kAnyValue, kAnyValue},     // Logitech G Pro Wireless
        // DDJ-FLX4 BLE chip (2B73:0045 interface 6). The BLE chip exposes
        // two HID interfaces (5 & 6) but only interface 5 carries the DJ
        // controller HID data. Interface 6 is a duplicate non-functional
        // interface that creates a second useless controller entry.
        // Bluetooth discovery is handled separately via the dedicated
        // Bluetooth scan button.
        {0x2b73, 0x0045, kAnyValue, kAnyValue, 0x6},
        // The following rules have been created using the official USB HID page
        // spec as specified at https://usb.org/sites/default/files/hut1_4.pdf
        {
                kAnyValue,
                kAnyValue,
                0x0D,
                0x04,
        }, // Touch Screen
        {
                kAnyValue,
                kAnyValue,
                0x0D,
                0x22,
        }, // Finger
};
