# Proposal

Planned for: **v1.0 · Ready for any DJ switching over**

## Why

No other DJ app can be driven from an AI assistant. DJ Sugar's control names and library crate make a local MCP server straightforward.

## What Changes

- A local Model Context Protocol server, off by default.
- Tools: search the library, build and edit lists, tag tracks, load, play, cue and sync decks, set FX and loops, record, read what's playing.
- A library-only mode that can't touch the decks.

## Capabilities

### New Capabilities

- `mcp-server`: control DJ Sugar from an AI assistant over MCP

### Modified Capabilities

None.

## Out of Scope

- Cloud access.
- Voice control.

## Impact

New crate or app module; uses the control registry and library.
