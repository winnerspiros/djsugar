# Design

How each candidate setting was sorted. The source list is VirtualDJ's Options & Tweaks (about 330 options,
virtualdj.com/manuals/virtualdj/appendix/optionslist.html); the layer rules come from what Mixxx, rekordbox, Serato,
Traktor and djay Pro do well and badly.

## Rules

1. **Only choices are settings.** VirtualDJ's list mixes four kinds of thing: real behaviour choices (about half),
   window and panel state (sizes, columns, last folder, zoom), the DJ's own content stored as text (custom buttons,
   quick filters, FX lists, pad pages) and bookkeeping (tips seen, dialogs hidden). In DJ Sugar state is remembered
   quietly and reset with View › Reset layout; content is edited where it lives; bookkeeping is hidden behind Help ›
   Show all hints again. Only choices reach Settings or Advanced.
2. **No per-deck rows.** A per-deck choice is a button on the deck (Q, key lock, slip); Settings picks what it starts as.
3. **One home per option.** Each option is one row in one table (key, page, name, description, search words, default).
   Pages show their layer-2 rows; Advanced shows every row; Show modified filters either by "differs from default".
   That list is also exactly what a profile will save later.

## What the other apps do well

- **djay Pro**: very few settings, big toggles, everything applies at once. Most behaviour (auto gain, sync, key lock) is a
  button on the deck, not a preference.
- **Mixxx**: a Decks page holding all load and playback behaviour in one place: cue mode, where a track loads (main cue,
  intro start, first sound, beginning), tempo reset on load, key lock mode, speed range.
- **rekordbox**: Preferences › Advanced holds the deep tweaks, but common choices (Load Lock, Quantize, Instant Doubles,
  Slip) are buttons in the deck's header too, so they are found where they are used.
- **Serato**: DJ Preferences is one short page of plain-English switches ("Lock playing deck", "Instant doubles",
  "Play from start"). Easy to read, though too short for power users.
- **Traktor**: very detailed but split into 20+ tabs, so things are hard to find. The lesson: deep options need search.
- **VDJ**: the flat searchable list, **Show modified**, the ⊗ reset per row and the description line at the bottom are the
  good parts. Internal names (`autoBPMMatch`) as the only label, and state mixed in with preferences, are the bad parts.

Taken together, for DJ Sugar:

1. **Settings in context.** Every layer-2 choice that affects a deck is also on that control's right-click menu (as SYNC's
   quantize popover already is). Settings is where you find it, the control is where you use it.
2. **Plain names first, key second.** Advanced rows read "Echo tails ring out after the fader closes" with the stable key
   (`fx_post_fader`) shown small and grey, so forum posts and docs can name it exactly.
3. **Search finds by meaning.** "pitch" finds tempo, "PFL" finds headphones cue, "hamster" finds reverse crossfader.
   Each row carries a few search words.
4. **Show modified and Reset**, on every page and in Advanced, as in VDJ.
5. **One home per option.** A layer-2 choice also appears in Advanced (same row, same reset), so "Show modified" is
   the complete picture of how your DJ Sugar differs from the defaults. That list is also exactly what a profile saves.

Timecode (add-dvs, v0.6) and Broadcast (its own Settings page since add-live-broadcast) are no longer "not in
DJ Sugar". The map predates the Sources, Plugins, Broadcast and Keyboard pages; their rows join the option table in
`add-settings-table`.

## The map

Layer: **1** = no setting, **2** = Settings page, **3** = Advanced, **–** = not in DJ Sugar. **Has** = DJ Sugar already has it (v0.4.5).

### Loading a track (Settings › Decks › Loading)

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| resetEqOnLoad, resetFXOnLoad, resetGainOnLoad | **2**, on. **Has** ("Auto reset on load") | Keep as is |
| resetPitchOnLoad | **2**, "Tempo on load": Reset (default) / Keep | New. Mixxx and rekordbox have it |
| resetKeyOnLoad | **1**, always reset | A key shift on a new track is never what you want |
| resetStemsOnLoad | **1**, always reset (all stems back on) | |
| autoCue | **2**, "Load at": first hot cue or first sound (default) / main cue / start | Mixxx's "track load point" |
| autoGain | **2**, on. **Has** | |
| autoBPMMatch | **2**, "Match tempo on load": Off (default) / Close tempos (±10%) / Always | Off fits long blends; SYNC does the rest |
| autoKey | **2**. **Has** "Match key on load", default ±1 |  |
| keepPlayStatusOnLoadSong | **3**, off | |
| loadSecurity | **2**, on. **Has** ("Protect playing decks") | |
| autoHeadphones, pflOnSelect | **2**, "Headphones follow the deck you load", on | Being built separately |
| (rekordbox/Serato) instant doubles | **1**: loading a track that plays on another deck starts it at the same place, in step | New |

### Playing, cues and loops (Settings › Decks › Playing)

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| cueMode | **2**. **Has** "CUE button" | |
| playMode | **1**, play/pause | Stutter play is a pad mode, not a setting |
| hotcueMode | **2**, "Hot cue on a playing deck": Jump and keep playing (default) / Stutter | Worth offering |
| loopBackMode | **3**, loops start at the playhead | |
| loopDefault, loopRollDefault | **3**, 4 beats; 1/4 | Last size used stays per deck |
| smartLoop | **1**, on: loops are trimmed to the beat grid so they never click | |
| smartPlay | **3**, off | Play lands in step with the master deck. In DJ Sugar this is what Quantize does for play |
| smartCue, cueLoopAutoSync | **1**, follows Quantize | |
| quantizeLoop, quantizeSetCue, globalQuantize | **2**. **Has** "Quantize" (on) and the SYNC popover's 1 beat / 1 bar | |
| quantizeScratch, smartScratch | **3**, off | Arrive with improve-scratching |
| keepPlayingPastEnd | **1**, stops at the end | |
| endOfSongWarning | **2**, "Track end warning": 30 s (default) / 20 / 60 / off | rekordbox has the same |
| autoSortCues | **–** | Hot cues stay where the DJ puts them |
| vinylMode | **2**, "Jog touch": Scratch (default) / Nudge | Also on the controller's VINYL button |
| touchWheelBackspin, jogSensitivity* | **3** | |
| rampStopTime, rampStartTime | **3**, 0 (instant) | Brake is a pad FX, not a setting |

### Tempo, key and sync (Settings › Decks › Tempo and key)

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| pitchRange | **2**. **Has** "Tempo fader range" (±8%) | |
| masterTempo | **2**. **Has** "Key lock on at start" | |
| keyMatching | **2**. **Has** "Key mixing" (Harmonic/Fuzzy) | |
| keyDisplay | **2**. **Has** key notation (Camelot default) | |
| autoPitchLock | **3**, off | |
| autoBpmStabilizer* | **–** for now | Belongs with beatgrids (fluid grids) |
| autoPitchRange | **1**: SYNC widens the range when it has to | |
| pitchResetSpeed | **3**, 1% a second | |

### Mixer and FX (Settings › Mixer)

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| crossfaderDisable | **2**. **Has** "Crossfader" switch | First-run default stays on |
| crossfaderCurve, crossfaderHamster | **2**. **Has** | |
| levelfaderHamster, faderCurve | **3** | |
| eqMode | **2**, "EQ": Isolator (default) / Classic | Partly in Settings review |
| equalizerFrequencySpread | **3**, full kill | |
| equalizerLow/Mid/HighFrequency | **3**, DJ Sugar's own crossover points | Offer, keep out of sight |
| filterDefaultResonance | **3** | |
| fxProcessing | **2**, "Effect tails": ring out after the fader closes (post) / stop with the fader (pre) | Already planned in add-fx-pre-post-and-headphone. Default: post, as many club DJs expect |
| zeroDB | **3**, headroom | |
| effects, masterEffects, mixFx, fxLists | **1**: remembered from the FX chooser | Not a setting |
| skin3FxLayout | **1**: the FX bar layout | |
| splitHeadphones, equalizerInHeadphones, headphonesGain | **2**. **Has** "Split cue"; EQ in headphones **3** | |
| stemsBleed* | **3** | Only matters for fast stems |

### Waveforms and look (Settings › Interface, later)

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| skinWaveformType, coloredWaveforms, waveUseFrequency | **2**, "Waveform colours": frequency (default) / stems / one colour | |
| waveGrayOnKill | **1**, killed bands go grey | |
| waveformCenter, skinPlayheadShadow, showGridLines | **1** | These are just how DJ Sugar looks |
| rhythmZoom, scratchZoomVertical | **1**, remembered (state) | |
| vuMeter | **3**, peak + RMS | |
| clockDisplay, displayTime, cueDisplay | **2**, "Time shown": remaining (default) / elapsed; clock follows the system | |
| dialogsColorTheme | **–**: DJ Sugar is dark | |
| fontSize, browserPadding, browserTextFit | **2**, "Text size" (one setting for the whole app) and "Rows": compact / roomy | |
| skinFPS | **3**, follows the display | |
| touchScreenMode, skin, skinPanels, customButtons… | **–** or state | |

### Browser and library (Settings › Collection)

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| searchInFolder, searchInDB, searchInOnlineCatalogs, OnlineCatalogs | **1**: search shows this list, then the collection, then streaming (if signed in) | |
| quickFilters, colorRules | **1**: filter chips and saved searches. **Has** | |
| user1FieldName, user2FieldName | **2**. **Has** (Collection › Tags, tag groups) | Fields became tag groups (add-tag-groups) |
| favoriteGenres, favoriteTags | **1**: most used come first | |
| prelistenStartPos | **2**, "Preview starts at": 1/3 of the track (default) / middle / start |  |
| prelistenStopOnChange | **3**, off | With add-browser-preview-player |
| autoSearchDB, analyzeSongsOnView | **3**, off | |
| historyDelay, writeHistory | **3**, a track counts as played after 45 s | |
| setTagsAuto, getCommentFromTags | **2**, "Write changes to the files' tags": on | |
| useKeyFromTag, getCuesFromTags | **3** | |
| automaticDatabaseBackupPeriod, databaseBackupLocation | **2**, "Back up the collection": daily (default), folder | Worth having from day one |
| browserShortcuts, sideviewShortcuts, rootFoldersLocation | **1**: pinned folders in the sidebar | |
| cdjExport* | **3**, with Pioneer USB export | |

### Automix (Settings › Automix, or the Automix panel)

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| automixMode, fadeLength | **1**: the panel's length and style. **Has** | Remembered, not a setting |
| automixAutoRemovePlayed | **2**, "Remove played tracks from the queue": on | |
| automixRepeat | **1**, on the panel | |
| automixDualDeck | **1** | |
| autoMixBeatMatchOnFade | **3** | |
| automixSkipLength, automixMaxLength | **3** | |

### Recording, internet, sampler, controllers

| VDJ option | DJ Sugar layer and default | Notes |
|---|---|---|
| recordFile | **1**, the Mixed Sets folder; **2** "File name" with date and time | Folder settled in the 2026-11-04 Settings clean-up |
| recordFormat, recordQuality | **2**. **Has** WAV/FLAC | MP3 would need an encoder: separate change |
| recordWriteCueFile, recordAutoSplit | **3** | Tracklist file is useful for mixes |
| earlyAccessUpdates | **2**, "Get beta versions" | With the in-app updater |
| sendAnonymousStats, sendHistory, liveFeedback | **–**: DJ Sugar sends nothing | |
| netsearchAudioQuality, autoRefreshDRM | **–**: streaming is browse only | |
| samplerSpanAcrossDecks | **3** | |
| samplerOutputDeck, samplerHeadphones | **2**, with add-sample-packs | |
| controllerTakeoverMode | **2**, "Knobs that don't match the screen": pick up (default) / jump | DJ Sugar has pickup already |
| disableBuiltInDefinitions | **–** | |
| stemsSavedStems | **2**, with improve-stem-separation | |
| pitchQuality, scratchFilterQuality | **3**, best | |
| experimental* | **3**, "Try new things" group | Lets DJs try beta features without a build |
| DJSUGAR_* environment variables | **3** where a DJ might set them (music folder, waveform renderer, UI font) | Developer-only ones stay variables |

### Not in DJ Sugar

Video, karaoke, VDJ login and DRM, iRemote, OS2L/OSC
(belong to a lights add-on), AI prompt, skins.

## Decisions (2026-10-31)

- Match key on load stays at ±1 by default.
- Effect tails default to post-fader: they ring out after the fader closes.
- Planned for v0.5 at first; moved to v0.6 at the 0.5.0 release, then split (below).

## Decisions (2026-11-05)

- Grey help lines under row names move into the ⓘ popover where the name is clear enough.
- Show modified and the option table come first; Advanced later.
- New choices for v0.6: Hot cue on a playing deck, Track end warning, Back up the collection.
- Split into small changes: `add-settings-table`, `add-settings-deck-choices`, then the rest here.
- Vinyl mode stays one on/off switch (it only matters for touch-sensing jogs).
