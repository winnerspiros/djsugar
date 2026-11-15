# Signing and notarizing the macOS app

The release workflow signs DJSugar.app with a Developer ID certificate and has
Apple notarize it, so the app opens with a double-click on any Mac. It does
this only when the five repository secrets below are set; without them the
app is ad-hoc signed as before.

## One-time setup

1. **Create the certificate.** On your Mac, open Xcode → Settings → Accounts,
   select your Apple ID, click *Manage Certificates…*, then **+** →
   *Developer ID Application*. (Only the Account Holder of the team can create
   one.)
2. **Export it as a .p12.** Open Keychain Access → *My Certificates*, find
   "Developer ID Application: <your name> (<TEAM ID>)", right-click →
   *Export…*, save as `djsugar.p12` and choose a password.
3. **Encode it for GitHub:**

   ```sh
   base64 -i djsugar.p12 | pbcopy
   ```

4. **Make an app-specific password** at <https://account.apple.com> →
   Sign-In and Security → App-Specific Passwords. Name it "DJ Sugar notarize".
5. **Find your Team ID** at <https://developer.apple.com/account> →
   Membership details (10 characters, e.g. `AB12CD34EF`).
6. **Add the secrets** in GitHub → the repository → Settings → Secrets and
   variables → Actions → *New repository secret*:

   | Secret | Value |
   | --- | --- |
   | `MACOS_CERTIFICATE` | the base64 text from step 3 |
   | `MACOS_CERTIFICATE_PASSWORD` | the .p12 password from step 2 |
   | `APPLE_ID` | your Apple ID email |
   | `APPLE_APP_PASSWORD` | the app-specific password from step 4 |
   | `APPLE_TEAM_ID` | the Team ID from step 5 |

Then delete `djsugar.p12` from your Mac (the certificate stays in Keychain).

## Checking it

Run the Release workflow by hand (Actions → Release → *Run workflow*), download
the `release-macos` artifact and check it on your Mac:

```sh
spctl -a -vv /Applications/DJSugar.app   # "accepted, source=Notarized Developer ID"
```

Notarization usually takes a few minutes. If it fails, the log shows a
submission id; `xcrun notarytool log <id> --apple-id … --team-id …` says why.

## What gets signed

- Hardened runtime and a secure timestamp, which notarization requires.
- Entitlements in `res/macos/djsugar.entitlements`: only
  `com.apple.security.device.audio-input`, so microphone and line inputs keep
  working under the hardened runtime.

## In-app updates

DJ Sugar checks GitHub's latest release when it starts (Settings → Help →
Check for updates). The macOS app installs an update itself only if the
download matches GitHub's checksum, passes Gatekeeper (notarized) and is
signed by the same Team ID as the running app, so every release must be
signed and notarized for updates to keep working. Builds older than the
updater (0.2.0 and before) need one manual download.
