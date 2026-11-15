# Proposal

Planned for: **v0.6 · Buy it from the artist** (a user request 2026-11-10; issue #554)

## Why

DJ Sugar now has several layouts (Player, 2-Deck, 2-Deck Pro, 4-Deck, and Prep to come), and more will follow. Not every DJ needs all of them: someone who only plays Player, or only mixes on 2-Deck Pro, scrolls past the rest every time. VirtualDJ and Mixxx answer this with skins; their pickers list every skin at once and can't take one out of the switcher. This change is the first step towards "DJ Sugar Layouts": the DJ keeps a list of the layouts they use, and only those appear in the layout menu.

## What Changes

- **Settings › Layouts**, a new page. At the top, **Your layouts**: the layouts the layout menu offers, in its order, each with a small schematic preview. Arrows move a layout up or down; **−** takes it out. The layout in use can't be taken out, so the list is never empty.
- Under it, **More layouts**: a card for each layout not in the list, with a larger preview, a line on what it's for and **+ Add**. Clicking any card or row switches to that layout at once.
- Previews are drawn in the app from the theme's colors, so they follow the look and never go stale like screenshots.
- The **layout menu** at the top left lists only the DJ's layouts. A small cog at the top right of the menu opens Settings › Layouts.
- A controller's **VIEW** (`[Skin],layout`, such as SHIFT + the right LOAD on the DDJ-FLX4) steps to the next layout in the list, round to the first, instead of toggling 2-Deck Pro. A mapping or the tour that sets a layout by number still reaches any layout.
- The list is a setting (`layouts`, layout numbers in order). At first it holds Player, 2-Deck, 2-Deck Pro and 4-Deck. Layouts added to DJ Sugar later start under More layouts.
- The **Prep** layout (add-prep-layout), built and hidden since v0.5, is offered from here on, under More layouts: not in the starting list, so the DJ adds it.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: the layout menu lists the DJ's layouts; Settings › Layouts; VIEW steps through the list

## Out of Scope

- Color schemes, show or hide parts of a layout (add-layout-customization), layouts the DJ builds from DJ Sugar's parts, and skin files loaded at runtime. These are later steps of DJ Sugar Layouts.

## Difficulty

**Small: one PR.** A setting, a Settings page with a new row kind, a filter in the menu, and the controller step.

## Performance

None: the list is read when the menu, the page or VIEW needs it.
