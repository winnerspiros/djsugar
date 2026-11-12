# Reloop Mixon 4 – Mapping für Mixxx

Mit diesem Mapping steuerst du die kostenlose DJ-Software **[Mixxx](https://mixxx.org)** vollständig über deinen **[Reloop Mixon 4](https://www.reloop.com/reloop-mixon-4)** – inklusive aller Lämpchen (LEDs) am Gerät: farbige Pads, Jog-Ring, Loop-Anzeige und Pegelanzeigen. Du brauchst dafür kein Serato und keine kostenpflichtige Software.

> **Ganz neu hier?** Kein Problem. Diese Anleitung führt dich Schritt für Schritt durch alles – von der Installation bis zum ersten Mix. Fachbegriffe werden unterwegs erklärt; ganz unten gibt es zusätzlich ein [kleines Wörterbuch](#wörterbuch-für-einsteiger).

---

## Inhalt

- [Was ist das hier eigentlich?](#was-ist-das-hier-eigentlich)
- [Das brauchst du](#das-brauchst-du)
- [Installation in 5 Minuten](#installation-in-5-minuten)
- [Mapping in Mixxx aktivieren](#mapping-in-mixxx-aktivieren)
- [Erster Test: Läuft alles?](#erster-test-läuft-alles)
- [Deine ersten Schritte als DJ](#deine-ersten-schritte-als-dj)
- [Was macht welcher Knopf?](#was-macht-welcher-knopf)
- [Die 8 Pad-Modi](#die-8-pad-modi)
- [Wichtig zu wissen: „Soft-Takeover"](#wichtig-zu-wissen-soft-takeover)
- [Fehlerbehebung](#fehlerbehebung)
- [Bekannte Grenzen](#bekannte-grenzen)
- [Deinstallation](#deinstallation)
- [Mithelfen & Feedback](#mithelfen--feedback)
- [Wörterbuch für Einsteiger](#wörterbuch-für-einsteiger)
- [Lizenz](#lizenz)

---

## Was ist das hier eigentlich?

Dein Reloop Mixon 4 ist ein **MIDI-Controller** – ein Gerät mit Reglern, Knöpfen und Plattentellern, das keine eigene Musik abspielt, sondern nur „Befehle" an eine Software schickt. Damit die Software weiß, dass „Drehregler links oben" = „Höhen von Deck 1" bedeutet, braucht sie eine Übersetzungstabelle. Genau das ist ein **Mapping**.

Dieses Mapping übersetzt jeden Knopf, Regler und Teller deines Mixon 4 in die passende Mixxx-Funktion – und schickt umgekehrt auch die Lämpchen-Signale zurück ans Gerät, damit z. B. die Play-Taste leuchtet, während ein Track läuft.

## Das brauchst du

1. **Einen Reloop Mixon 4** (per USB-Kabel am Computer).
2. **Mixxx in Version 2.5 oder neuer** – kostenlos herunterladen unter **[mixxx.org/download](https://mixxx.org/download/)** (Windows, macOS und Linux).
3. Dieses Mapping (die zwei Dateien im Ordner `controllers/`). Wie du an sie kommst, steht gleich im nächsten Abschnitt.

> Getestet mit Mixxx 2.5.6 unter Windows 11 und der Mixon-4-Firmware v2218.

## Installation in 5 Minuten

Es gibt zwei Wege. **Weg A** (Datei-Kopieren von Hand) funktioniert auf jedem System und ist am einfachsten zu verstehen. **Weg B** (Skript) geht unter Windows automatisch.

### Zuerst: die Dateien herunterladen

1. Klicke oben auf dieser Seite auf den grünen Knopf **`Code`** → **`Download ZIP`**.
2. Entpacke die ZIP-Datei irgendwohin (z. B. auf den Desktop).
3. Im entpackten Ordner findest du den Unterordner **`controllers`** mit zwei Dateien:
   - `Reloop-Mixon-4.midi.xml`
   - `Reloop-Mixon-4.scripts.js`

### Weg A – Dateien von Hand kopieren (alle Systeme)

Kopiere **beide** Dateien aus `controllers/` in den Mixxx-Controller-Ordner deines Systems:

| System | Zielordner |
|---|---|
| **Windows** | `%LOCALAPPDATA%\Mixxx\controllers` |
| **macOS** | `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/controllers` |
| **Linux** | `~/.mixxx/controllers` |

> **Windows-Tipp:** Drücke `Windows-Taste + R`, tippe `%LOCALAPPDATA%\Mixxx\controllers` ein und drücke Enter – der Ordner öffnet sich direkt. Falls der Unterordner `controllers` noch nicht existiert, lege ihn einfach an.

### Weg B – Automatisch per Skript (nur Windows)

Öffne den entpackten Ordner, gehe in den Unterordner `scripts`, und führe `install.ps1` aus. Falls ein Doppelklick nicht funktioniert: Rechtsklick auf die Datei → **„Mit PowerShell ausführen"**. Das Skript kopiert beide Dateien automatisch an die richtige Stelle.

## Mapping in Mixxx aktivieren

1. Schließe deinen Mixon 4 per USB an und schalte ihn ein.
2. Starte Mixxx (**neu starten**, falls es beim Kopieren schon lief).
3. Gehe im Menü auf **Optionen → Einstellungen** (unter macOS: **Mixxx → Einstellungen**).
4. Wähle links **Controller**. Dein **Mixon 4** sollte in der Liste stehen.
5. Klicke ihn an und wähle rechts im Auswahlfeld **„Load Mapping"** das Mapping **„Reloop Mixon 4"** aus.
6. Setze den Haken bei **„Enabled"** (Aktiviert) und bestätige mit **Übernehmen / OK**.

Fertig! Ab jetzt reagiert Mixxx auf deinen Controller.

## Erster Test: Läuft alles?

So prüfst du in einer Minute, ob alles sitzt:

1. Zieh in Mixxx einen Song aus der Bibliothek auf **Deck 1** (oder markiere ihn und drücke am Controller die **Load-Taste** über dem linken Deck).
2. Drück am Controller **Play** – der Song läuft, und die **Play-Taste leuchtet**.
3. Schieb den **Lautstärke-Regler (Kanalfader)** von Deck 1 nach oben – du solltest die Musik hören, und die **Pegel-LEDs** am Gerät zucken im Takt.
4. Dreh an **Höhen / Mitten / Tiefen** – der Klang verändert sich.

Wenn das klappt, ist alles korrekt installiert. 🎉

> **Kein Ton oder Regler tun „nichts"?** Das ist fast immer normal und schnell erklärt – siehe [Soft-Takeover](#wichtig-zu-wissen-soft-takeover) und [Fehlerbehebung](#fehlerbehebung).

## Deine ersten Schritte als DJ

Ein einfacher Übergang von einem Song zum nächsten:

1. Lade **Song A** auf Deck 1, starte ihn, Fader hoch – das Publikum hört Song A.
2. Lade **Song B** auf Deck 2.
3. Setz den Kopfhörer auf und drück am Deck 2 die **CUE-Taste (PFL)** – jetzt hörst *nur du* Song B im Kopfhörer und kannst ihn vorbereiten.
4. Drück am Deck 2 kurz **Sync** – Mixxx passt das Tempo von Song B automatisch an Song A an.
5. Starte Song B (Play) und schieb langsam den **Crossfader** (der waagerechte Regler in der Mitte) von links nach rechts – Song A wird leiser, Song B lauter. Fertig ist der Übergang.

Von hier an lohnt es sich, die **Loops** und die **Pads** auszuprobieren (siehe unten).

## Was macht welcher Knopf?

### Transport & Deck (pro Seite)

| Bedienelement | Funktion | Mit gedrückter Shift-Taste |
|---|---|---|
| **Play** | Startet/pausiert den Song (LED leuchtet beim Abspielen) | Rückwärts abspielen, solange gehalten |
| **Cue** | Springt zur Cue-Marke bzw. setzt sie | Zum Song-Anfang springen und stoppen |
| **Sync** | Kurz = Tempo einmalig angleichen · Halten = dauerhaft koppeln (LED) | Quantisierung für dieses Deck an/aus |
| **Jog-Teller (oben berühren)** | Scratchen | – |
| **Jog-Teller (am Rand)** | Song leicht anschubsen/bremsen (Feintuning) | – |
| **Jog-LED-Ring** | Zeigt, wie weit der Song fortgeschritten ist | – |
| **Pitch-Fader** | Tempo feinjustieren (sehr fein auflösend) | Fader-Start (hoch = Play, runter = Stopp) |
| **Keylock** | Tonhöhe beim Tempo-Ändern einfrieren (LED) · 2 Sek. halten = Tempo auf 0 zurück | – |
| **Key Sync** | Tonart an das andere Deck angleichen · Doppelklick = zurück zur Originaltonart | – |
| **Slip** | Slip-Modus (LED): der Song läuft im Hintergrund weiter, auch während du scratchst | **Shift+Slip = Passthrough an/aus** (externer/Streaming-Eingang durch den Kanal) |
| **PHONO** (nur Deck 3/4) | Passthrough an/aus (wie Shift+Slip) | – |
| **Load** | Lädt den in der Bibliothek markierten Song ins Deck (LED = Song geladen) | – |

### Loops

| Bedienelement | Funktion | Mit Shift |
|---|---|---|
| **Auto Loop** | Setzt/löst eine Schleife (LED = Loop aktiv) | Letzte Schleife erneut aktivieren |
| **Loop-Encoder (drehen)** | Schleife halbieren / verdoppeln | – |
| **Loop-LEDs** | Zeigen die Schleifenlänge: **blinkend** = 1/32–1/2 Beats, **leuchtend** = 1–16 Beats | – |
| **Param − / +** | Im Song vor-/zurückspringen (Beatjump) | Aktive Schleife verschieben |

### Mixer & Master (Klang und Lautstärke)

| Bedienelement | Funktion |
|---|---|
| **Gain / High / Mid / Low** | Vorverstärkung und Klangregelung (Höhen, Mitten, Tiefen) |
| **Filter-Regler** | Weicher Filter (dumpfer/schärfer) pro Kanal |
| **Filter-FX-Taste** | Weist den Kanal einer Effekteinheit zu (Deck 1/3 → FX1, Deck 2/4 → FX2) |
| **Kanalfader** | Lautstärke des Decks |
| **Crossfader** | Blende zwischen linker und rechter Seite (dazu Kurven-Regler und A/Thru/B-Schalter) |
| **CUE (PFL)** | Song im Kopfhörer vorhören (LED), dazu Cue-Mix- und Kopfhörer-Lautstärke |
| **Pegel-LEDs** | Aussteuerung pro Kanal und Master (links/rechts) |
| **Sampler-Regler** | Lautstärke aller Sampler |
| **Quantize** (2 Sek. halten – es ist die Hot-Cue-Modus-Taste) | Schaltet die Quantisierung für alle 4 Decks um |

### Musikbibliothek durchsuchen

| Bedienelement | Funktion | Mit Shift |
|---|---|---|
| **TRAX drehen** | Durch die Songliste blättern | 10 Zeilen pro Rastung (schnelles Scrollen) |
| **TRAX drücken** | Ausgewählten Eintrag öffnen | – |
| **BACK** | Zurück zur Ordner-/Seitenleiste | – |

### Effekte (FX) – je Seite eine Effekteinheit

| Bedienelement | Funktion |
|---|---|
| **Regler 1–3** | Stärke der drei Effekt-Plätze |
| **ON 1–3** | Effekt an/aus (LED); lange halten = nur solange gedrückt aktiv |
| **TAP halten + ON** | Einen Effekt „in den Fokus" holen (dann steuern die Regler dessen Feineinstellungen) |
| **Parameter-Encoder** | Effekt-Anteil (trocken/nass); drücken = zurück auf 50 % |

> Welche Effekte in den drei Plätzen liegen, stellst du in Mixxx selbst per Maus ein (in der Effektleiste oben).

## Die 8 Pad-Modi

Die 8 farbigen Pads pro Seite haben mehrere Modi. Umgeschaltet wird über die Modus-Tasten neben den Pads. Die Pad-Farbe zeigt dir jeweils den Zustand.

| Modus | Was die Pads tun | Mit Shift auf dem Pad |
|---|---|---|
| **Hot Cue** | Sprungmarken 1–8 setzen und anspringen (Pad leuchtet in der Cue-Farbe) | Marke löschen |
| **Loop Roll** | Kurze Wiederhol-Schleifen 1/32–4 Beats, solange gedrückt (blau) | – |
| **Sampler** | Sampler 1–8 abspielen oder laden (weiß = geladen, grün = spielt) | Stoppen bzw. auswerfen |
| **Slicer** | Im Song springen (Beatjump −8/−4/−2/−1/+1/+2/+4/+8) | – |
| **Cue Loop** | Schleife ab einer Sprungmarke starten | Marke löschen |
| **Saved Loops** | Schleifen speichern und wieder abrufen (Marken 9–16) | Marke löschen |
| **Pitch Play** | Song in verschiedenen Tonhöhen anspielen (blau = tiefer, weiß = original, rot = höher) | Zurück zur Originaltonart |
| **Slicer Loop** | Feste Schleifenlängen 1/8–16 Beats an-/ausschalten (türkis) | – |

> Hinweis: Mixxx hat keine echte „Slicer"-Funktion wie Serato. Die beiden Slicer-Modi sind hier deshalb mit den sinnvollsten Alternativen (Beatjump bzw. feste Loops) belegt. Eine echte Slicer-Nachbildung ist [in Arbeit](../../issues).

## Wichtig zu wissen: „Soft-Takeover"

**Das häufigste Missverständnis für Einsteiger:** Wenn du Mixxx startest, stehen die Regler in der Software vielleicht ganz woanders als an deinem Controller. Damit die Lautstärke nicht plötzlich springt, „wartet" Mixxx bei manchen Reglern (EQ, Filter, Pitch), bis du mit dem physischen Regler den Software-Wert **erreichst** – erst dann greift er.

**Was das für dich heißt:** Wenn ein Regler beim ersten Antippen „nichts tut", dreh ihn einmal langsam durch seinen Bereich. Sobald du den aktuellen Software-Wert passierst, rastet er ein und funktioniert normal. Das ist **kein Fehler**, sondern ein Schutz gegen Lautstärke-Sprünge.

## Fehlerbehebung

**Der Controller taucht in Mixxx nicht auf.**
- USB-Kabel prüfen, Gerät einschalten, Mixxx neu starten.
- Möglichst direkt an den Computer anschließen (nicht über einen billigen USB-Hub).

**Ich habe das Mapping kopiert, aber es steht nicht in der Auswahlliste.**
- Liegen beide Dateien wirklich im richtigen `controllers`-Ordner (siehe [Installation](#installation-in-5-minuten))?
- Mixxx **komplett schließen und neu starten** – die Liste wird nur beim Start eingelesen.

**Ein Regler oder Fader tut nichts.**
- Meist ist es [Soft-Takeover](#wichtig-zu-wissen-soft-takeover): den Regler einmal langsam ganz durchdrehen.

**Ein Effekt-Regler wirkt nicht.**
- Effekte müssen erst in Mixxx in die Effekt-Plätze geladen werden (Effektleiste oben, per Maus).

**Eine LED leuchtet/leuchtet nicht wie erwartet.**
- Einige Lämpchen steuert der Controller selbst (z. B. die Modus-Tasten, Shift). Das ist normal und nicht vom Mapping beeinflussbar.

**Ich möchte genau sehen, was der Controller sendet (für Fortgeschrittene).**
- Windows: `scripts\debug-midi.ps1` startet Mixxx mit MIDI-Protokollierung. Jede eingehende Nachricht landet dann in der Logdatei unter `%LOCALAPPDATA%\Mixxx\mixxx.log`.

Wenn nichts hilft: [ein Issue eröffnen](../../issues) und dabei dein Betriebssystem, deine Mixxx-Version und eine genaue Beschreibung angeben.

## Bekannte Grenzen

- **Kein echter Slicer:** Mixxx bietet die Funktion nicht; die Slicer-Modi sind ersatzweise mit Beatjump und festen Loops belegt.
- **Pitch Play** verändert bislang die Tonhöhe des laufenden Songs (echtes Neuanspielen ab einer Marke ist in Arbeit).
- **Jog-Empfindlichkeit** ist ein Erfahrungswert (Reloop dokumentiert die genaue Auflösung nicht). Fühlt sich das Scratchen zu schnell/langsam an, lässt sich das im Skript unter `Mixon4.JOG.wheelResolution` anpassen.

## Deinstallation

Lösche einfach die beiden Dateien `Reloop-Mixon-4.midi.xml` und `Reloop-Mixon-4.scripts.js` aus deinem `controllers`-Ordner und starte Mixxx neu.

## Mithelfen & Feedback

Dieses Mapping soll ins offizielle Mixxx-Projekt aufgenommen werden. Rückmeldungen von Mixon-4-Besitzern sind sehr willkommen!

- **Fehler oder Wunsch?** [Ein Issue eröffnen](../../issues).
- **Läuft super?** Gern im [Mixxx-Forum](https://mixxx.discourse.group/) berichten.
- Technische Details zum Aufbau: [`docs/architecture.md`](docs/architecture.md) und die vollständige MIDI-Tabelle in [`docs/hardware/midi-map.md`](docs/hardware/midi-map.md).

## Wörterbuch für Einsteiger

- **Deck** – ein virtueller Plattenspieler in Mixxx. Der Mixon 4 steuert bis zu 4 davon.
- **Cue / Hot Cue** – eine Sprungmarke im Song, zu der du per Knopfdruck zurückspringst.
- **PFL (Pre-Fader Listening)** – Vorhören im Kopfhörer, ohne dass das Publikum es hört.
- **Beatgrid** – das von Mixxx erkannte Taktraster eines Songs; Basis für Sync, Loops und Beatjump.
- **Sync** – automatisches Angleichen des Tempos (und der Taktphase) an ein anderes Deck.
- **Loop / Schleife** – ein Abschnitt, der endlos wiederholt wird.
- **Beatjump** – ein exakter Sprung um eine bestimmte Anzahl Beats vor oder zurück.
- **Slip-Modus** – der Song läuft „im Hintergrund" normal weiter, während du z. B. scratchst; danach springt er an die Stelle, an der er ohne dein Eingreifen wäre.
- **Quantisierung** – rastet Aktionen (Cues, Loops) sauber auf den Takt ein.
- **Crossfader** – der waagerechte Regler, der zwischen linker und rechter Seite überblendet.
- **Soft-Takeover** – Schutzmechanismus, damit Regler nicht springen (siehe [oben](#wichtig-zu-wissen-soft-takeover)).

## Lizenz

Veröffentlicht unter der **GPL v2 (oder später)** – dieselbe Lizenz wie Mixxx.
