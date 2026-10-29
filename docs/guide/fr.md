# ResinCraft Launcher : guide d’utilisation

Comment faire les choses les plus courantes dans ResinCraft. Les noms en **gras** sont ceux que vous verrez à l’écran. Certaines parties de la page des serveurs ne sont pas encore traduites et l’application les affiche en anglais ; elles apparaissent donc aussi en anglais ici (avec la traduction entre parenthèses). Chaque section indique la version dans laquelle elle est arrivée.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

## Sommaire

1. [Installer et se connecter](#1-installer-et-se-connecter)
2. [Créer votre première instance](#2-créer-votre-première-instance)
3. [Ajouter des mods, packs de ressources et shaders](#3-ajouter-des-mods-packs-de-ressources-et-shaders)
4. [Installer un modpack](#4-installer-un-modpack)
5. [Changer la version de Minecraft ou le chargeur de mods](#5-changer-la-version-de-minecraft-ou-le-chargeur-de-mods)
6. [Quand Minecraft plante](#6-quand-minecraft-plante)
7. [Copier depuis une autre instance](#7-copier-depuis-une-autre-instance)
8. [Héberger un serveur](#8-héberger-un-serveur)
9. [Laisser vos amis rejoindre votre serveur](#9-laisser-vos-amis-rejoindre-votre-serveur)
10. [Demandes de whitelist (page web et bot Discord)](#10-demandes-de-whitelist-page-web-et-bot-discord)
11. [Bedrock Edition](#11-bedrock-edition)
12. [Venir d’un autre launcher](#12-venir-dun-autre-launcher)
13. [Mises à jour et zone de notification](#13-mises-à-jour-et-zone-de-notification)
14. [Thèmes et CSS personnalisé](#14-thèmes-et-css-personnalisé)
15. [Contrôle à distance (console web)](#15-contrôle-à-distance-console-web)
16. [Réinstaller et réparer](#16-réinstaller-et-réparer)
17. [Trouver et rejoindre des serveurs](#17-trouver-et-rejoindre-des-serveurs)
18. [Référencer votre serveur et partager ses mods](#18-référencer-votre-serveur-et-partager-ses-mods)
19. [Vos autres PC et les amis qui aident](#19-vos-autres-pc-et-les-amis-qui-aident)
20. [Sauvegardes des mondes](#20-sauvegardes-des-mondes)
21. [Stockage et passage à un autre PC](#21-stockage-et-passage-à-un-autre-pc)
22. [Dépannage](#22-dépannage)

## 1. Installer et se connecter

*Depuis la version 0.1.0.*

1. Téléchargez ResinCraft pour votre système depuis la [page des versions](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/latest) :
   - **Windows :** [`ResinCraft-Setup.exe`](https://github.com/ResinCoreAI/ResinCraft-Launcher/releases/download/setup/ResinCraft-Setup.exe) (installe toujours la dernière version), ou `…_amd64_windows.exe` / `…_amd64_windows.msi`
   - **macOS :** `…_universal_macos.dmg`
   - **Linux :** `.AppImage`, `.deb` ou `.tar.gz`
2. Installez-le et ouvrez-le. Windows peut indiquer que l’éditeur n’est pas vérifié ; sur macOS, la première fois, faites un clic droit sur l’application et choisissez **Ouvrir**.
3. Ouvrez **Comptes** → **Se connecter avec Microsoft**, et connectez-vous sur la page de Microsoft elle-même. Votre mot de passe ne passe jamais par ResinCraft.
   - Pas de compte Microsoft ? Ajoutez un **Profil hors ligne** pour le solo et le LAN.

## 2. Créer votre première instance

*Depuis la version 0.1.0.*

Une *instance* est une installation de Minecraft à part, avec ses propres mods, mondes et réglages.

1. Ouvrez **Bibliothèque** → **Nouvelle instance**.
2. Choisissez une **Version de Minecraft** et un **Chargeur de mods** :
   - **Vanilla :** sans mods.
   - **Fabric** ou **Quilt :** mods légers, mods de performance.
   - **Forge** ou **NeoForge :** gros mods de contenu.
3. Donnez-lui un nom et cliquez sur **Créer l’instance**.
4. Cliquez sur **Jouer**. Le premier lancement télécharge le jeu et le bon Java ; les suivants sont rapides.

Astuce : **Connexion rapide**, à gauche, liste vos serveurs et mondes récents ; un clic et vous y êtes.

## 3. Ajouter des mods, packs de ressources et shaders

*Depuis la version 0.1.0.*

1. Ouvrez votre instance → onglet **Contenu** → **Ajouter des mods**. Ou ouvrez **Découvrir** dans la barre de gauche.
2. Cherchez, ou parcourez Modrinth, CurseForge, FTB et les autres sites. Seules les versions compatibles avec votre instance sont affichées.
3. Cliquez sur **Installer**. Les mods requis (dépendances) sont ajoutés pour vous.
4. De retour dans **Contenu** :
   - utilisez l’interrupteur à côté d’un mod pour l’activer ou le désactiver ;
   - cliquez sur **Rechercher des mises à jour** pour mettre à jour les mods (un ou tous). **Tout mettre à jour** en télécharge jusqu’à quatre à la fois.

Certains auteurs CurseForge n’autorisent pas les launchers à télécharger leurs fichiers. ResinCraft les liste alors sous **Tout télécharger** : téléchargez-les dans votre navigateur, et ResinCraft les trouve tout seul dans votre dossier Téléchargements. Votre navigateur enregistre ailleurs ? Ajoutez ce dossier dans **Paramètres → Téléchargements → Dossiers surveillés**.

## 4. Installer un modpack

*Depuis la version 0.1.0.*

1. Ouvrez **Découvrir** → **Modpacks** (ou **Bibliothèque** → **Nouvelle instance** → **Télécharger un modpack**).
2. Ouvrez un pack pour voir sa description, ses versions et les **Mods** qu’il contient.
3. Cliquez sur **Installer le pack**. La nouvelle instance apparaît tout de suite dans la **Bibliothèque**, avec la progression de l’installation sur sa carte ; **Jouer** fonctionne une fois l’installation terminée.
4. Quand le pack reçoit une mise à jour, ResinCraft vous prévient ; vous pouvez aussi choisir une autre version du pack sur la page de l’instance.

## 5. Changer la version de Minecraft ou le chargeur de mods

*Depuis la version 1.2.7.*

1. Sur la page de l’instance, cliquez sur la pastille de version en haut (par exemple **1.21.4 · Fabric 0.16.10**). Elle ouvre **Paramètres → Version**.
2. Choisissez la version de **Minecraft**, le **Chargeur de mods** et la version du chargeur.
3. Cliquez sur **Changer de version** et confirmez. Le prochain lancement télécharge ce qu’il faut.
4. Les mods faits pour l’ancienne version risquent de ne plus marcher : allez dans **Contenu** et cliquez sur **Rechercher des mises à jour**.

## 6. Quand Minecraft plante

*Depuis la version 1.2.7.*

Quand le jeu plante, une fenêtre **Minecraft a planté** s’ouvre. ResinCraft lit le rapport de plantage et les journaux et vous dit ce qui s’est passé. Pour l’instant, l’application affiche ces messages en anglais, par exemple :

- *Sodium Extra needs Sodium, which isn't installed* (Sodium Extra a besoin de Sodium, qui n’est pas installé) → **Corriger** installe Sodium.
- *citresewn crashed the game* (citresewn a fait planter le jeu) → **Corriger** désactive ce mod (réactivez-le plus tard dans **Contenu**).
- *This needs Java 21 or newer* (il faut Java 21 ou plus récent) → **Corriger** laisse ResinCraft choisir le bon Java.
- *Minecraft ran out of memory* (Minecraft a manqué de mémoire) → **Corriger** lui donne plus de mémoire.

Cliquez sur **Corriger** pour chacun, ou sur **Tout corriger et jouer**. Rien ne change tant que vous n’avez pas cliqué sur un bouton. Si ResinCraft ne trouve pas la cause, **Ouvrir les journaux** montre ce que le jeu a dit en dernier.

## 7. Copier depuis une autre instance

*Depuis la version 1.2.5.*

1. Ouvrez l’instance *dans laquelle* vous voulez copier → cliquez sur le bouton **Copier depuis une autre instance** en haut.
2. Choisissez l’instance *depuis laquelle* copier.
3. Cochez ce que vous voulez :
   - **Mondes** ;
   - **Mods** et **Réglages des mods** (le dossier config) ;
   - **Shaders** et **Packs de ressources** ;
   - **Réglages du jeu** (graphismes, contrôles, touches).
4. Cliquez sur **Copier**. Les mondes dont le nom est déjà pris reçoivent un nouveau nom. Les mods et packs que vous avez déjà restent tels quels.

## 8. Héberger un serveur

*Depuis la version 0.1.0.*

1. Ouvrez **Servers** → **Create server** (créer un serveur).
2. Choisissez comment :
   - **Server software** (logiciel serveur) : Paper, Fabric, Forge, NeoForge, Vanilla, Bedrock…
   - **Modpack :** le même pack que celui de vos amis.
   - **From an instance** (depuis une instance) : les mods que vous utilisez déjà.
   - **From a file** (depuis un fichier).
3. Cliquez sur **Create** (créer). La fenêtre se ferme et le téléchargement continue en bas à droite.
4. Acceptez le CLUF (EULA) de Minecraft quand on vous le demande, puis cliquez sur **Start** (démarrer).

Sur la page d’un serveur :

- **Console :** ce que dit le serveur ; tapez des commandes en bas.
- **Players** (joueurs) : qui est en ligne en ce moment, plus la whitelist, les opérateurs et les bannissements.
- **Mods** ou **Plugins :** ajouter, mettre à jour ou désactiver. **Tout mettre à jour** en télécharge jusqu’à quatre à la fois ; si le serveur tourne, les mises à jour s’appliquent au prochain redémarrage.
- **Properties** (propriétés) : les réglages du serveur (difficulté, whitelist activée ou non, etc.).
- **Settings** (paramètres) :
  - mémoire, Java et **Java flags** (ceux de ResinCraft ou ceux du modpack) ;
  - comment vos amis se connectent ;
  - l’icône du serveur.
- **Redémarrer** et le menu **⋯** (copier l’adresse, ouvrir le dossier, sauvegarder maintenant, supprimer).

- **Settings → Anti-xray** (paramètres → anti-xray ; Paper, Purpur, Folia, Leaf) : cachez les minerais aux mods et packs de textures de rayons X. **Hide ores** (cacher les minerais) est léger pour le serveur ; **Fake ores** (faux minerais) est plus efficace. Le réglage s’applique au prochain démarrage.
- **Settings → Anti-cheat** (paramètres → anti-triche ; serveurs à plugins) : un seul interrupteur installe Grim.

Les mods qui ne tournent que sur le client du jeu (shaders, minicartes…) restent sur le serveur tant qu’il démarre correctement. Si l’un d’eux le fait planter, ResinCraft désactive ce mod et relance le serveur ; l’onglet **Mods** permet de le réactiver.

Quand un modpack n’a pas de server pack officiel, ResinCraft construit le serveur à partir de la liste de mods du pack : la fenêtre qui suit **Create** (créer) vous le signale, et le serveur affiche un badge **Built by ResinCraft** (construit par ResinCraft). Certains fichiers du pack ne peuvent être téléchargés que depuis le site de leur auteur. Ils sont listés en haut de l’onglet **Console**, avec **Get file** (obtenir le fichier). Téléchargez-les dans votre navigateur et ResinCraft les met en place avant le prochain démarrage, ou cliquez sur **Look again** (chercher à nouveau).

## 9. Laisser vos amis rejoindre votre serveur

*Depuis la version 0.2.0.*

Dans l’onglet **Settings** du serveur → **How friends connect** (comment vos amis se connectent) :

- **Automatic** (automatique, recommandé, et le choix par défaut des nouveaux serveurs) : d’abord votre box ou routeur, puis **ResinCraft Relay**, puis playit.gg.
- **ResinCraft Relay :** vos amis rejoignent `yourname.rscraft.resincore.tech`. Il faut un compte Microsoft connecté.
- **playit.gg :** une adresse gratuite qui ne change pas. Au premier démarrage, on vous demande de lier un compte playit.gg (un compte invité suffit).
- **Port redirigé moi-même :** si vous avez configuré votre box ou routeur vous-même.

L’onglet **Console** affiche l’adresse à donner à vos amis. Vos amis sur Bedrock peuvent aussi rejoindre les serveurs Java : activez **Bedrock players can join** (les joueurs Bedrock peuvent rejoindre).

**Votre propre domaine :** avec ResinCraft Relay, activez **Relier votre propre domaine à ResinCraft Relay** et tapez le domaine. Ajoutez les enregistrements DNS affichés, dont l’enregistrement TXT qui prouve que le domaine est à vous, puis cliquez sur **Vérifier**. Le badge passe de **Vérification en attente** à **Vérifié**. Avec le DNS automatique Cloudflare activé, ResinCraft ajoute les enregistrements pour vous.

## 10. Demandes de whitelist (page web et bot Discord)

*Depuis la version 1.2.7.*

Laissez les joueurs demander à rejoindre, et acceptez-les ou refusez-les vous-même.

1. Dans **Properties**, mettez **white-list** sur **true** (sinon tout le monde peut rejoindre de toute façon).
2. Ouvrez l’onglet **Players** du serveur et activez **Whitelist requests** (demandes de whitelist).
3. Choisissez comment les joueurs demandent :
   - **Web page** (page web) :
     - Fonctionne quand le serveur est partagé via ResinCraft Relay.
     - Cliquez sur **Copy link** (copier le lien) et envoyez le lien aux joueurs. C’est l’adresse propre de votre serveur, ou votre domaine vérifié.
     - La page fonctionne tant que le serveur est en ligne.
   - **Your Discord bot** (votre bot Discord) :
     1. Ouvrez le [Discord Developer Portal](https://discord.com/developers/applications) → **New Application** → **Bot** → **Reset Token**, et copiez le jeton.
     2. Collez-le dans ResinCraft et cliquez sur **Save** (enregistrer). Il est conservé en lieu sûr sur votre PC et n’est plus jamais affiché.
     3. Quand il indique **Online as …**, cliquez sur **Add the bot to your Discord** (ajouter le bot à votre Discord) et choisissez votre serveur Discord.
     4. Les joueurs tapent `/whitelist TheirName` sur votre Discord (avec leur propre pseudo Minecraft).
4. Les demandes apparaissent dans l’onglet **Players** :
   - **Accept** (accepter) : ajoute le joueur à la whitelist (le bot le prévient par message privé).
   - **Reject** (refuser) : refuse la demande.
   - **Reject and block** (refuser et bloquer) : la refuse et empêche cette personne de redemander. Les joueurs bloqués apparaissent dans la liste **Blocked** (bloqués), avec **Unblock** (débloquer) pour chacun.
   - **Depuis la liste des serveurs :** les joueurs qui ont ResinCraft peuvent demander depuis la page de votre serveur dans Découvrir → Serveurs (section 17).

Les demandes des joueurs bannis sont refusées automatiquement. Le bot Discord fonctionne tant que ResinCraft est ouvert ; activez la zone de notification (section 13) pour qu’il continue quand la fenêtre est fermée.

## 11. Bedrock Edition

*Depuis la version 0.3.1.*

Sous Windows, **Bedrock**, à gauche, installe, ouvre et désinstalle Minecraft pour Windows et Minecraft Preview. La désinstallation sauvegarde d’abord vos mondes.

## 12. Venir d’un autre launcher

*Depuis la version 1.0.5.*

**Bibliothèque** → **Venir d’un autre launcher** importe des instances depuis :

- Prism Launcher et MultiMC
- la Modrinth App et l’app CurseForge
- ATLauncher et GDLauncher
- le Minecraft Launcher officiel

Leurs mods, mondes et réglages viennent avec. Les originaux restent là où ils sont.

## 13. Mises à jour et zone de notification

*Depuis la version 0.3.0 (la zone de notification depuis la 1.2.7).*

- **Mises à jour :**
  - Quand une nouvelle version de ResinCraft est prête, un bouton **Mettre à jour** apparaît dans la barre de titre.
  - Si un jeu ou un serveur tourne, vous choisissez **Les fermer et mettre à jour** (les serveurs sauvegardent d’abord leurs mondes) ou **Mettre à jour quand ils s’arrêtent**.
- **Mettre à jour en silence** (Paramètres → Mises à jour) : installe les nouvelles versions en arrière-plan, sans demander, dès que plus rien ne tourne.
- **Rester dans la zone de notification** (Paramètres → Apparence) :
  - Fermer la fenêtre laisse ResinCraft tourner : les serveurs, le bot Discord et les téléchargements continuent.
  - Cliquez sur l’icône dans la zone de notification pour le rouvrir ; clic droit → **Quit ResinCraft** (quitter ResinCraft) pour le fermer pour de bon.

## 14. Thèmes et CSS personnalisé

*Depuis la version 1.3.0.*

1. Ouvrez **Paramètres → Apparence → Thème** et choisissez une carte : **Système**, Sombre, Clair, Obsidienne, Ardoise des abîmes, Cerisier, Nether ou L’End.
2. **CSS personnalisé :** cliquez sur **Insérer les couleurs actuelles**, modifiez les couleurs que vous voulez, puis cliquez sur **Appliquer**. Cela marche par-dessus n’importe quel thème ; **Effacer** le retire.
3. Ouvrez **Comment personnaliser** sous la zone : vous y trouverez les couleurs et formes modifiables, les parties de l’application que vous pouvez styliser, et des exemples (une image de fond, des coins plus arrondis, un texte plus grand…). Cliquez sur l’un d’eux pour l’ajouter à votre CSS.

Psst : essayez le code Konami (↑ ↑ ↓ ↓ ← → ← → B A), ou cliquez sept fois sur le logo ResinCraft.

## 15. Contrôle à distance (console web)

*Depuis la version 1.3.0.*

Pilotez vos serveurs depuis n’importe quel navigateur, ou depuis votre téléphone.

1. Connectez-vous avec un compte Microsoft dans **Comptes**.
2. Activez **Paramètres → Contrôle à distance → Autoriser le contrôle à distance**. Quand il indique **Connecté en tant que …**, c’est prêt.
3. Activez **Rester dans la zone de notification** (section 13), pour que cela continue de marcher après la fermeture de la fenêtre.
4. Ouvrez `https://console.rscraft.resincore.tech` (ou cliquez sur **Ouvrir la console web**) et connectez-vous avec **le même compte Minecraft**.

Depuis la console, vous pouvez faire tout ce que fait le gestionnaire de serveurs : démarrer, arrêter et redémarrer les serveurs, suivre la console et taper des commandes, gérer les joueurs et les demandes de whitelist, ajouter et mettre à jour des plugins et des mods, modifier les propriétés et les réglages, changer la version du modpack, réinstaller, sauvegarder et créer de nouveaux serveurs. Chaque modification s’affiche sous forme de notification dans ResinCraft.

Le gestionnaire de fichiers, les réglages Java et les plugins depuis n’importe quel lien de téléchargement peuvent lancer des programmes sur votre PC ; ils demandent donc un second interrupteur : **Autoriser le contrôle total**. Activez-le seulement si votre compte Minecraft (Microsoft) a la vérification en deux étapes. Sans lui, la console ne peut jamais accéder à quoi que ce soit d’autre sur votre PC.

## 16. Réinstaller et réparer

*Depuis la version 1.3.0.*

- **Une instance ne démarre pas parce que des fichiers manquent ou sont abîmés :**
  - Faites un clic droit dessus dans la **Bibliothèque** → **Réinstaller** → **Fichiers du jeu**, ou ouvrez ses **Paramètres → Réparer**.
  - Minecraft, le chargeur de mods, les bibliothèques et les ressources sont retéléchargés. Les mondes, mods et réglages restent.
- **Les fichiers d’un modpack ont été modifiés ou supprimés :** **Réinstaller** → **Fichiers du modpack** remet les mods, configurations et scripts du pack. Les fichiers que vous avez modifiés sont d’abord sauvegardés ; les mondes et vos propres mods restent.
- **Un serveur :** son menu **⋯** → **Réinstaller** → **Logiciel du serveur** (ou **Fichiers du modpack** pour un serveur de modpack). Le monde, les réglages et les listes de joueurs restent.
- **ResinCraft lui-même :** **Paramètres → Mises à jour → Réinstaller ResinCraft** réinstalle l’application par-dessus elle-même.

## 17. Trouver et rejoindre des serveurs

*Depuis la version 1.4.0.*

1. Ouvrez **Découvrir** → **Serveurs**. Vous y trouvez les serveurs hébergés avec ResinCraft et ceux ajoutés par des joueurs.
2. Cherchez, ou filtrez par catégorie (SMP, PvP, Mini-jeux, Skyblock…), région, langue, modpack, et selon que les joueurs Bedrock ou les profils hors ligne peuvent rejoindre.
3. Cliquez sur l’icône ou le nom d’un serveur pour ouvrir sa page : la description complète, tous les détails, son pack client et ses liens.
4. Cliquez sur **Rejoindre**. ResinCraft choisit l’instance qui convient au serveur, ou vous demande laquelle utiliser. Un serveur avec ses propres mods propose **Installer et rejoindre**, qui crée une instance avec exactement ces mods et le rejoint.
5. Serveur avec whitelist ? Cliquez sur **Demander à rejoindre**. Le bouton affiche **Waiting for the host** (en attente de l’hôte), puis **On the whitelist** (sur la whitelist ; cliquez sur **Rejoindre**) ou **Ask again** (redemander). ResinCraft vous prévient, et fait clignoter la barre des tâches, quand l’hôte répond.

Aimez les serveurs qui vous plaisent, et signalez ceux qui enfreignent les règles.

## 18. Référencer votre serveur et partager ses mods

*Depuis la version 1.4.0.*

1. Dans l’onglet **Settings** (paramètres) d’un serveur hébergé, activez **Liste des serveurs** (désactivé par défaut).
2. Ajoutez une description, des tags, une région et une langue. La prise en charge de Bedrock et des profils hors ligne, ainsi que le modpack, se remplissent tout seuls.
3. **Pack client :** publiez ce dont les joueurs ont besoin, pour qu’ils puissent utiliser **Installer et rejoindre** :
   - le modpack du serveur, installé depuis son propre site ;
   - chaque autre mod, depuis Modrinth ou CurseForge ;
   - vos propres fichiers, envoyés directement depuis votre PC tant que ResinCraft tourne ;
   - les packs de ressources et les shaders que vous choisissez d’envoyer avec.
   Les serveurs à plugins et vanilla peuvent aussi en avoir un : choisissez une instance avec la version de Minecraft du serveur.

Un serveur que vous faites tourner ailleurs ? Ajoutez-le avec **Proposer un serveur** dans Découvrir → Serveurs.

## 19. Vos autres PC et les amis qui aident

*Depuis la version 1.4.1.*

**Vos autres PC :** activez le contrôle à distance sur chaque PC (section 15) et connectez-vous avec le même compte Minecraft. Ensuite, dans **Servers**, ouvrez le menu des PC en haut et choisissez un PC. Vous pouvez démarrer, arrêter et gérer ses serveurs d’ici, et aussi y utiliser **Create server** (créer un serveur) ou **Importer un serveur**. Pour importer depuis un dossier de ce PC, tapez le dossier ; il faut pour cela **Autoriser le contrôle total** sur ce PC.

**Des amis qui vous aident à gérer vos serveurs :**

1. Ouvrez **Paramètres → Contrôle à distance → People who can use your servers** (personnes qui peuvent utiliser vos serveurs).
2. Tapez leur pseudo Minecraft et cliquez sur **Add** (ajouter).
3. Choisissez ce qu’ils peuvent faire :
   - **Viewer** (observateur) : voir la console, les joueurs et les réglages.
   - **Operator** (opérateur) : en plus, démarrer, arrêter et redémarrer, envoyer des commandes, et gérer les joueurs et les demandes de whitelist.
   - **Manager** (gestionnaire) : en plus, modifier les réglages, les plugins et les mods, et le modpack.
4. Gardez **All servers** (tous les serveurs), ou choisissez les serveurs sous **Which servers?** (quels serveurs ?).

Ils ouvrent vos serveurs depuis leur ResinCraft (le menu des PC dans **Servers**) ou depuis la console web, tant que le contrôle à distance est activé et que ResinCraft tourne sur votre PC. Les fichiers, les réglages Java, et la création ou la suppression de serveurs restent à vous.

## 20. Sauvegardes des mondes

*Depuis la version 1.4.0.*

- **Automatiques :** pendant qu’un jeu ou un serveur tourne, ses mondes sont sauvegardés toutes les 30 minutes, en gardant les 3 plus récentes par monde. Modifiez ou désactivez cela dans **Paramètres → Sauvegarde auto**.
- **Restaurer :** dans l’onglet **Mondes** d’une instance, ou dans **Settings → Backups** (paramètres → sauvegardes) pour un serveur hébergé, cliquez sur **Restaurer** sur une sauvegarde, la vôtre ou une automatique. Le monde remplacé part dans la Corbeille, donc une restauration peut être annulée. Un monde solo peut aussi revenir sous forme de copie.

## 21. Stockage et passage à un autre PC

*Depuis la version 1.1.0 (Passer à un autre PC depuis la 1.1.1).*

- **Un autre disque :** **Paramètres → Stockage → Modifier…**, choisissez un disque ou un dossier, puis :
  - **Déplacer les instances :** déplace vos instances, vos serveurs et les fichiers partagés de Minecraft là-bas.
  - **Utiliser sans déplacer :** les nouveautés vont là-bas ; vos instances actuelles restent et reviennent quand vous revenez en arrière.
- **Un nouveau PC :** **Paramètres → Passer à un autre PC** enregistre tout dans un seul fichier `.resincraft`. Ouvrez-le avec ResinCraft sur le nouveau PC.

## 22. Dépannage

- **Le jeu ne démarre pas :** regardez la fenêtre **Minecraft a planté** (section 6), ou l’onglet **Journaux** de l’instance. Des fichiers manquent ou sont abîmés ? Réinstallez-les (section 16).
- **Un mod n’apparaît pas dans Découvrir :** il n’existe peut-être pas pour votre version de Minecraft ou votre chargeur. Essayez une autre version, ou changez la version de l’instance (section 5).
- **Vos amis ne peuvent pas rejoindre :** vérifiez l’adresse dans l’onglet **Console** du serveur et que le serveur indique **Running** (en marche). Avec ResinCraft Relay, vérifiez que vous êtes connecté avec un compte Microsoft.
- **Le serveur plante au démarrage :** ResinCraft désactive tout seul les mods côté client et le relance. Cherchez dans la **Console** la ligne qui indique quel mod a été désactivé. Si elle indique que des fichiers manquent, récupérez-les depuis la liste en haut de l’onglet **Console**.
- **Toujours bloqué ?** Signalez le problème sur [GitHub](https://github.com/ResinCoreAI/ResinCraft-Launcher/issues) avec le journal de l’onglet **Journaux**.
