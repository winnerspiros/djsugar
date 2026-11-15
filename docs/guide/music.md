# Your collection

Your **collection** is every track DJ Sugar knows about, with everything it
knows about them: one database file with every track's tags, rating,
color, cues, loops and beat grid, your playlists, smart lists and folders,
and the history of what you played. It points at your audio files where
they are, and only moves them between your stage folders when you turn that
on (see [Stages](#stages-inbox-in-prep-and-play-ready)).

**Collection** is the top of the tree on the left, and the first source in
the rail. Together with Files and the streaming services it makes up the
**browser**, the lower half of the screen.

## Sources

The rail at the far left picks where the tree and the track list come from:

- **Collection**: all your tracks, with their stages, crates, sets, playlists
  and history.
- **Files**: any folder on your computer, as it is on disk.
- **Spotify**, **TIDAL**, **Apple Music**, **SoundCloud** and
  **Mixcloud**: your playlists and saved tracks on those services, for
  browsing (see [Streaming services](#streaming-services)).
- **Jam**: the Groove Box, your instrument plugins, the Mic and the
  Mixer (your aux inputs), dragged onto a deck to play there instead of a track (see
  [Jam on a deck](decks.md#jam-on-a-deck)).
- **Plex**, **Navidrome** and **Jellyfin**: your own music server, searched from DJ Sugar
  with its tracks downloaded into your collection (see
  [Your own music server](#your-own-music-server)).
- **Bandcamp**, **Beatport** and **OneLibrary**: coming soon.

**Settings › Sources** lists every source by kind (streaming services,
stores, your own servers, free music, add-ons and the built-in ones). Each
row says whether the source is signed in or set up, and its **In rail**
switch shows or hides it in the rail, as right-clicking the rail does. Click
a source to open its settings: its server and sign-in, the folder its
downloads go to, or **Sign out** for a streaming service. The cache for
tracks played from your own servers is at the top of the page: one size
for all of them (10 GB unless you pick another); **Manage** under
**Settings › Collection › Storage** clears it.

### Streaming services

DJ Sugar shows what you saved on a streaming service, but doesn't play its
audio: a track plays on a deck when your collection holds the same track,
and the others are greyed out. Double-click one to open it on the service's
own site; SoundCloud tracks and Mixcloud shows also have a **▶** button
(and **Play on…** when you right-click) that plays them there in your
browser. Tracks are matched by ISRC when both sides have one, then by
artist and title. If a match is wrong, right-click the track and pick
**Change Match…**.

To keep track of songs you mean to get, right-click a greyed-out track
(or several picked ones) and pick **Add to Wanted**. Right-click a
playlist and pick **Add to Wanted** to use it as a shopping list: its
tracks you don't have go on the Wanted list, and so does anything you add
to it later, each time DJ Sugar signs in or you open it. Tracks you take off
the Wanted list stay off. **Stop Adding to Wanted** leaves what's there.
With a download add-on such as the Soulseek add-on installed, **Add to
Downloads** sits beside it and searches the add-on for each track.

A download add-on's panel lists your **Searches**, then **Downloads**
(everything it is fetching and has fetched, with cancel, retry and clear),
then any page of the add-on's own, such as the Soulseek add-on's
**Sharing**: what you share, and uploads now, waiting and done lately.

Under the playlists, **Signed in as** shows your account; click it to
**Sign out**.

Each service is signed in to with your own app, so DJ Sugar ships no keys:

- **Spotify**: create an app at developer.spotify.com with the redirect URI
  `http://127.0.0.1:8898/callback` and paste its client ID.
- **TIDAL**: create an app at developer.tidal.com with the same redirect
  URI and the scopes `user.read`, `playlists.read` and `collection.read`,
  and paste its client ID.
- **Apple Music**: needs an Apple Developer account. Under **Certificates,
  Identifiers & Profiles › Keys**, create a key with **MusicKit** and
  download its `.p8` file. Enter your Team ID, the key's Key ID and the
  file's path, then sign in with your Apple ID on the page that opens. You
  see your library's songs and playlists.
- **SoundCloud**: register an app at soundcloud.com/you/apps with the same
  redirect URI and paste its client ID and secret. You see your Likes, your
  own tracks and your playlists.
- **Mixcloud**: just type your username. You see your shows, favourites,
  listening history and playlists; shows play on mixcloud.com.

DJ Sugar keeps your playlists and the tracks of those you opened, so after a
restart they show straight away. A playlist you open is checked again in
the background and updated if it changed (Spotify, TIDAL, SoundCloud and
Apple Music say when a playlist changed, so unchanged ones aren't fetched
again at all). The refresh button at the top right of the side panel,
beside the service's name, fetches the list of playlists again. Signing out forgets what was kept.

### Your own music server

If your music lives on a **Plex Media Server**, on **Jellyfin** (or Emby),
or on **Navidrome** (or any other server that speaks the Subsonic API, such
as Gonic, Airsonic, Ampache or LMS), DJ Sugar can search it and bring tracks into your collection. These
are your own files, so unlike the streaming services they play on decks.

Pick the source in the rail: until it's set up, its panel asks for the
server and sign-in, and **Connect** saves them. You can also fill them in
later in **Settings › Sources**: click **Plex**, **Jellyfin** or
**Navidrome** to open its settings.

- **Navidrome**: the server's address (for example
  `http://192.168.1.20:4533`), your username and your password. The
  password is only ever sent as a salted token.
- **Jellyfin** (or Emby): the server's address (for example
  `http://192.168.1.20:8096`), your username and your password. DJ Sugar signs
  in once and then uses the server's access token.
- **Plex**: the server's address (for example `http://192.168.1.20:32400`)
  and your Plex token. To find the token, open any track's **⋯** menu in
  Plex Web, pick **Get Info › View XML**, and copy the `X-Plex-Token` at
  the end of the address that opens.

Then pick the source in the rail, type in the search bar at the top and
press Return. The cog next to it sets what to search for: **Auto** finds
albums and songs, **Track**, **Album** and **Artist** narrow it down (Artist
lists that artist's albums), and **Playlist** lists your server's playlists
whose name has the text in it (leave it as `*` for all of them). Open an
album or playlist to see its songs.

Each song, album and playlist offers two ways to bring it in:

- **Play** (double-click a song, or its **▶**) loads it onto a deck that
  isn't playing, fetching it into the cache first. A FLAC on your home
  network takes a second or two.
- The **cloud** downloads it into the cache, to play later or offline. A
  filled cloud means it's there. Cached tracks stay out of your collection,
  and the oldest go when the cache is full (10 GB for all sources together
  unless you pick another size in **Settings › Sources**). Their waveform, tempo and key are kept as
  long as the file stays in the cache.
- **+ Collection** (or **+ Album**) brings it into your collection for good:
  `Downloaded/Plex`, `Downloaded/Jellyfin` or `Downloaded/Navidrome` in your
  music folder, one
  folder per artist and album with its cover, or the **Collection folder**
  set for the source in **Settings › Sources**. A song already in the cache moves over from
  there instead of downloading again.

Tick several rows to **Download** them to the cache or **Add to Collection**
together. DJ Sugar fetches the original files, so a FLAC stays a FLAC.

## How tracks come in

Tracks come into your collection in three ways:

- **Your music folders.** DJ Sugar looks through them each time it starts,
  and when you press the circling arrows at the top right of the Collection
  sidebar. New files are added, and changed files have their tags read
  again. Add as many folders as you like under **Music Collection** in
  **Settings › Collection** with **+** (pick the folder in Finder or your
  file manager), and take one out by selecting it and pressing **−**, or start DJ Sugar with the
  `DJSUGAR_MUSIC` environment variable set to one. There's no default folder:
  `~/Music` is where Ableton, GarageBand and other apps keep their samples,
  so DJ Sugar only looks there if you add it. When a folder is no longer looked through,
  its tracks leave your collection, except ones you rated, played, staged or put
  in a list. The files stay where they are.
- **Files.** The **Files** source in the rail browses any folder on your
  computer. Load or drag a track from there to add it.
- **Another DJ app.** DJ Sugar can bring in your VirtualDJ library (see below).

Bringing a track in only reads its tags: title, artist, album, genre, year,
key, BPM, rating, comment and cover art. It is quick, and nothing is played
or analysed yet.

## Stages: Inbox, In Prep and Play Ready

Stages take new music from wherever it lands to tracks that are ready to
play. Set them up in **Settings › Collection › Stages**:

- **Inbox**: the folder where new music arrives, such as Downloads, a
  Soulseek folder, or where purchases land.
- **In Prep**: where tracks you are preparing are kept. Its
  subfolders are **prep crates** (for example `Yoga STG` or
  `Journey STG`), for sorting tracks by where they'll go.
- **Play Ready**: where prepared tracks are kept. Its subfolders
  (such as `Albums`) stay as they are.

Once the In Prep and Play Ready folders are set, **Inbox**, **In Prep**
and **Play Ready** show under **Collection** at the top of the tree, after
**Wanted**, each with its track count and subfolders. Every track gets the
stage of the folder it is in.

Right-click tracks to move them along, or drag them onto a stage. The
track menu shows **Inbox › In Prep › Play Ready** with the track's stage
lit; click another stage to move it there, or use the lines below it, whose
arrows say whether a track goes back or forward:

1. **Send to In Prep** (or **Send to In Prep › a crate**) for inbox
   tracks worth keeping. Tracks you don't want go to
   [Quarantine](collection-health.md#quarantine).
2. In In Prep, prepare each track: tags, cover, rating, tempo and key. Show
   the **Prep** column (right-click a column heading) for a dot each for
   tags, rating, and tempo and key; tracks in In Prep are analysed in the
   background, so tempo and key fill in on their own. The In Prep list has
   its own columns, to see what each track still needs: **Grid** (✓ locked,
   ✓ edited, ? check when the analysis wasn't sure, the number of tempo
   sections, or detected), **Cues** (how many hot cues), **Tags** (✓ with a
   genre and at least one of your tags) and **Cover**. Pick other columns
   from the header's menu as in any list; In Prep keeps its own set.
3. **Send to Play Ready** when it's ready. A track from a prep crate also
   joins the list named after the crate (`Yoga STG` goes to **Yoga**).

**Back to In Prep** and **Back to Inbox** move a track back a stage, and
**⌘Z** (Ctrl+Z on Windows and Linux), or **Edit › Undo** on a Mac, takes
back the last change.

Files only move between the folders when **Move files** is on. Leave it
off while another app, such as VirtualDJ, still uses them: tracks change
stage in DJ Sugar at once, their moves wait, and they are made when you turn
it on. Files going from the inbox to In Prep are renamed `Artist - Title
(Remix)` from their tags, unless you turn **Rename files** off. A file is never moved over another one.

Search with `stage:inbox`, `stage:prep` or `stage:playready` (the older
`stage:triage` and `stage:collection` work too), and
`is:prepped` for tracks with tags, rating, tempo and key.

## When tracks are analysed

Tracks in In Prep are analysed in the background. Any other track is
analysed the first time it is loaded on a deck: its waveform,
tempo and beat grid, and its key when the tags don't have one. It plays
while that happens, and the waveform fills in as it goes. The result is
saved, so the track loads at once next time.

**Settings › Collection › Storage** shows how
the disk with your music is used: your music, the analysis results, high
quality stems, tracks played from your servers, everything else, and how
much is free. **Manage** clears the analysis results, the stems or the
played tracks, each after saying what it costs, and **Show cache folder**
opens the folder they are kept in.

Grids, keys and cues brought in from VirtualDJ are used as they are, so
those tracks don't wait for analysis.

The beat grid stays at one steady tempo unless the track really drifts,
as live drums and older recordings do; only then does it get tempo
sections, and its BPM in the list shows a ~ (as in VirtualDJ). With
**Beat detection** set to **Neural** (the default, in
**Settings › Analysis**), DJ Sugar then listens for each beat and downbeat
with Beat This!, so bar 1 starts on a downbeat. That takes a few seconds
more, after the waveform appears: the track plays on its first grid
meanwhile. The first time, DJ Sugar downloads the model (about 11 MB).

Beat This! also sets the tempo, from the beats it hears, which suits slow
and soft music (ambient, acoustic) far better than the standard detector.
Tempos are kept within **BPM range** (**Settings › Analysis**, 60 to 180
BPM unless you change it): a faster reading is halved, a slower one
doubled.

The grid menu (click the BPM on a deck) says where the grid came from:
**Detected**, **Edited** or the app it was imported from. A detected grid
that fits the beats poorly says **may be off, check by ear**. **Lock grid**
keeps a grid as it is: a locked grid can't be edited, and analysing the
track again leaves it alone. Grids imported from another app come in
locked. **Back to the detected grid** throws away your edits or the
imported grid and uses DJ Sugar's own.

To check a grid by ear, turn on **Metronome** in the same menu: the deck
clicks on every beat of the grid, higher on bar 1, through the deck's
fader and cue like the music. If the clicks drift off the music, fix the
grid while it plays: **Bar 1 here** on a downbeat puts bar 1 there,
**Move grid** ‹ › nudges it, **Tempo here** − + changes the tempo,
**Tempo × ½ / × 2** fixes a tempo read at half or double, **Tap tempo**
sets it from your taps, and **New tempo section
from here** starts a new tempo where a live track speeds up or slows down.

### Edit Grid

The quickest way to fix a grid is the pencil in the bottom corner of each
deck's big waveform (left for decks 1 and 3, right for 2 and 4). It stays
dimmed until you point at it. Click it, or press **G** for the master
deck, to start editing: the beat lines run through the waveform, bar lines
in lilac, and the pencil lights up. While you edit:

- **Drag the waveform** to move the grid under the music; the audio slides
  and the beat lines stay put until you let go. Hold **Alt** (⌥) to scrub as
  usual instead.
- **Set Downbeat** in the Edit Grid popover makes the beat at the play line
  bar 1, and **◀ |** **| ▶** nudge the whole grid a little earlier or later.
- **More…** opens the full grid menu: tempo, ½ and ×2, tempo sections,
  undo and the metronome.
- **Reset** goes back to the detected grid; **Lock** locks it.

Click the pencil again, press **Done** or **Esc** to finish. Editing keeps
whatever zoom the waveform is at, and loading another track finishes it.
In the [Prep layout](layouts.md) the grid is always open for editing while
it is unlocked, and the metronome turns on by itself there and back off
once the grid is locked. Controllers and keyboard shortcuts can use
`edit_grid` (the `beats_edit` control).

To analyse tracks yourself, right-click them and choose **Analyze Track**
(or **Analyze 12 Tracks** when you picked several), or right-click a
list, folder or stage and choose **Analyze All**. They are analysed again
from scratch in the background, one at a time: tempo, beat grid, key,
loudness, mini waveform and phrases. A grid you edited and a key you typed
in or got from the tags stay as they are. While it runs, each waiting
track has a gray dot by its title (lilac and pulsing while it is being
analysed), and the footer shows how far along it is, with a ✕ to stop.

A track with no cover also gets one: DJ Sugar takes the first cover it finds
online, if it is at least 600 pixels, and saves it in the file's tags. When
the first one is smaller the track is left without, so you can pick one
yourself from the cover dialog in **Info**.

## Phrases

Once a track has a beat grid, DJ Sugar splits it into phrases of whole
bars: intro, build (up), drop (peak), chorus (main groove), verse, break
and outro. Each deck's overview shows them as a bar under the minute dots:
deep blue for the intro and outro, violet for a build, rose for a drop,
green for a chorus, gold for a verse and tangerine for a break. Phrases
are 8 to 32 bars long, so a short fill never gets one of its own.
DJ Sugar listens for changes in the bass, the level and the chords, so a
new part starts where the music changes even when it doesn't get louder,
and it matches parts that come back: a second verse is a verse too. They are saved in your
collection with the track, and move with the grid when you edit it.

Rest the pointer on a deck's overview to see the phrase under it: its
kind, how many bars it runs and where it starts and ends.

The scrolling waveforms flag where each phrase starts: a line in the
phrase's color and a small tab with its name (INTRO, BUILD, DROP and so on)
just under the bar numbers. Between flags the waveform is left as it is. In
4-Deck, where the waveforms are thin, only the lines show.

Pick which show with **Phrases** in the drop-down on the waveforms button
in the top bar: **Overview** for the bars, **Waveform** for the flags,
**Both** or **Off**. It applies to every deck.

## Bringing in a VirtualDJ library

Press **Set up…** next to **Import from VirtualDJ** in **Settings ›
Collection › Import**. That tab is always there and lists every DJ app DJ Sugar
can bring a library in from; Serato DJ is marked coming soon.
Three steps follow:

1. **Find your library.** DJ Sugar looks for `database.xml` in
   `~/Library/Application Support/VirtualDJ` (or `~/.vdj` on Linux) and
   shows what it holds. Choose another file if yours is elsewhere.
2. **Choose which lists come across.** Every playlist, virtual folder,
   filter folder (as a smart list) and history session is in a tree with a
   tick box. Ticking a folder ticks everything in it, and **All lists** at
   the top does the lot. History is grouped by year. Everything starts
   ticked except filter folders, whose searches may not mean quite the same
   here. Lists you add in VirtualDJ later come in ticked.
3. **Choose whose tags win** when a track's title, rating or color differ:
   VirtualDJ's, while you still use it, or DJ Sugar's.

Press **Import**. Each track keeps VirtualDJ's tags, stars, color, comment,
play count, dates, BPM, beat grid, key, hot cues with their names and
colors, saved loops, gain, and its remix and automix points. Anything
VirtualDJ leaves empty is filled in from the file's own tags. The lists go
into a **VirtualDJ** folder, and history into the History tab. A VirtualDJ
folder that holds songs itself becomes a folder whose first list, with the
same name, holds those songs.

DJ Sugar only ever reads VirtualDJ's files. If you keep using VirtualDJ,
DJ Sugar checks its files each time it starts and brings in what changed.
Tracks are matched by their file, so nothing is added twice. Grids and cues
you edited in DJ Sugar are kept. The VirtualDJ folder is made again each time,
so change those lists in VirtualDJ, or copy one out of the folder to keep
editing it in DJ Sugar.

Once it's on, VirtualDJ's section of the Import tab opens up with these
choices and the two buttons below. **Lists › Choose…** opens the tree again
to change which lists come in. **Stop** leaves what came in but
stops updating it. **Remove…** takes out
the VirtualDJ folder and history, VirtualDJ's cues, grids and markers, and
the tracks only VirtualDJ had. Tracks in your own lists or music folder
stay. Either one closes the section again, back to **Set up…**.

### Lists of online tracks

Streamed tracks (TIDAL and VirtualDJ's other online catalogues, or TIDAL,
Spotify and the other streaming services in rekordbox) have no file to
play, so they stay in their lists as placeholders, in their place. A
placeholder is greyed out and shows its service (**TIDAL**) where the album
would be. It keeps the artist, title and length the app saved, and the
service's own track id. Double-click one to open it on the service's site.
Lists made only of streamed tracks come in too, marked **online only** in
the list picker.

When the song comes into your collection later (you buy it and it lands in
the inbox, or a scan finds the file), the placeholder becomes that track,
in the same place. DJ Sugar matches by ISRC when both sides have one, then by
artist and title at the same length (within 2 seconds). While you're signed
in to TIDAL or Spotify, DJ Sugar asks the service for its tracks' ISRCs, so
the match is sure. If a match is wrong, or one is missed, right-click the
track and pick **Change Match…**.

Placeholders aren't put on the **Wanted** list for you: right-click one
(or several) for **Add to Wanted** or **Add to Downloads**, or right-click
the list in the tree to add every placeholder in it. To have every
streamed track in the app's playlists go on the Wanted list, turn on **Add
streamed tracks to Wanted** under that app in Settings › Library › Import.
New ones join at each import; tracks you take off stay off. History is
never added: what you played isn't always what you want to keep.

The Wanted list holds only what you put there: tracks you clicked Buy on
and tracks you added with **Add to Wanted**. While the stores are off, it's under Collection. The search
box narrows it. A track leaves the list once your collection has it; to
take tracks off by hand, pick them (⌘A picks all) and right-click for
**Remove from Wanted**.

### Buying Wanted tracks

Pick tracks on the Wanted list, right-click and choose **Buy…** (or **Buy
N Tracks…**). DJ Sugar looks for each track in the stores it knows: the
artist's own site and Bandcamp, Mirlo, the iTunes Store, Qobuz, Bleep,
Boomkat, Beatport, Juno Download, Traxsource and Amazon Music. It finds
tracks by their ISRC on MusicBrainz, by Bandcamp's and the iTunes Store's
search, and opens a search on the stores it can't ask.

For each track it chooses a store that has it, preferring the artist's
own site, Bandcamp and Mirlo, where most of the money goes to the artist,
then the store whose files reach your library fastest, then lossless
files. A store that doesn't have the track only shows up as **Try**, to
search there. Click a track's store to choose another one it's sold on.

The tracks are grouped by store. **Open** opens the first track's page:
add it to the store's cart, then press **Next** for the next one. Click a
track to open just its page. Check out once on that store when they're all
in the cart. DJ Sugar never sees your payment, and its links carry no
referral codes.
Tracks you opened show **Buying on …** for a week.

Turn on **Bring purchases into Inbox** and what you download into
Downloads afterwards is filed into your music folder, goes to the
**Inbox** stage and comes off the Wanted list, matched by ISRC or by
artist, title and length. Zips are unpacked. It takes only files that
arrive after you turn it on. On a Mac, **Also take purchases from the
Music app** does the same for iTunes Store purchases the Music app
downloads, taking only tracks you are buying and leaving them where the
Music app keeps them.

## Bringing in a rekordbox library

**Import from rekordbox** works the same way. DJ Sugar reads rekordbox 6 and
7's own database, `master.db`, which it finds where rekordbox keeps it
(`~/Library/Pioneer/rekordbox` on a Mac, `AppData\Roaming\Pioneer\rekordbox`
on Windows). For rekordbox 5, or if the database can't be opened, export
your collection from rekordbox (**File › Export Collection in xml format**)
and choose that file in the first step.

From `master.db` each track keeps its tags, stars, color, comment, play
count, date added, key, My Tags (each category a tag group, see
[Your tags](#your-tags)), hot cues A to H
with their names and colors, memory cues, saved loops, its beat grid with
any tempo changes, and rekordbox's phrases (Intro, Verse, Chorus and so on,
shown on the waveform as DJ Sugar's own phrases are). Playlists come in their
folders, intelligent playlists as smart lists where DJ Sugar's search can say
the same thing, and history into the History tab. An XML export holds
tracks, grids, cues and playlists, but no phrases, history or intelligent
playlists.
## Bringing in a Mixxx library

**Import from Mixxx** works the same way. DJ Sugar reads Mixxx's
`mixxxdb.sqlite` (only reading it, so Mixxx can stay open), which it finds
where Mixxx keeps it: `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx`
or `~/Library/Application Support/Mixxx` on a Mac, `~/.mixxx` on Linux and
`AppData\Local\Mixxx` on Windows.

Each track keeps its tags, stars, color, comment, play count, dates, key,
hot cues with their labels and colors, saved loops, the main cue (as a
memory cue), intro and outro (as sections) and its beat grid, whether a
steady grid or a beat map with tempo changes. Playlists come in a
**Playlists** folder, crates in a **Crates** folder, and history sessions go
to the History tab. Auto DJ's queue stays in Mixxx.

## Bringing in a Traktor library

**Import from Traktor** works the same way. DJ Sugar reads Traktor's
`collection.nml` (only reading it), which it finds in the newest
`Documents/Native Instruments/Traktor x.y.z` folder on a Mac or Windows. If
yours is elsewhere, choose the file in the first step.

Each track keeps its tags, stars, color, comment, play count, dates, key,
hot cues 1 to 8 with their names, saved loops, its beat grid, other cues (as
memory cues) and fade in and fade out (as Automix points). Playlists come in
their folders, folders within folders just as Traktor shows them. Traktor's
own `_LOOPS` and `_RECORDINGS` lists and its smart lists stay out.

## Bringing in a djay Pro library

**Import from djay Pro** works the same way. DJ Sugar reads a copy of djay's
`MediaLibrary.db`, so djay can stay open and its library is never changed.
It finds it in `Music/djay/djay Media Library.djayMediaLibrary` on a Mac or
`Music/djay/djay Media Library` on Windows. If yours is elsewhere, choose
the file in the first step.

Each track that is a file on this computer keeps its title, artist, album,
genre, BPM, key, rating, year, comment, hot cues 1 to 8 with their names and
colors, saved loops (as memory cues), Automix start and end (as Automix
points), play count and color. When you set the first downbeat in djay, the
grid starts there. Playlists come in a **djay Pro** folder, in djay's order
and with djay's folders, and djay's history comes as one session per set.
Tracks streamed from TIDAL, Beatport, SoundCloud or Spotify show in their
lists as streamed tracks, with their artist, title and length.

## Bringing in an iTunes or Music library

**Import from iTunes** works the same way for iTunes and the Music app on a
Mac, and iTunes on Windows. DJ Sugar reads the library's XML file (only
reading it):

- **Music app:** choose **Music › Settings › Advanced** and turn on **Share
  Library XML with other applications**, so Music keeps
  `Music/Music/Library.xml` up to date. Or choose **File › Library › Export
  Library…** once and save `Library.xml`.
- **iTunes:** it keeps `iTunes Library.xml` (or `iTunes Music Library.xml`)
  in its `Music/iTunes` folder by itself.

DJ Sugar looks in those places, and for an exported `Library.xml` in your Music
folder or on the Desktop. If yours is elsewhere, choose the file in the first
step. When Music's file changes, DJ Sugar reads it again at its next start.

Each track keeps its title, artist, album, genre, grouping, composer,
comments, year, track and disc number, BPM, stars (not the ones Music worked
out from the album), play count, date added and when it was last played. The
file has no cues or beat grids, so DJ Sugar analyses the tracks as usual.
Playlists come in their folders, folders within folders. Music's own lists
(Library, Music, Purchased, Genius and the like) stay out. Smart playlists
are off at first; ticked, each comes in as a plain list of the tracks it held
when Music last wrote the file. Podcasts, audiobooks, videos and voice memos
stay out. Apple Music songs you added but don't have as files (and protected
downloads, which DJ Sugar can't play) show in their lists as streamed tracks,
with their artist, title and length.

### The same song from two apps

When a second app's library has a different file of a song you already have
(the same artist and title, ignoring case and punctuation, and a length
within two seconds), DJ Sugar doesn't add it twice. The copy already in your
collection stays, and the second app only fills in what it left empty. Its
lists point at that copy. Its grid is only used when the track has none from
another app, and its hot cues come in as memory cues when the track already
has hot cues, so nothing is lost. Removing the second import takes out what
it added and leaves the track as it was.

## The tree

The collection's tree has these parts:

- **Collection**, with how many tracks you have. Select it for all of
  them; open it for **Wanted** (tracks you went to buy or added with **Add to
  Wanted**) and the stages.
- **Crates**: lists for sorting your music, such as Yoga, Journeys, Dance
  or a style. A crate has no order of its own (it sorts like the
  Collection) and holds each track once. The **+** beside the heading adds
  a crate or a **Crate Folder**; crates have a box icon, or the one you
  pick.
- **Playlists**: running orders, for gigs or anything else. A playlist
  keeps the order you add tracks in, and a track can be in it twice. Its
  icon is always lines with a note. The **+** beside the heading adds a
  playlist, a smart playlist or a folder. Your smart lists are here too.

Right-click any folder for **New Crate**, **New Playlist** or **New
Folder**, so folders can hold folders and a folder can hold both. Right-click
a list for **Make It a Crate** or **Make It a Playlist** (a playlist that
becomes a crate keeps each track once).

Drag a crate, playlist or folder onto a folder to move it there, or onto
the **Crates** or **Playlists** heading to take it back to the top (a
crate's top is Crates, a playlist's is Playlists; a folder can go under
either).
- **Imported**: other apps' lists (a folder per app) and **Groupings and
  Tags**. They are read-only, because the app's next import makes them
  again, so they show a padlock and dimmer names. To keep an app's lists as
  your own (say, once you stop using that app), right-click any of them and
  pick **Make These Lists Mine**: they move to Playlists and can be changed, and that
  import keeps bringing in tracks, cues and grids but no more lists.
  Turning the import off and on again brings its lists back.

Everything starts closed, and the tree remembers which folders you left
open. Drag tracks onto a crate or playlist to add them, or onto a stage to
move them there. Or right-click tracks and point at **Add to playlist**: a
panel opens beside the menu with the lists you added to most recently at
the top. Type to find one, or pick **New crate** to make one with the
tracks in it. **Show in playlist** lists the crates and playlists a track
is in and opens the one you pick.

**Go to Artist** and **Go to Album** on the track menu show the whole
collection narrowed to that artist (any track they're on) or album, with
a chip on the filter bar; clear the chip to see everything again.

A track can be in as many crates as you like, and the **Crates** column
(right-click a heading) shows which ones it is in. Search with
`crate:yoga` for tracks in a crate with "yoga" in its name, `crate:=yoga`
for that crate only, and `crate:""` for tracks in no crate yet.

### History

Every track that plays on a deck for 30 seconds goes into that night's
history session, named after when it started (such as "Sat 31 Oct 2026,
22:15"), in the History tab beside the tree. A session opens in the order
you played it: the **#** column is the play order and **Played At** the time
each track was played. Click any heading to sort by it, and **#** to go back
to the order played. History sessions have their own columns, so changing
them doesn't change your other lists. Playlists and crates open in their own
order too, and the collection keeps its sort.

Right-click a session, playlist or crate and pick **Export to Spotify
Playlist** to make a private Spotify playlist of it in that order. DJ Sugar
finds each track on Spotify by its ISRC, then by artist, title and length;
the list's header says how many went in and names those Spotify doesn't
have, and the new playlist opens in your browser. If you signed in to
Spotify before this was added, sign out and in again so DJ Sugar may make
playlists.

### Playlist files

Right-click a playlist, crate, folder, history session, album or mixed set
and pick **Export as M3U8…**, then the folder to save it in. A list becomes
one `.m3u8` file named after it; a folder becomes a folder of them, with
its folders inside. Tracks kept in the file's folder or below it are
written with paths from there, so a USB stick with the music beside the
list works on any computer; the rest are written in full. Online tracks
in a list have no file, so they go in as a note with their name and
service.

To bring a list in from another app, pick **Import Playlist File…** from
the **+** beside Playlists. DJ Sugar reads M3U, M3U8, PLS and XSPF (from
VLC, foobar2000, Winamp, Serato and others), makes a playlist with the
file's name and finds each track in your collection: by its path, by its
path from the file's folder, then by its file name. Files on disk that
aren't in your collection yet join it. The list's header names any tracks
it couldn't find.

### Taking your library to another app

**File › Export › rekordbox XML…** (or right-click **Collection** and pick
**Export as rekordbox XML…**) saves your whole collection as one
`rekordbox.xml` file: every track with its tags, rating, color, comment,
date added and play count, its beat grid (one marker per tempo change, so
a grid that changes tempo comes across), hot cues with their numbers and
colors, memory cues and saved loops, and every crate, playlist and folder
in its tree. Right-click a single list or folder to save just that one.

To open it in rekordbox, pick the file under **Preferences › Advanced ›
Database › rekordbox xml**, turn on **rekordbox xml** under **Preferences ›
View › Layout**, and it shows as a node in rekordbox's tree. Library
tools such as Lexicon and Rekord Buddy read it too and convert it to
Serato, Traktor, Engine DJ and VirtualDJ. A few things don't fit the
format:

- Your tags go into the comment as `/* Tag / Tag */`, the way rekordbox
  writes its own My Tags there.
- rekordbox has eight hot cues (A to H) and eight track colors: pads 9
  and up come across as memory cues, and a track's color as the nearest
  of the eight.
- Phrases, stems, stages and online tracks with no file are left out.

## Collection Health

[Collection Health](collection-health.md), the cross under Collection in the
rail, finds what needs fixing in your collection: duplicates, junk in names,
missing or poor tags, covers, low bitrates, files that went missing, and
more.

## Search and filters

Search, the filter bar and smart lists have a page of their own:
[Filtering tracks](filtering.md).

## Picking several tracks

Click a track to pick it, **shift-click** another to pick every track in
between, and **⌘-click** (Ctrl-click on Windows and Linux) to add or drop
one track at a time; ⌘A picks the whole list. Right-click, drag, rate or
add to a playlist and it applies to every picked track. Sorting or a
refreshed list keeps the same tracks picked. With several picked, the list's
heading says how many and their total time ("12 selected · 1h 14m") in
place of the whole list's time.

The track menu's **Rating** and **Color** rows set every picked track at
once; the colors are the ones `color:red`, `color:blue` and so on search for.

A track's color shows as a dot in the **Color** column (headed ●, sorted
red, orange, yellow, green, cyan, blue, purple, pink). **Settings › Library ›
Track color** changes that to a strip at the row's left edge or a soft tint
of the whole row, or turns it off; color searches work either way.

## Track info

The **Info** tab on the right of the browser shows the picked track (or
the track on a deck) and lets you edit it in place. The rating is under the
title. Below, three tabs keep everything in view without scrolling:

- **Track**: track, artist, remix, album and genre, then year and track
  number, BPM and key, gain and time, two to a line.
- **Tags**: your tag groups, grouping and the comment. Each tag is a
  chip: **×** takes it off, and the **+ tag** box adds one (or several,
  with commas between), suggesting tags you already use.
- **File**: format, audio, size, the file (with Show in Finder), its folder,
  plays, when it was added and last played.

DJ Sugar remembers the tab you were on. Tags go into the file as well as your
collection.

Pick several tracks and **Info** edits them together. A field they share
shows its value; one that differs says how many values it has. Type a value
to give it to them all, or click **▾** to pick one of the values they have
(with how many tracks have it), keep each track's own, or clear it in all.
Changed fields are marked in lilac (a tab with changes gets a dot) and wait
for **Apply** (or **Discard**).
Apply writes them to your collection and the files' tags in one go, and
⌘Z takes the whole change back. Fields you typed this way are left alone by
Clean up and Look up online. The stars set the rating of every picked track
at once.

Tags work on all the picked tracks at once, with no Apply. A tag on only
some of them shows how many have it ("dreamy 5/12"): click it to put it on
all. **×** takes a tag off all of them, and **+ tag** adds one to all.
⌘Z takes a tag change back. You can also right-click tracks and choose
**Add Tag**: it lists the tags you added lately, then the ones you use most,
and you can type to find one or make a new one.

## Fixing names

Right-click one or more tracks and choose **Fix Names…** to tidy their names
in one go. Pick a tool, choose **In**: all names (track, artist, remix and
album) or one field such as Genre or Comment, and watch the preview:

- **Find and Replace**: what to find and what to put instead (leave it empty
  to take it out). **Match case**, **Whole words**, and **Pattern** for a
  regular expression, where `$1` in the replacement puts back what the first
  brackets found.
- **Change Case**: Title Case (small words like "of" and "the" stay lower,
  DJ, MC and UK stay in capitals, names like McCartney are kept), Sentence
  case, UPPER or lower.
- **Remove Brackets**: takes out what's in (round) and [square] brackets.
- **Swap Artist and Title**: for tracks that have them the wrong way round.

**Review** opens the review table: untick what you'd rather keep, then
**Apply**. Undo takes the whole change back. A track or artist name is never
emptied.

## Columns

Right-click any column heading to choose which columns the list shows, your
tag groups included, or **Reset columns** to go back to the default. Drag a
divider on a heading's right edge to make that column wider or narrower (the
columns after it move along and keep their widths), and drag the heading itself
to move it. Click a heading to sort by it. DJ Sugar remembers your columns.

## Your tags

Tags sit in groups you make in **Settings › Collection › Tags**, for example
Mood, Instruments or Vibe: add as many as you need, rename them, and set
each to **Several** tags per track or **One**. A tag is a name of its own,
spaces included ("Peak Time"), with no `#` needed. Removing a group takes its
tags off every track (DJ Sugar asks first when tracks use it), and **↑**
moves a group up.

**Tags** on a group's row lists its tags with how many tracks have each.
Give a tag a color (it shows as a dot on its chip), or type a new name and
press Return to rename it on every track: a name the group has already
merges the two into one. **×** deletes a tag from every track.

Each group's name works as a search word (`mood:uplifting`,
`vibe:"peak time"`), and so does its place (`tag1:uplifting` for the first
group); `tag:` searches every group. The `#` in front of a tag is optional
when searching. On the same tab, give Grouping the name you use it for, such
as Trip Stage (`tripstage:"4 peak"`). **Copy into a tag group** there
makes a group of that name with each track's grouping as a tag, so a track
can then have more than one (the Grouping tag itself stays as it is).

Tags from other apps come into your groups: VirtualDJ's User 1 and User 2
each fill a group (each word a tag, the `#` dropped), and each rekordbox My
Tag category becomes a group of the same name. A group you already have
with that name takes them. Rename a group made this way and later imports
still go into it.

## Backups

DJ Sugar backs up your collection (cues, tags, crates, playlists and
history) and your settings once a day, under **Backups** in
**Settings › Collection › General**. **Back up** sets how often: **Off**,
**Daily** (the default) or **Weekly**. The newest 10 backups are kept.
Each is one `.djsugarbackup` file, which also holds your own controller
mappings. Sign-ins to streaming services stay out of it.

**Backup folder** is a `Backups` folder next to DJ Sugar's settings until
you pick another; a cloud folder (iCloud Drive, Dropbox) keeps your
backups off the computer. **Back Up Now** makes one straight away.

**Restore…** picks a backup. It goes in when DJ Sugar next starts, so quit
and open it again. How things were before is saved first, as a
"Before restore" backup in the backup folder, and you stay signed in to
your services. **Cancel** drops a restore that hasn't happened yet.
Music and sample files are not part of a backup.
