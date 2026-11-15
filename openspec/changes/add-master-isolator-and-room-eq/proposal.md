# Proposal

Planned for: **v0.8 · From the booth to the studio** (2026-11-14, a user request after a review of parametric EQ and live dynamics in DJ tools)

## Why

DJ Sugar's master has a level, a balance and a true-peak limiter, and nothing that shapes its sound. Two needs come up in DJ tools and venues:

- **Riding the whole mix.** Pioneer's DJM-V10 has a three-band Isolator on the master, used by long-mix house and disco DJs to take the low end out of everything for a breakdown and bring it back. DJ Sugar can only do this deck by deck.
- **Fixing the room.** Many venues can't tune their system at soundcheck, which leaves muddy bass or harsh highs. djay Pro users load an EQ plugin in its Main Out Effect slot for this, and VirtualDJ users ask for a master EQ. In DJ Sugar the same needs a plugin in a master FX slot, which is easy to switch off by mistake and also ends up in recordings. Booth monitors have the same problem with no fix at all.

Mastered tracks don't need a parametric EQ, so this change puts one only where the room is corrected, and keeps the live control to three knobs.

## What Changes

1. **Master Isolator** in the MASTER panel: LOW, MID and HIGH knobs over the whole mix, flat at the centre, the band killed fully left, up to +6 dB right. A Flat button resets them. An ISO tag lights on the MASTER button in the top bar while any knob is off centre. Mappable as `[Master],iso_low`, `iso_mid` and `iso_high`.
2. **Room EQ** in Settings › Audio: a six-band EQ (low cut, low shelf, two bells, high shelf, high cut) drawn as a curve with a dot per band. Drag a dot for frequency and gain, scroll on it for width, double-click to reset. Separate curves for the main output and the booth, an On switch for each, and an optional live spectrum behind the curve.
3. **Room EQ presets:** Flat, Muddy room, Harsh highs and Small speakers built in, and the DJ's own, named per venue. The MASTER panel shows the preset in use with an Edit… link.
4. **BOOTH EQ** in the MASTER panel: LOW and HIGH knobs, shown only while a booth output is set up. They are the booth curve's low and high shelves, so the knobs and the curve always agree.
5. **Recordings stay clean.** Recording, broadcasting and the loudness meter take the mix after the Isolator and before Room EQ, so a fix for one room never ends up in a recording. The headphones don't get Room EQ either.


## Capabilities

### Modified Capabilities

- `mixer`: a master Isolator and booth EQ in the MASTER panel, Room EQ on the main and booth outputs, and recordings taken before Room EQ

## Out of Scope

- A compressor on decks or inputs, and a parametric EQ as an effect: plugins such as ZL Equalizer 2 already load in any FX slot.
- Automatic room measurement with a microphone.
- A four-band channel EQ.

## Impact

`crates/engine` dsp.rs (shelf filters, a Room EQ with smoothed coefficients, the Isolator with boost), engine.rs (the master chain, the recording tap and its own limiter), controls.rs; `crates/app` MASTER panel, a Room EQ page in Settings › Audio with a curve widget, presets in settings; controller catalog for the `[Master]` keys. User guide page in `website/book/src/guide`.

## Difficulty

**Medium: about six PRs, 5 to 7 days.** The filters already exist in `dsp.rs`; most of the work is the curve widget and moving the recording tap without changing how recordings sound when Room EQ is off.

## Performance

A three-band Isolator and up to twelve biquads per output, only while they're not flat: well under 1% of a core. Room EQ off or flat costs nothing.
