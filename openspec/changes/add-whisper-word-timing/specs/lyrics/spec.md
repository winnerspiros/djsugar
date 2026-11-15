## ADDED Requirements

### Requirement: Word timing from the audio
For lyrics not timed by word, the Lyrics tab SHALL offer Time Words, which listens to the track's vocals with Whisper on the DJ's computer, lines the lyrics up with the words heard, and saves the lyrics with a time for each word as an `.lrc` beside the track. The model SHALL be downloaded only the first time it is needed and checked against a pinned hash. When too few of the words are heard, nothing SHALL be saved and the tab SHALL say so.

#### Scenario: Line-timed lyrics get word times
- **WHEN** the DJ presses Time Words for a track whose lyrics are timed by line
- **THEN** the tab shows the progress, and afterwards says the lyrics are timed by word, with the `.lrc` beside the track holding a time before each word

#### Scenario: Words Whisper missed
- **WHEN** Whisper hears "hold the line" and "on time" but not "love isn't always" between them
- **THEN** "love isn't always" are timed in order between "line" and "on", by their length

#### Scenario: Nothing heard
- **WHEN** Whisper hears less than a third of the words of the lyrics
- **THEN** the lyrics are left as they were and the tab says the words couldn't be made out

### Requirement: Transcription of tracks without lyrics
For a track with no lyrics, the Lyrics tab SHALL offer Transcribe, which writes down the words Whisper hears in the vocals as lyrics timed by word, grouped into lines at pauses and sentence ends, and saves them as an `.lrc` beside the track marked as transcribed by DJ Sugar. The tab SHALL show such lyrics as transcribed by DJ Sugar.

#### Scenario: A track with no lyrics
- **WHEN** the DJ presses Transcribe for a track with no lyrics and Whisper hears "Hold the line." then "Love isn't always on time"
- **THEN** the track gets two lines timed by word, and the tab says "Transcribed by DJ Sugar"

#### Scenario: Nothing sung
- **WHEN** Whisper hears no words in an instrumental track
- **THEN** nothing is saved and the tab says no singing was heard
