# ResinCraft Launcher: guida all’uso

Come fare le cose più comuni in ResinCraft. I nomi in **grassetto** sono quelli che vedrai sullo schermo. Alcune parti della pagina dei server non sono ancora tradotte e l’app le mostra in inglese; per questo qui compaiono anche in inglese (con la traduzione tra parentesi). Ogni sezione indica in quale versione è arrivata.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Indice

1. [Installa e accedi](#1-installa-e-accedi)
2. [Crea la tua prima istanza](#2-crea-la-tua-prima-istanza)
3. [Aggiungi mod, pacchetti di risorse e shader](#3-aggiungi-mod-pacchetti-di-risorse-e-shader)
4. [Installa un modpack](#4-installa-un-modpack)
5. [Cambia la versione di Minecraft o il mod loader](#5-cambia-la-versione-di-minecraft-o-il-mod-loader)
6. [Quando Minecraft va in crash](#6-quando-minecraft-va-in-crash)
7. [Copia da un’altra istanza](#7-copia-da-unaltra-istanza)
8. [Ospita un server](#8-ospita-un-server)
9. [Fai entrare gli amici nel tuo server](#9-fai-entrare-gli-amici-nel-tuo-server)
10. [Richieste di whitelist (pagina web e bot Discord)](#10-richieste-di-whitelist-pagina-web-e-bot-discord)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Trasferisciti da un altro launcher](#12-trasferisciti-da-un-altro-launcher)
13. [Aggiornamenti e area di notifica](#13-aggiornamenti-e-area-di-notifica)
14. [Temi e CSS personalizzato](#14-temi-e-css-personalizzato)
15. [Controllo remoto (console web)](#15-controllo-remoto-console-web)
16. [Reinstalla e ripara](#16-reinstalla-e-ripara)
17. [Trova i server ed entra](#17-trova-i-server-ed-entra)
18. [Metti in elenco il tuo server e condividi le sue mod](#18-metti-in-elenco-il-tuo-server-e-condividi-le-sue-mod)
19. [I tuoi altri PC e gli amici che aiutano](#19-i-tuoi-altri-pc-e-gli-amici-che-aiutano)
20. [Backup dei mondi](#20-backup-dei-mondi)
21. [Archiviazione e passaggio a un altro PC](#21-archiviazione-e-passaggio-a-un-altro-pc)
22. [Risoluzione dei problemi](#22-risoluzione-dei-problemi)

## 1. Installa e accedi

*Dalla versione 0.1.0.*

1. Scarica ResinCraft per il tuo sistema dalla [pagina delle release](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest):
   - **Windows:** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (installa sempre la versione più recente), oppure `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS:** `…_universal_macos.dmg`
   - **Linux:** `.AppImage`, `.deb` o `.tar.gz`
2. Installalo e aprilo. Windows potrebbe dire che l’editore non è verificato; su macOS, la prima volta fai clic destro sull’app e scegli **Apri**.
3. Apri **Account** → **Accedi con Microsoft** e accedi sulla pagina di Microsoft. La tua password non passa mai da ResinCraft.
   - Non hai un account Microsoft? Aggiungi un **Profilo offline** per giocatore singolo e LAN.

## 2. Crea la tua prima istanza

*Dalla versione 0.1.0.*

Un’*istanza* è un’installazione di Minecraft separata, con mod, mondi e impostazioni propri.

1. Apri **Libreria** → **Nuova istanza**.
2. Scegli una **Versione di Minecraft** e un **Loader di mod**:
   - **Vanilla:** nessuna mod.
   - **Fabric** o **Quilt:** mod leggere, mod per le prestazioni.
   - **Forge** o **NeoForge:** grandi mod di contenuti.
3. Dagli un nome e premi **Crea istanza**.
4. Premi **Gioca**. Il primo avvio scarica il gioco e il Java giusto; quelli dopo sono veloci.

Suggerimento: **Accesso rapido**, a sinistra, elenca i tuoi server e mondi recenti; un clic e sei subito dentro.

## 3. Aggiungi mod, pacchetti di risorse e shader

*Dalla versione 0.1.0.*

1. Apri la tua istanza → scheda **Contenuti** → **Aggiungi mod**. Oppure apri **Scopri** dalla barra a sinistra.
2. Cerca, oppure sfoglia Modrinth, CurseForge, FTB e gli altri siti. Vengono mostrate solo le versioni adatte alla tua istanza.
3. Premi **Installa**. Le mod richieste (dipendenze) vengono aggiunte per te.
4. Di nuovo in **Contenuti**:
   - usa l’interruttore accanto a una mod per attivarla o disattivarla;
   - premi **Cerca aggiornamenti** per aggiornare le mod (una o tutte). **Aggiorna tutto** ne scarica fino a quattro alla volta.

Alcuni autori di CurseForge non permettono ai launcher di scaricare i loro file. In quel caso ResinCraft li elenca sotto **Scarica tutto**: scaricali con il browser e ResinCraft li trova da solo nella cartella Download. Il tuo browser salva altrove? Aggiungi quella cartella in **Impostazioni → Download → Cartelle controllate**.

## 4. Installa un modpack

*Dalla versione 0.1.0.*

1. Apri **Scopri** → **Modpack** (oppure **Libreria** → **Nuova istanza** → **Scarica un modpack**).
2. Apri un pack per vedere la descrizione, le versioni e le **Mod** che contiene.
3. Premi **Installa modpack**. La nuova istanza compare subito nella **Libreria**, con l’avanzamento dell’installazione sulla sua scheda; **Gioca** funziona quando ha finito.
4. Quando il pack riceve un aggiornamento, ResinCraft te lo dice; puoi anche scegliere un’altra versione del pack nella pagina dell’istanza.

## 5. Cambia la versione di Minecraft o il mod loader

*Dalla versione 1.2.7.*

1. Nella pagina dell’istanza, fai clic sul chip della versione in alto (per esempio **1.21.4 · Fabric 0.16.10**). Si apre **Impostazioni → Versione**.
2. Scegli la versione di **Minecraft**, il **Mod loader** e la versione del loader.
3. Premi **Cambia versione** e conferma. Al prossimo avvio viene scaricato quello che serve.
4. Le mod fatte per la vecchia versione potrebbero non funzionare: vai in **Contenuti** e premi **Cerca aggiornamenti**.

## 6. Quando Minecraft va in crash

*Dalla versione 1.2.7.*

Quando il gioco va in crash, si apre la finestra **Minecraft si è chiuso per errore**. ResinCraft legge il crash report e i log e ti dice cosa è andato storto. Per ora l’app mostra questi messaggi in inglese, per esempio:

- *Sodium Extra needs Sodium, which isn't installed* (Sodium Extra richiede Sodium, che non è installato) → **Risolvi** installa Sodium.
- *citresewn crashed the game* (citresewn ha mandato in crash il gioco) → **Risolvi** disattiva quella mod (riattivala più tardi in **Contenuti**).
- *This needs Java 21 or newer* (serve Java 21 o più recente) → **Risolvi** lascia che ResinCraft scelga il Java giusto.
- *Minecraft ran out of memory* (Minecraft ha finito la memoria) → **Risolvi** gli dà più memoria.

Premi **Risolvi** su ognuno, oppure **Risolvi tutto e gioca**. Non cambia niente finché non premi un pulsante. Se ResinCraft non capisce il perché, **Apri log** mostra l’ultima cosa che ha detto il gioco.

## 7. Copia da un’altra istanza

*Dalla versione 1.2.5.*

1. Apri l’istanza *in cui* vuoi copiare → premi il pulsante **Copia da un’altra istanza** in alto.
2. Scegli l’istanza *da cui* copiare.
3. Spunta quello che vuoi:
   - **Mondi**;
   - **Mod** e **Impostazioni delle mod** (la cartella config);
   - **Shader pack** e **Pacchetti risorse**;
   - **Impostazioni di gioco** (grafica, controlli, tasti).
4. Premi **Copia**. I mondi con un nome già usato ricevono un nome nuovo. Le mod e i pack che hai già restano come sono.

## 8. Ospita un server

*Dalla versione 0.1.0.*

1. Apri **Servers** → **Create server** (crea server).
2. Scegli come:
   - **Server software** (software del server): Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack:** lo stesso pack a cui giocano i tuoi amici.
   - **From an instance** (da un’istanza): le mod che usi già.
   - **From a file** (da un file).
3. Premi **Create** (crea). La finestra si chiude e il download continua in basso a destra.
4. Accetta l’EULA di Minecraft quando te lo chiede, poi premi **Start** (avvia).

Nella pagina di un server:

- **Console:** quello che dice il server; scrivi i comandi in basso.
- **Players** (giocatori): chi è online in questo momento, più whitelist, operatori e ban.
- **Mods** o **Plugins:** aggiungi, aggiorna o disattiva. **Aggiorna tutto** ne scarica fino a quattro alla volta; mentre il server è in esecuzione, gli aggiornamenti vengono applicati al prossimo riavvio.
- **Properties** (proprietà): le impostazioni del server (difficoltà, whitelist attiva o no, e così via).
- **Settings** (impostazioni):
  - memoria, Java e **Java flags** (quelli di ResinCraft o quelli del modpack);
  - come si collegano gli amici;
  - l’icona del server.
- **Riavvia** e il menu **⋯** (copia indirizzo, apri cartella, backup ora, elimina).

- **Settings → Anti-xray** (impostazioni → anti-xray; Paper, Purpur, Folia, Leaf): nascondi i minerali alle mod e ai pacchetti di texture x-ray. **Hide ores** (nascondi minerali) pesa poco sul server; **Fake ores** (minerali finti) è più efficace. Vale dal prossimo avvio.
- **Settings → Anti-cheat** (impostazioni → anticheat; server con plugin): un solo interruttore installa Grim.

Le mod che funzionano solo sul client del gioco (shader, minimappe…) restano sul server finché si avvia senza problemi. Se una lo manda in crash, ResinCraft disattiva quella mod e riavvia il server; dalla scheda **Mods** puoi riattivarla.

Quando un modpack non ha un server pack ufficiale, ResinCraft costruisce il server dall’elenco di mod del pack: la finestra dopo **Create** (crea) te lo dice, e il server mostra il badge **Built by ResinCraft** (creato da ResinCraft). Alcuni file del pack si possono scaricare solo dal sito del loro autore. Sono elencati in cima alla scheda **Console**, con **Get file** (scarica il file). Scaricali con il browser e ResinCraft li mette al loro posto prima del prossimo avvio, oppure premi **Look again** (cerca di nuovo).

## 9. Fai entrare gli amici nel tuo server

*Dalla versione 0.2.0.*

Nella scheda **Settings** del server → **How friends connect** (come si collegano gli amici):

- **Automatic** (automatico, consigliato e predefinito per i nuovi server): prima il tuo router, poi **ResinCraft Relay**, poi playit.gg.
- **ResinCraft Relay:** gli amici entrano con `yourname.rscraft.resincore.tech`. Serve un account Microsoft collegato.
- **playit.gg:** un indirizzo gratuito che resta sempre lo stesso. Al primo avvio ti chiede di collegare un account playit.gg (va bene anche un account ospite).
- **Porta inoltrata da me:** se hai configurato il router da solo.

La scheda **Console** mostra l’indirizzo da dare agli amici. Anche gli amici su Bedrock possono entrare nei server Java: attiva **Bedrock players can join** (i giocatori Bedrock possono entrare).

**Il tuo dominio:** con ResinCraft Relay, attiva **Collega il tuo dominio a ResinCraft Relay** e scrivi il dominio. Aggiungi i record DNS che mostra, compreso il record TXT che dimostra che il dominio è tuo, poi premi **Verifica**. Il badge passa da **In attesa di verifica** a **Verificato**. Con il DNS automatico di Cloudflare attivo, ResinCraft aggiunge i record per te.

## 10. Richieste di whitelist (pagina web e bot Discord)

*Dalla versione 1.2.7.*

Lascia che i giocatori chiedano di entrare, e accettali o rifiutali tu.

1. In **Properties**, imposta **white-list** su **true** (altrimenti chiunque può entrare comunque).
2. Apri la scheda **Players** del server e attiva **Whitelist requests** (richieste di whitelist).
3. Scegli come chiedono i giocatori:
   - **Web page** (pagina web):
     - Funziona quando il server è condiviso tramite ResinCraft Relay.
     - Premi **Copy link** (copia link) e manda ai giocatori il link. È l’indirizzo del tuo server, o il tuo dominio verificato.
     - La pagina funziona finché il server è online.
   - **Your Discord bot** (il tuo bot Discord):
     1. Apri il [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token** e copia il token.
     2. Incollalo in ResinCraft e premi **Save** (salva). Viene conservato al sicuro sul tuo PC e non viene più mostrato.
     3. Quando compare **Online as …**, premi **Add the bot to your Discord** (aggiungi il bot al tuo Discord) e scegli il tuo server Discord.
     4. I giocatori scrivono `/whitelist TheirName` nel tuo Discord (con il loro nome di Minecraft).
4. Le richieste compaiono nella scheda **Players**:
   - **Accept** (accetta): mette il giocatore in whitelist (il bot glielo dice con un messaggio diretto).
   - **Reject** (rifiuta): rifiuta la richiesta.
   - **Reject and block** (rifiuta e blocca): la rifiuta e impedisce a quella persona di chiedere di nuovo. I giocatori bloccati compaiono nell’elenco **Blocked** (bloccati), con **Unblock** (sblocca) per ognuno.
   - **Dall’elenco dei server:** i giocatori con ResinCraft possono chiedere dalla pagina del tuo server in Scopri → Server (sezione 17).

Le richieste dei giocatori bannati vengono rifiutate in automatico. Il bot Discord funziona finché ResinCraft è aperto; attiva l’area di notifica (sezione 13) per farlo funzionare anche a finestra chiusa.

## 11. Bedrock Edition

*Dalla versione 0.3.1.*

Su Windows, **Bedrock** a sinistra installa, apre e disinstalla Minecraft per Windows e Minecraft Preview. Prima di disinstallare fa un backup dei tuoi mondi.

## 12. Trasferisciti da un altro launcher

*Dalla versione 1.0.5.*

**Libreria** → **Trasferisci da un altro launcher** importa istanze da:

- Prism Launcher e MultiMC
- Modrinth App e app di CurseForge
- ATLauncher e GDLauncher
- il Minecraft Launcher ufficiale

Mod, mondi e impostazioni vengono con loro. Gli originali restano dove sono.

## 13. Aggiornamenti e area di notifica

*Dalla versione 0.3.0 (l’area di notifica dalla 1.2.7).*

- **Aggiornamenti:**
  - Quando una nuova versione di ResinCraft è pronta, nella barra del titolo compare un pulsante **Aggiorna**.
  - Se è in esecuzione un gioco o un server, scegli **Chiudili e aggiorna** (i server salvano prima i loro mondi) oppure **Aggiorna quando si fermano**.
- **Aggiorna in silenzio** (Impostazioni → Aggiornamenti): installa le nuove versioni in background, senza chiedere, appena non c’è niente in esecuzione.
- **Resta nell’area di notifica** (Impostazioni → Aspetto):
  - Chiudendo la finestra ResinCraft resta attivo: server, bot Discord e download continuano.
  - Fai clic sull’icona nell’area di notifica per riaprirlo; clic destro → **Quit ResinCraft** (esci da ResinCraft) per chiuderlo davvero.

## 14. Temi e CSS personalizzato

*Dalla versione 1.3.0.*

1. Apri **Impostazioni → Aspetto → Tema** e scegli una scheda: **Sistema**, Scuro, Chiaro, Ossidiana, Ardesia abissale, Ciliegio, Nether o L’End.
2. **CSS personalizzato:** premi **Inserisci i colori attuali**, cambia i colori che vuoi, poi premi **Applica**. Funziona sopra qualsiasi tema; **Svuota** lo rimuove.
3. Apri **Come personalizzare** sotto la casella: elenca i colori e le forme che puoi cambiare, le parti dell’app a cui puoi dare uno stile ed esempi (un’immagine di sfondo, angoli più arrotondati, testo più grande…). Fai clic su uno per aggiungerlo al tuo CSS.

Psst: prova il codice Konami (↑ ↑ ↓ ↓ ← → ← → B A), oppure fai clic sette volte sul logo di ResinCraft.

## 15. Controllo remoto (console web)

*Dalla versione 1.3.0.*

Gestisci i tuoi server da qualsiasi browser, o dal telefono.

1. Accedi con un account Microsoft in **Account**.
2. Attiva **Impostazioni → Controllo remoto → Consenti il controllo remoto**. Quando dice **Connesso come …**, sei pronto.
3. Attiva **Resta nell’area di notifica** (sezione 13), così continua a funzionare anche dopo che chiudi la finestra.
4. Apri `https://console.rscraft.resincore.tech` (oppure premi **Apri la console web**) e accedi con **lo stesso account Minecraft**.

Dalla console puoi fare tutto quello che fa la gestione server: avviare, fermare e riavviare i server, seguire la console e scrivere comandi, gestire i giocatori e le richieste di whitelist, aggiungere e aggiornare plugin e mod, cambiare proprietà e impostazioni, cambiare la versione del modpack, reinstallare, fare backup e creare nuovi server. Ogni modifica compare come notifica in ResinCraft.

Il file manager, le impostazioni di Java e i plugin da qualsiasi link di download possono avviare programmi sul tuo PC, quindi richiedono un secondo interruttore: **Consenti il controllo completo**. Attivalo solo se il tuo account Minecraft (Microsoft) ha la verifica in due passaggi. Senza, la console non può mai arrivare a nient’altro sul tuo PC.

## 16. Reinstalla e ripara

*Dalla versione 1.3.0.*

- **Un’istanza non parte perché mancano file o sono danneggiati:**
  - Fai clic destro su di essa nella **Libreria** → **Reinstalla** → **File del gioco**, oppure apri le sue **Impostazioni → Ripara**.
  - Minecraft, il mod loader, le librerie e le risorse vengono scaricati di nuovo. Mondi, mod e impostazioni restano.
- **I file di un modpack sono stati modificati o eliminati:** **Reinstalla** → **File del modpack** rimette le mod, le configurazioni e gli script del pack. I file che hai modificato vengono salvati prima; i mondi e le tue mod restano.
- **Un server:** il suo menu **⋯** → **Reinstalla** → **Software del server** (oppure **File del modpack** per un server da modpack). Mondo, impostazioni e liste dei giocatori restano.
- **ResinCraft stesso:** **Impostazioni → Aggiornamenti → Reinstalla ResinCraft** reinstalla l’app sopra sé stessa.

## 17. Trova i server ed entra

*Dalla versione 1.4.0.*

1. Apri **Scopri** → **Server**. Elenca i server ospitati con ResinCraft e quelli aggiunti dai giocatori.
2. Cerca, oppure filtra per categoria (SMP, PvP, Minigiochi, Skyblock…), regione, lingua, modpack, e se possono entrare giocatori Bedrock o profili offline.
3. Fai clic sull’icona o sul nome di un server per aprire la sua pagina: la descrizione completa, tutti i dettagli, il suo pack client e i link.
4. Premi **Entra**. ResinCraft sceglie l’istanza adatta al server, oppure ti chiede quale usare. Un server con mod proprie ha **Installa ed entra**, che crea un’istanza con esattamente quelle mod ed entra.
5. Server con whitelist? Premi **Chiedi di entrare**. Il pulsante mostra **Waiting for the host** (in attesa dell’host), poi **On the whitelist** (in whitelist; premi **Entra**) o **Ask again** (chiedi di nuovo). ResinCraft ti avvisa, e fa lampeggiare la barra delle applicazioni, quando l’host risponde.

Metti mi piace ai server che ti piacciono, e segnala quelli che non rispettano le regole.

## 18. Metti in elenco il tuo server e condividi le sue mod

*Dalla versione 1.4.0.*

1. Nella scheda **Settings** (impostazioni) di un server ospitato, attiva **Elenco server** (è disattivato di default).
2. Aggiungi descrizione, tag, regione e lingua. Il supporto per Bedrock e per i profili offline e il modpack si compilano da soli.
3. **Pack client:** pubblica quello che serve ai giocatori, così possono usare **Installa ed entra**:
   - il modpack del server, installato dal suo sito;
   - ogni altra mod, da Modrinth o CurseForge;
   - i tuoi file, inviati direttamente dal tuo PC mentre ResinCraft è in esecuzione;
   - i pacchetti di risorse e gli shader che scegli di mandare insieme.
   Anche i server con plugin e vanilla possono averne uno: scegli un’istanza con la versione di Minecraft del server.

Un server che fai girare altrove? Aggiungilo con **Invia un server** in Scopri → Server.

## 19. I tuoi altri PC e gli amici che aiutano

*Dalla versione 1.4.1.*

**I tuoi altri PC:** attiva il controllo remoto su ogni PC (sezione 15) e accedi con lo stesso account Minecraft. Poi, in **Servers**, apri il menu dei PC in alto e scegli un PC. Da qui puoi avviare, fermare e gestire i suoi server, e anche usare lì **Create server** (crea server) o **Importa un server**. Per importare da una cartella su quel PC, scrivi la cartella; serve **Consenti il controllo completo** su quel PC.

**Amici che ti aiutano a gestire i server:**

1. Apri **Impostazioni → Controllo remoto → People who can use your servers** (persone che possono usare i tuoi server).
2. Scrivi il loro nome di Minecraft e premi **Add** (aggiungi).
3. Scegli cosa possono fare:
   - **Viewer** (osservatore): vedere la console, i giocatori e le impostazioni.
   - **Operator** (operatore): anche avviare, fermare e riavviare, inviare comandi e gestire i giocatori e le richieste di whitelist.
   - **Manager** (gestore): anche cambiare impostazioni, plugin e mod, e il modpack.
4. Lascia **All servers** (tutti i server), oppure scegli i server sotto **Which servers?** (quali server?).

Aprono i tuoi server dal loro ResinCraft (il menu dei PC in **Servers**) o dalla console web, mentre il controllo remoto è attivo e ResinCraft è in esecuzione sul tuo PC. I file, le impostazioni di Java e la creazione o l’eliminazione dei server restano tuoi.

## 20. Backup dei mondi

*Dalla versione 1.4.0.*

- **Automatici:** mentre un gioco o un server è in esecuzione, i suoi mondi vengono salvati ogni 30 minuti, tenendo i 3 backup più recenti per mondo. Cambialo o disattivalo in **Impostazioni → Backup automatico**.
- **Ripristina:** nella scheda **Mondi** di un’istanza, o in **Settings → Backups** (impostazioni → backup) per un server ospitato, premi **Ripristina** su un backup, tuo o automatico. Il mondo sostituito finisce nel Cestino, quindi un ripristino si può annullare. Un mondo in giocatore singolo può anche tornare come copia.

## 21. Archiviazione e passaggio a un altro PC

*Dalla versione 1.1.0 (Passa a un altro PC dalla 1.1.1).*

- **Un altro disco:** **Impostazioni → Archiviazione → Cambia…**, scegli un disco o una cartella, poi:
  - **Sposta istanze:** sposta lì le tue istanze, i server e i file condivisi di Minecraft.
  - **Usa senza spostare:** le cose nuove vanno lì; le istanze attuali restano e tornano quando torni indietro.
- **Un nuovo PC:** **Impostazioni → Passa a un altro PC** salva tutto in un unico file `.resincraft`. Aprilo con ResinCraft sul nuovo PC.

## 22. Risoluzione dei problemi

- **Il gioco non parte:** guarda la finestra **Minecraft si è chiuso per errore** (sezione 6) o la scheda **Log** dell’istanza. Mancano file o sono danneggiati? Reinstallali (sezione 16).
- **Una mod non compare in Scopri:** forse non esiste per la tua versione di Minecraft o per il tuo loader. Prova un’altra versione, o cambia la versione dell’istanza (sezione 5).
- **Gli amici non riescono a entrare:** controlla l’indirizzo nella scheda **Console** del server e che il server dica **Running** (in esecuzione). Con ResinCraft Relay, assicurati di aver fatto l’accesso con un account Microsoft.
- **Il server va in crash all’avvio:** ResinCraft disattiva da solo le mod solo client e lo riavvia. Cerca nella **Console** la riga che dice quale mod ha disattivato. Se dice che mancano dei file, prendili dall’elenco in cima alla scheda **Console**.
- **Ancora bloccato?** Segnala un problema su [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) allegando il log della scheda **Log**.
