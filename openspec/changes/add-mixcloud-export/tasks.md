# Tasks

## 1. Mixcloud-ready MP3

- [x] 1.1 Recordings join Mixed Sets as soon as they stop
- [x] 1.2 Recorder writes `<name>.tracklist.txt` beside the `.cue`
- [x] 1.3 Read the recorder's `.cue` (several parts, tracks in the same second dropped) into one tracklist
- [x] 1.4 Export for Mixcloud: 320 kbps MP3, parts joined, streamed, ID3v2.4 tag with title, artist, tracklist comment and chapters
- [x] 1.5 Mixcloud upload form fields and limits checked before sending

## 2. Upload to Mixcloud

- [x] 2.1 Client secret lives on github.com/winnerspiros/djsugar: Cloudflare Pages functions `website/functions/mixcloud/{login,callback}.js` swap the code for a token and hand it to the app on 127.0.0.1 with a one-off state
- [x] 2.2 Sign in with Mixcloud in the browser; token kept in the keychain; Sign Out of Mixcloud; a revoked token signs in again once
- [x] 2.3 Upload to Mixcloud: makes the MP3 if needed, uploads it private with the tracklist as sections and description, progress, opens the show to name, tag and publish
- [x] 2.4 Register the DJ Sugar app on Mixcloud (redirect URI https://github.com/winnerspiros/djsugarmixcloud/callback) and set `MIXCLOUD_CLIENT_ID` and `MIXCLOUD_CLIENT_SECRET` in the Cloudflare Pages project
- [x] 2.7 Track menu keeps only Upload to Mixcloud; the MP3 export moves to File › Export › Mixcloud… on macOS
- [x] 2.8 Windows and Linux: the same File and Edit items from a menu button in the top bar (#400)
- [ ] 2.5 An upload dialog in the app (name, description, tags, cover, publish now) instead of finishing on mixcloud.com
- [ ] 2.6 Update the user guide (recording, Mixed Sets, Mixcloud)

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.
