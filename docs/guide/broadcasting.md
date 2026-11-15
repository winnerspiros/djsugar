# Loudness and broadcasting

DJ Sugar streams your mix to internet radio (Icecast and Shoutcast servers) and
measures its loudness as it plays, so you can hit the level your station or
platform asks for.

## Streaming to a radio station

Your station or streaming host gives you a server address, a port, a mount
point and a source password. Put them into DJ Sugar once:

1. Open **Settings › Broadcast**, switch on **Broadcasting**, and click **Add**
   next to **Add a station**.
2. Fill in **Address**, **Port**, **Mount point** (Icecast) and **Password**.
   Leave **User** as `source` unless your host says otherwise.
3. Choose the **Format** and **Bitrate**. **MP3 at 128 kb/s** plays everywhere
   and shows listeners each track's title. **Opus** sounds better at low
   bitrates (96 kb/s is plenty).
4. Click **Test**. DJ Sugar connects, checks the password and hangs up, without
   going on air.

The password is kept in your system's keychain (Keychain on macOS, Credential
Manager on Windows, the Secret Service on Linux), never in DJ Sugar's settings
file. Where there's no keychain, DJ Sugar remembers it until you quit.

You can add several stations and switch each on or off: **ON AIR** streams to
every station that's switched on, at once. The **ON AIR** button only shows in
the top bar while **Broadcasting** is on and at least one station is set up and
switched on.

### Going on air

Click **ON AIR** in the top bar, then **Go on air**. The button turns red and
counts the time on air. Each station gets a light:

| Light | Means |
|---|---|
| Red | On air |
| Amber, pulsing | Connecting, or reconnecting after a drop |
| Grey | Gave up: the message says why (wrong password, mount point in use, no server) |

If the connection drops, DJ Sugar reconnects at once and then every 5 seconds, for
up to 10 minutes. A slow upload never makes the music stutter: if it falls more
than about 10 seconds behind, DJ Sugar reconnects that station and carries on from
now. Icecast servers also report how many people are listening, next to the
time.

Click **Go off air** to stop. Controllers with a broadcast button (mapped to
Mixxx's `[Shoutcast],enabled`) go on and off air too.

### What listeners see

Listeners see the track that's heard most (playing, with its channel fader up,
on the crossfader's live side), a couple of seconds after it takes over, so a
quick cut back and forth doesn't flicker. Change the text in **Settings ›
Broadcast › Title sent to listeners**: `{artist}`, `{title}` and `{dj}` are
filled in, and plain text such as `Live from the club` is sent as it is.

MP3 streams update the title on the server, which every player shows. Opus
carries titles inside the stream: the first title always goes out, and later
ones only with **Send each new title in the stream**, because starting the
stream again at each track stops some players.

### Now playing in OBS

Streaming on Twitch or YouTube with OBS or Streamlabs? Switch on **Settings ›
Broadcast › Now playing for streaming software** to show the track on screen.
It follows the same track listeners would see, once it has been heard for the
**Changes after** time (10 seconds at first), and works without a radio
station.

- **The overlay page**: in OBS add a **Browser Source** with the address shown
  under the switch (such as `http://127.0.0.1:7487/now-playing`), 1920 × 1080.
  Its background is clear, so only the card with the cover, artist and title
  shows, and it fades from one track to the next. Pick **Lower third**,
  **Corner** or **Ticker** under **Overlay style**; the page follows without
  being reloaded. It is read only, and only this computer can open it unless
  you switch on **Open to other devices**.
- **The text file**: in OBS add a **Text** source, tick **Read from file** and
  pick `now-playing.txt` in the **Files folder** (`Music/DJSugar/Now Playing`
  at first). Its line is `{artist} - {title}`; change it under **Text file
  line**, with `{album}`, `{label}`, `{genre}`, `{bpm}` and `{key}` too.
- **`now-playing.json`**, beside it, has the track, its BPM, key, deck and
  start time, and the ten tracks before it, for chat bots and other tools.
  The same JSON is at `/now-playing.json` on the overlay's address.

### Scrobbling to ListenBrainz

Switch on **Settings › Player › Send plays to ListenBrainz** and paste
your user token (from listenbrainz.org, under Settings) into **ListenBrainz
token**. Each track the audience hears then goes to your ListenBrainz
profile, from any layout, as a music player's plays do: it shows as playing
now after a few seconds, and counts as a listen once half of it, or four
minutes, has played (tracks under 30 seconds never count). Tracks you only
cue in the headphones are left out. Without a connection, plays wait and
go once ListenBrainz answers again. Under the token it says who you are
signed in as and how many plays were sent.

### Scrobbling to Last.fm

The same plays can go to Last.fm. It needs a free API account of your own:
make one at last.fm/api/account/create (any name and description; leave the
callback empty), then switch on **Send plays to Last.fm**, paste the **API
key** and **shared secret**, and press **Sign in**. Last.fm opens in the
browser; allow DJ Sugar there, and Settings says who you are signed in as.
**Sign out** forgets the session. Both services can be on at once.

### Licences

Playing other people's music to the public needs licences in most countries,
from bodies such as PRS, GEMA, SACEM, ASCAP, BMI and SoundExchange. Many radio
hosts include them; check yours before you go on air.

## The loudness meter

Open the **ON AIR** button in the top bar. Under **Go on air**, **Loudness**
shows:

| Reading | What it is |
|---|---|
| The big number | **Short-term** loudness: the last 3 seconds. Steer by this one. |
| **MOMENTARY** | The last 0.4 seconds. Jumps with every kick and break. |
| **INTEGRATED** | The whole mix since the last reset: what a platform measures for your set. |
| **RANGE** | Loudness range in LU: how far quiet and loud parts are apart. 5 to 10 LU is usual for a DJ set. |
| **TRUE PEAK** | The highest peak since the reset, including peaks between samples, in dB. Red above −1 dB. |

The readings are green within 1 LU of your target, amber within 3 and red
further off. **Reset** starts integrated loudness, the range and the true peak
again. They also start again when you press **REC** or go on air.

While recording, the **REC** button in the top bar shows the short-term
loudness next to the time.

## Choosing a target

Pick the target in **Settings › Broadcast › Stations** under **Loudness**:

| Target | For |
|---|---|
| **Internet radio · −16 LUFS** (the default) | Icecast and Shoutcast stations, mix shows |
| **Podcast or talk · −18 LUFS** | Shows with a lot of talking |
| **Livestream · −14 LUFS** | Twitch, YouTube, Mixcloud, and uploads to streaming services |
| **Broadcast, EBU R128 · −23 LUFS** | FM and DAB radio, TV |

The target only colors the meter: it doesn't change any level. To move your
mix, turn the master down (or up) until the short-term reading sits green most
of the time. With **Auto gain** on (**Settings › Mixer**), every track already
starts at the same loudness, so the mix stays close once it's set.

## The limiter while recording or on air

The master always runs through a limiter, so the mix never clips. While you
record or are on air, it also keeps the peaks between samples under **−1 dB**
true peak, which MP3, AAC and Opus need so they don't distort. For streams at
low bitrates (96 kb/s or less), choose **−2 dBTP** in **Settings › Audio ›
Recording**.

The headphones and booth don't go through the limiter (they would lag behind
the master), so they have a soft clipper instead: nothing changes below
**−1.5 dBFS**, and louder peaks are bent smoothly under full scale rather than
squared off.

## For controllers

| Control | Value |
|---|---|
| `[Master],loudness_momentary`, `loudness_short_term`, `loudness_integrated` | LUFS, or −100 with nothing to measure |
| `[Master],loudness_range` | LU |
| `[Master],true_peak` | dBTP, highest since the reset |
| `[Master],loudness_reset` | Set to 1 to reset |
| `[Master],loudness_target` | The target, in LUFS |
| `[Master],air_ceiling` | The limiter's ceiling while recording or on air, in dBTP |
| `[Shoutcast],enabled` | 1 goes on air, 0 off air |
| `[Shoutcast],status` | 0 off air, 1 connecting, 2 on air to every station, 3 failed, 4 on air to some |
| `[Broadcast],on_air_seconds` | Time on air, in seconds |
