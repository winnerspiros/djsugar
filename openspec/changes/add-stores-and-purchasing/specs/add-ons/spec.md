## ADDED Requirements

### Requirement: Add-ons page
Settings › Add-ons SHALL list installed add-ons (name, version, repository, on/off, Update, Remove), available ones (description, hosts, publisher, Install) and repositories (official plus added ones).

#### Scenario: Install
- **WHEN** Install is pressed on an available add-on
- **THEN** it downloads, its checksum is verified, and its source or settings page appears without a rebuild

### Requirement: Repositories
Add repository… SHALL take a URL and an optional token; an added repository SHALL be marked unsupported and its add-ons SHALL show a warning before installing. Tokens SHALL be kept in the system keychain.

#### Scenario: Private repo
- **WHEN** a private GitHub repository is added with a read-only token
- **THEN** its add-ons are listed and the token is not in settings.json

### Requirement: Integrity
A download whose checksum doesn't match SHALL NOT be installed. A repository's signing key, when it has one, SHALL be pinned when added and checked on every update.

#### Scenario: Tampered index
- **WHEN** a pinned repository's index is signed by another key
- **THEN** DJ Sugar refuses its updates and says why

### Requirement: Updates
Add-on updates SHALL be checked at start and at most once a day, shown as a dot on Settings, and never installed while a deck plays.

#### Scenario: Update waits
- **WHEN** an update is available while a deck plays
- **THEN** it is offered but not installed until the DJ chooses and the decks stop

### Requirement: Crash recovery
A crashed add-on SHALL be restarted with a growing delay and its source SHALL show that it stopped and why.

#### Scenario: Crash
- **WHEN** an add-on exits unexpectedly
- **THEN** its source shows it stopped and DJ Sugar starts it again after a delay
