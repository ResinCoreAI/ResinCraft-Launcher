# ResinCraft Launcher: Benutzerhandbuch

So erledigst du die häufigsten Dinge in ResinCraft. **Fett** gedruckte Namen siehst du genau so auf dem Bildschirm. Ein paar Teile der Server-Seite sind noch nicht übersetzt; dort zeigt die App Englisch, deshalb stehen diese Namen hier auch auf Englisch (mit Übersetzung in Klammern).

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Inhalt

1. [Installieren und anmelden](#1-installieren-und-anmelden)
2. [Deine erste Instanz erstellen](#2-deine-erste-instanz-erstellen)
3. [Mods, Ressourcenpakete und Shader hinzufügen](#3-mods-ressourcenpakete-und-shader-hinzufügen)
4. [Ein Modpack installieren](#4-ein-modpack-installieren)
5. [Minecraft-Version oder Mod-Loader ändern](#5-minecraft-version-oder-mod-loader-ändern)
6. [Wenn Minecraft abstürzt](#6-wenn-minecraft-abstürzt)
7. [Inhalte aus einer anderen Instanz kopieren](#7-inhalte-aus-einer-anderen-instanz-kopieren)
8. [Einen Server hosten](#8-einen-server-hosten)
9. [Freunde auf deinen Server lassen](#9-freunde-auf-deinen-server-lassen)
10. [Whitelist-Anfragen (Webseite und Discord-Bot)](#10-whitelist-anfragen-webseite-und-discord-bot)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Von einem anderen Launcher umziehen](#12-von-einem-anderen-launcher-umziehen)
13. [Updates und der Infobereich](#13-updates-und-der-infobereich)
14. [Fehlerbehebung](#14-fehlerbehebung)

## 1. Installieren und anmelden

1. Lade ResinCraft für dein System von der [Releases-Seite](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest) herunter:
   - **Windows:** `…_amd64_windows.exe`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` oder `.tar.gz`
2. Installiere und öffne es. Windows meldet eventuell, dass der Herausgeber nicht verifiziert ist; unter macOS klickst du beim ersten Mal mit der rechten Maustaste auf die App und wählst **Öffnen**.
3. Öffne **Konten** → **Mit Microsoft anmelden** und melde dich auf Microsofts eigener Seite an. Dein Passwort geht nie durch ResinCraft.
   - Kein Microsoft-Konto? Füge ein **Offline-Profil** für Einzelspieler und LAN hinzu.

## 2. Deine erste Instanz erstellen

Eine *Instanz* ist eine eigene Minecraft-Installation mit eigenen Mods, Welten und Einstellungen.

1. Öffne **Bibliothek** → **Neue Instanz**.
2. Wähle eine **Minecraft-Version** und einen **Mod-Loader**:
   - **Vanilla:** keine Mods.
   - **Fabric** oder **Quilt:** leichte Mods, Performance-Mods.
   - **Forge** oder **NeoForge:** große Content-Mods.
3. Gib ihr einen Namen und klicke auf **Instanz erstellen**.
4. Klicke auf **Spielen**. Beim ersten Start werden das Spiel und das passende Java heruntergeladen; danach geht der Start schnell.

Tipp: **Schnellbeitritt** links zeigt deine letzten Server und Welten; ein Klick, und du bist direkt drin.

## 3. Mods, Ressourcenpakete und Shader hinzufügen

1. Öffne deine Instanz → Tab **Inhalte** → **Mods hinzufügen**. Oder öffne **Entdecken** in der linken Leiste.
2. Suche, oder stöbere auf Modrinth, CurseForge, FTB und den anderen Seiten. Es werden nur Versionen angezeigt, die zu deiner Instanz passen.
3. Klicke auf **Installieren**. Benötigte Mods (Abhängigkeiten) kommen von selbst dazu.
4. Zurück in **Inhalte**:
   - mit dem Schalter neben einem Mod schaltest du ihn ein oder aus;
   - mit **Nach Updates suchen** aktualisierst du Mods (einzeln oder alle).

Manche CurseForge-Autoren erlauben Launchern nicht, ihre Dateien herunterzuladen. ResinCraft listet sie dann unter **Alle herunterladen** auf: Lade sie in deinem Browser herunter, und ResinCraft findet sie von selbst in deinem Downloads-Ordner. Speichert dein Browser woanders? Füge diesen Ordner unter **Einstellungen → Downloads → Überwachte Ordner** hinzu.

## 4. Ein Modpack installieren

1. Öffne **Entdecken** → **Modpacks** (oder **Bibliothek** → **Neue Instanz** → **Modpack herunterladen**).
2. Öffne ein Pack, um seine Beschreibung, seine Versionen und die enthaltenen **Mods** zu sehen.
3. Klicke auf **Pack installieren**. ResinCraft erstellt eine neue Instanz mit allem drin.
4. Bekommt das Pack ein Update, sagt ResinCraft dir Bescheid; du kannst auf der Instanzseite auch eine andere Pack-Version wählen.

## 5. Minecraft-Version oder Mod-Loader ändern

1. Klicke auf der Instanzseite oben auf den Versions-Chip (zum Beispiel **1.21.4 · Fabric 0.16.10**). Er öffnet **Einstellungen → Version**.
2. Wähle die **Minecraft**-Version, den **Mod-Loader** und die Loader-Version.
3. Klicke auf **Version ändern** und bestätige. Beim nächsten Start wird heruntergeladen, was nötig ist.
4. Mods für die alte Version laufen eventuell nicht mehr: Geh zu **Inhalte** und klicke auf **Nach Updates suchen**.

## 6. Wenn Minecraft abstürzt

Stürzt das Spiel ab, öffnet sich das Fenster **Minecraft ist abgestürzt**. ResinCraft liest den Absturzbericht und die Logs und sagt dir, was schiefgelaufen ist. Diese Meldungen zeigt die App noch auf Englisch, zum Beispiel:

- *Sodium Extra needs Sodium, which isn't installed* (Sodium Extra braucht Sodium, das fehlt) → **Beheben** installiert Sodium.
- *citresewn crashed the game* (citresewn hat das Spiel abstürzen lassen) → **Beheben** schaltet diesen Mod aus (später in **Inhalte** wieder einschalten).
- *This needs Java 21 or newer* (braucht Java 21 oder neuer) → **Beheben** lässt ResinCraft das passende Java wählen.
- *Minecraft ran out of memory* (Minecraft hatte zu wenig Arbeitsspeicher) → **Beheben** gibt ihm mehr Speicher.

Klicke bei jedem Punkt auf **Beheben** oder gleich auf **Alles beheben und spielen**. Es ändert sich nichts, bevor du einen Button drückst. Findet ResinCraft die Ursache nicht, zeigt dir **Protokoll öffnen**, was das Spiel zuletzt gemeldet hat.

## 7. Inhalte aus einer anderen Instanz kopieren

1. Öffne die Instanz, *in die* du kopieren willst → klicke oben auf den Button **Aus anderer Instanz kopieren**.
2. Wähle die Instanz, *aus der* du kopierst.
3. Hake an, was du haben willst:
   - **Welten**;
   - **Mods** und **Mod-Einstellungen** (der config-Ordner);
   - **Shaderpacks** und **Ressourcenpakete**;
   - **Spieleinstellungen** (Grafik, Steuerung, Tastenbelegung).
4. Klicke auf **Kopieren**. Welten, deren Name schon vergeben ist, bekommen einen neuen Namen. Mods und Packs, die du schon hast, bleiben, wie sie sind.

## 8. Einen Server hosten

1. Öffne **Servers** → **Create server** (Server erstellen).
2. Wähle, wie:
   - **Server software** (Server-Software): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** dasselbe Pack, das deine Freunde spielen.
   - **From an instance** (aus einer Instanz): die Mods, die du schon benutzt.
   - **From a file** (aus einer Datei).
3. Klicke auf **Create** (Erstellen). Das Fenster schließt sich, und der Download läuft unten rechts weiter.
4. Stimme der Minecraft-EULA zu, wenn du gefragt wirst, und klicke dann auf **Start**.

Auf der Seite eines Servers:

- **Console** (Konsole): was der Server meldet; unten tippst du Befehle ein.
- **Players** (Spieler): wer gerade online ist, dazu Whitelist, Operatoren und Bans.
- **Mods** oder **Plugins:** hinzufügen, aktualisieren oder ausschalten.
- **Properties** (Eigenschaften): die Servereinstellungen (Schwierigkeit, Whitelist an/aus und so weiter).
- **Settings** (Einstellungen):
  - Arbeitsspeicher, Java und **Java flags** (die von ResinCraft oder die eigenen des Modpacks);
  - wie Freunde sich verbinden;
  - das Server-Icon.
- **Neu starten** und das **⋯**-Menü (Adresse kopieren, Ordner öffnen, jetzt sichern, löschen).

Mods, die nur im Spiel-Client laufen (Shader, Minimaps…), werden auf Servern automatisch ausgeschaltet. Du siehst sie im Tab **Mods** und kannst sie bei Bedarf wieder einschalten.

## 9. Freunde auf deinen Server lassen

Auf der Server-Seite unter **Settings** → **How friends connect** (wie Freunde sich verbinden):

- **Automatic** (automatisch, empfohlen): probiert zuerst **ResinCraft Relay**, dann playit.gg, dann bore.
- **ResinCraft Relay:** Freunde verbinden sich mit `yourname.rscraft.resincore.tech`. Dafür muss ein Microsoft-Konto angemeldet sein.
- **playit.gg:** eine kostenlose Adresse, die gleich bleibt. Beim ersten Start wirst du gebeten, ein playit.gg-Konto zu verknüpfen (ein Gastkonto reicht).
- **Port selbst weitergeleitet:** wenn du deinen Router selbst eingerichtet hast.

Im Tab **Console** steht die Adresse, die du deinen Freunden gibst. Auch Freunde mit Bedrock können auf Java-Server: Schalte **Bedrock players can join** (Bedrock-Spieler können beitreten) ein.

## 10. Whitelist-Anfragen (Webseite und Discord-Bot)

Lass Spieler fragen, ob sie beitreten dürfen, und entscheide selbst, wen du annimmst oder ablehnst.

1. Setze in **Properties** den Eintrag **white-list** auf **true** (sonst kann sowieso jeder beitreten).
2. Öffne den Tab **Players** des Servers und schalte **Whitelist requests** (Whitelist-Anfragen) ein.
3. Wähle, wie Spieler fragen:
   - **Web page** (Webseite):
     - Funktioniert, wenn der Server über ResinCraft Relay geteilt wird.
     - Klicke auf **Copy link** (Link kopieren) und schick den Spielern den Link: `https://rscraft.resincore.tech/join/<name>`.
     - Die Seite funktioniert, solange der Server online ist.
   - **Your Discord bot** (dein Discord-Bot):
     1. Öffne das [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token** und kopiere den Token.
     2. Füge ihn in ResinCraft ein und klicke auf **Save** (Speichern). Er wird sicher auf deinem PC verwahrt und nie wieder angezeigt.
     3. Sobald dort **Online as …** steht, klicke auf **Add the bot to your Discord** (Bot zu deinem Discord hinzufügen) und wähle deinen Discord-Server.
     4. Spieler tippen in deinem Discord `/whitelist TheirName` (mit ihrem eigenen Minecraft-Namen).
4. Anfragen erscheinen im Tab **Players**:
   - **Accept** (annehmen): setzt den Spieler auf die Whitelist (der Bot sagt es ihm per Direktnachricht).
   - **Reject** (ablehnen): lehnt die Anfrage ab.
   - **Reject and block** (ablehnen und blockieren): lehnt ab und verhindert, dass diese Person erneut fragt. **Unblock all** (alle entsperren) hebt Discord-Sperren wieder auf.

Anfragen von gebannten Spielern werden automatisch abgelehnt. Der Discord-Bot arbeitet, solange ResinCraft offen ist; schalte den Infobereich ein (Abschnitt 13), damit er auch bei geschlossenem Fenster weiterläuft.

## 11. Bedrock Edition

Unter Windows installiert, öffnet und deinstalliert **Bedrock** in der linken Leiste Minecraft für Windows und Minecraft Preview. Vor dem Deinstallieren werden deine Welten gesichert.

## 12. Von einem anderen Launcher umziehen

**Bibliothek** → **Von einem anderen Launcher umziehen** holt Instanzen aus:

- Prism Launcher und MultiMC
- der Modrinth App und der CurseForge-App
- ATLauncher und GDLauncher
- dem offiziellen Minecraft Launcher

Ihre Mods, Welten und Einstellungen kommen mit. Die Originale bleiben, wo sie sind.

## 13. Updates und der Infobereich

- **Updates:**
  - Ist eine neue ResinCraft-Version bereit, erscheint in der Titelleiste ein **Update**-Button.
  - Läuft gerade ein Spiel oder Server, wählst du **Schließen und aktualisieren** (Server speichern vorher ihre Welten) oder **Aktualisieren, sobald sie gestoppt sind**.
- **Still aktualisieren** (Einstellungen → Updates): installiert neue Versionen im Hintergrund, ohne zu fragen, sobald nichts mehr läuft.
- **Im Infobereich weiterlaufen** (Einstellungen → Darstellung):
  - Wenn du das Fenster schließt, läuft ResinCraft weiter: Server, der Discord-Bot und Downloads arbeiten weiter.
  - Klicke auf das Symbol im Infobereich, um es wieder zu öffnen; Rechtsklick → **Quit ResinCraft** (ResinCraft beenden) beendet es wirklich.

## 14. Fehlerbehebung

- **Das Spiel startet nicht:** Schau dir das Fenster **Minecraft ist abgestürzt** an (Abschnitt 6) oder den Tab **Logs** der Instanz.
- **Ein Mod taucht in Entdecken nicht auf:** Vielleicht gibt es ihn für deine Minecraft-Version oder deinen Loader nicht. Probiere eine andere Version, oder ändere die Version der Instanz (Abschnitt 5).
- **Freunde können nicht beitreten:** Prüfe die Adresse im Tab **Console** des Servers und ob der Server **Running** (läuft) anzeigt. Mit ResinCraft Relay: Achte darauf, dass du mit einem Microsoft-Konto angemeldet bist.
- **Der Server stürzt beim Start ab:** ResinCraft schaltet reine Client-Mods von selbst aus und startet ihn neu. In der **Console** steht in einer Zeile, welchen Mod es ausgeschaltet hat.
- **Kommst du immer noch nicht weiter?** Melde ein Problem auf [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) und häng das Log aus dem Tab **Logs** an.
