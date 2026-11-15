# Design

## Decisions (2026-10-27)

- Mixxx mappings are only read in. A DJ's own mapping is an DJ Sugar file, `<config>/controller-mappings/<id>.json`: its name, the Mixxx mapping it is `based_on`, and a list of `changes` made on top of it (bindings replaced or added, options, LEDs, DJ Sugar options such as value-scaling takeover, ranges and comments). The original's scripts and settings keep running underneath, so the format is a superset of Mixxx's without copying it, and Reload Original is emptying `changes`. Which one a controller uses is the setting `controller_versions` (Mixxx mapping file → own mapping id).
- Duplicate and rename instead of per-row overrides; one mapping per controller at a time; Reload Original always available.
- Mapping is done when not playing, in the large editor window; Map mode shows the main window behind it.
