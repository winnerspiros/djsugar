## ADDED Requirements

### Requirement: First-run guided tour
On first start DJ Sugar SHALL run a guided tour through audio setup, controller setup (only when a controller is connected), the music folder and library imports, and simple playback and mixing; every step SHALL be skippable, and the tour SHALL not show again once finished or closed.

#### Scenario: Skip
- **WHEN** every step is skipped
- **THEN** DJ Sugar opens with defaults and the tour doesn't return

#### Scenario: No controller
- **WHEN** no controller is connected on first start
- **THEN** the controller step is left out

#### Scenario: Run it again
- **WHEN** the DJ chooses Take the tour in Settings › Help
- **THEN** the tour starts from the first step

#### Scenario: Steps wait for the DJ
- **WHEN** a mixing step points at a control
- **THEN** the control is ringed with a tip beside it, and the step goes on once the control is used, on screen or on a controller, or when the DJ skips it

#### Scenario: Upgrading
- **WHEN** a DJ who used an earlier DJ Sugar starts this one
- **THEN** the tour is offered once in a banner instead of starting
