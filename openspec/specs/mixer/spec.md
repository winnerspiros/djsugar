# mixer Specification

## Purpose
The mixer between the decks: each channel's gain, EQ, filter and fader, the knob modes (EQ, stems, sculpt), the crossfader, headphones, master and booth, auto gain and the master limiter.

## Requirements

### Requirement: Channel strip
Each deck's channel SHALL have gain (`pregain`), a 3-band EQ (high, mid, low) with kills reachable from controllers, a bipolar filter knob (`[QuickEffectRack1_[ChannelN]],super1`) playing the chosen Color FX, by default the filter (low-pass left, high-pass right), a channel fader (`volume`), a headphone cue button (`pfl`) and a stereo level meter with peak indicator.

#### Scenario: Low kill
- **WHEN** a channel's low EQ kill is turned on from a controller
- **THEN** the bass of that deck is removed from every mix

#### Scenario: Filter centre is flat
- **WHEN** the filter knob is at its centre
- **THEN** the channel is unfiltered

#### Scenario: Filter moves without clicks
- **WHEN** the filter knob is turned, fast or in big controller steps, including across the centre
- **THEN** the cutoff glides over about 5 ms and each side fades in from the centre, so the sweep has no zipper noise or clicks

### Requirement: Color FX
Each channel's filter knob SHALL play the chosen Color FX, off at its centre: Filter (low-pass left, high-pass right), Space (filtered reverb send), Dub Echo (filtered half-beat echo send), Noise (filtered white noise) or Crush (bit and sample-rate reduction). Each deck SHALL have its own Color FX, as `[QuickEffectRack1_[ChannelN]],loaded_chain_preset` (1 Filter to 5 Crush); setting it, or stepping it with `next_chain_preset` / `prev_chain_preset`, SHALL change only that deck's. A small ▾ box beside each filter knob, on its inner side, SHALL open a COLOR FX list to pick one at a time, the knob's label SHALL name it, and SHIFT + SMART CFX on the DDJ-FLX4 SHALL step both decks to their next one.

#### Scenario: Space tail
- **WHEN** Space is chosen, the filter knob is turned right and then back to the centre
- **THEN** the reverb rings on and fades instead of stopping

#### Scenario: FLX4 picks the Color FX
- **WHEN** SHIFT + SMART CFX is pressed on a DDJ-FLX4 while Filter is chosen
- **THEN** decks 1 and 2's filter knobs play Space and their labels show SPACE

#### Scenario: Each deck picks its own
- **WHEN** Crush is picked from the ▾ beside deck 2's filter knob
- **THEN** deck 2's knob plays Crush and deck 1's keeps its Color FX

### Requirement: Knob modes
The mixer's side knobs SHALL switch between EQ (high, mid, low, filter), STEMS (hi-hat, vocal, kick and bass stem volumes) and SCULPT (Model 1 style LPF, swept bell FREQ and CUT/BOOST, HPF). Controller EQ knobs SHALL follow the chosen mode through `[Mixer],knob_mode`.

#### Scenario: Stems mode on a controller
- **WHEN** the mixer is in STEMS mode and a controller's low EQ knob turns
- **THEN** that deck's bass stem volume changes

#### Scenario: Stems mode without stems
- **WHEN** the mixer is in STEMS mode and a deck's track has no stems
- **THEN** that deck's knobs are dimmed

### Requirement: Crossfader
The crossfader SHALL blend the left and right sides with the curve chosen in Settings › Mixer: Smooth (constant power, default), Linear, or Scratch (both sides full until the last 5 % of travel). "Reverse crossfader" SHALL swap its sides (`[Mixer Profile],xFaderReverse`).

#### Scenario: Scratch curve
- **WHEN** the curve is Scratch and the crossfader is moved 10 % from the left end
- **THEN** both sides are at full level

### Requirement: Crossfader assignment
Each deck SHALL have a crossfader side, LEFT, THRU or RIGHT (`orientation` 0, 1, 2). Decks 1 and 3 SHALL start on the left and 2 and 4 on the right. In 4-Deck, clicking a deck's label SHALL change it.

#### Scenario: Thru ignores the crossfader
- **WHEN** deck 3 is set to THRU and the crossfader is fully right
- **THEN** deck 3 is still heard in the master

### Requirement: Headphones
The headphone (cue) mix SHALL carry every cued channel. CUE MIX (`headMix`) SHALL blend cue and master with constant power, equal at the centre; CUE LEVEL (`headGain`) sets its level. "Split cue" SHALL put cue in the left ear and master in the right, both mono.

#### Scenario: Split cue
- **WHEN** split cue is on and deck 2 is cued
- **THEN** the left ear has deck 2 and the right has the master, each in mono

### Requirement: Master and booth
MASTER (`[Master],gain`), BOOTH (`booth_gain`) and balance SHALL set the master and booth levels. MASTER, PHONES and BOOTH levels SHALL be unipolar: 0 % fully left, no centre detent.

### Requirement: Remembered levels
MASTER, PHONES (`headGain`), CUE MIX and BOOTH SHALL be saved as the DJ leaves them and put back at the next start. The first time, MASTER SHALL start at 50 % of its travel, PHONES at 25 % and CUE MIX fully on the cue side; BOOTH at 100 %.

#### Scenario: First start
- **WHEN** DJ Sugar starts with no saved levels
- **THEN** MASTER is at 50 %, PHONES at 25 % and the headphones hear only the cue

#### Scenario: Next start
- **WHEN** the DJ set MASTER to 70 % and quits
- **THEN** MASTER is at 70 % at the next start

#### Scenario: Booth independent
- **WHEN** BOOTH is turned down
- **THEN** the booth output gets quieter and the master output does not change

### Requirement: Auto gain
With "Auto gain" on (Settings › Mixer), each deck SHALL be levelled to the chosen loudness (-8 to -23 LUFS, default -14) from its EBU R128 measurement, with boost capped so peaks stay under the limiter ceiling; a playing deck glides to a new gain at 6 dB/s. The gain knob SHALL trim on top. Applied gain shows in `[ChannelN],replaygain`.

#### Scenario: Quiet track levelled
- **WHEN** auto gain is on at -14 LUFS and a track measured at -20 LUFS loads
- **THEN** the deck is raised by about 6 dB unless that would push its peaks over the ceiling

### Requirement: Track gain
A gain stored with a track (from the Info panel or an import) SHALL set the deck's gain when it loads.

#### Scenario: Stored gain
- **WHEN** a track with a stored gain of -2 dB loads
- **THEN** the deck's gain knob is at -2 dB

### Requirement: Master limiter
The master SHALL pass through a look-ahead, stereo-linked peak limiter with a ceiling of -0.5 dBFS that leaves quieter audio untouched.

#### Scenario: No clipping
- **WHEN** two loud decks play with every fader up
- **THEN** the master output never exceeds -0.5 dBFS

### Requirement: MASTER panel
The centre mixer SHALL have no tabs: the channels and the crossfader only, with the middle of its top row free for the effects. A click on MASTER in the top bar SHALL open a panel with horizontal sliders for Master, Phones, Booth (only while a booth output is set up) and Cue / Mix, plus Ableton Link; a click outside closes it. Cue / Mix SHALL have Cue on the left, Mix on the right and a detent at the centre. Audio routing SHALL live only in Settings › Audio.

#### Scenario: Cue / Mix detent
- **WHEN** the Cue / Mix slider is dragged to within 3 % of the centre
- **THEN** it rests at the centre

#### Scenario: No booth output
- **WHEN** no output route has the booth role
- **THEN** the MASTER panel shows no Booth slider

