# ResinCraft Launcher : fonctionnalités

Un launcher Minecraft moderne et convivial, créé par **Puirkle**, pour Windows, macOS et Linux. Aucune télémétrie et aucun compte ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

La version entre parenthèses est celle où chaque fonctionnalité est arrivée. Les plus récentes sont 1.4.0 et 1.4.1.

## Jouer

- **Instances (0.1.0) :** des installations séparées par version ou par modpack, chacune avec ses propres mods, mondes, Java, mémoire et arguments JVM.
- **Toutes les versions de Minecraft (0.1.0) :** chaque version et chaque snapshot, jusqu’aux alphas classiques.
- **Chargeurs de mods (0.1.0) :** Fabric, Quilt, Forge et NeoForge, avec choix de la version du chargeur. Forge classique pour 1.7.10 à 1.12.2 (1.1.2), Forge jusqu’à Minecraft 1.2.5 et LiteLoader (1.2.0). OptiFine (1.0.1) et les mods jar (1.2.0) aussi.
- **Changer de version et de chargeur (1.2.7) :** changez la version de Minecraft ou le chargeur de mods d’une instance depuis son onglet Paramètres, comme dans Prism. La pastille de version sur la page de l’instance y mène directement.
- **Jouer (0.1.0) :** un clic. **Jouer cette fois en tant que** pour utiliser un autre compte, ou **Jouer aussi en tant que** pour lancer la même instance deux fois avec deux comptes (1.0.5).
- **Connexion rapide (1.1.0) :** vos serveurs récents, et vos mondes solo (1.2.3) ; un clic et vous y êtes.
- **Mondes (0.1.0) :** renommez un monde, sauvegardez-le, importez un monde depuis un zip. Jouez directement à un monde (1.2.0).
- **Sauvegardes automatiques des mondes (1.4.0) :** pendant qu’un jeu ou un serveur tourne, ses mondes sont sauvegardés toutes les 30 minutes, en gardant les 3 plus récentes par monde (Paramètres → Sauvegarde auto).
- **Restaurer une sauvegarde (1.4.0) :** remettez un monde depuis n’importe quelle sauvegarde, la vôtre ou une automatique, ou récupérez-le sous forme de copie. Le monde remplacé part dans la Corbeille.
- **Réparation des plantages (1.2.7) :** quand Minecraft plante, ResinCraft lit le rapport de plantage et les journaux. Il vous explique simplement ce qui s’est passé et propose une correction en un clic. Corrections : installer un mod manquant, désactiver un mod qui a planté, choisir le bon Java, donner plus de mémoire au jeu.
- **Réinstaller (1.3.0) :** retéléchargez les fichiers du jeu d’une instance, ou remettez les fichiers d’un modpack tels que le pack les a. Depuis le menu de l’instance ou **Paramètres → Réparer**. Les mondes restent.
- **Flags Java optimisés (1.2.7) :** les jeux démarrent avec les flags G1GC de ResinCraft pour des images plus fluides (Paramètres → Java, ou par instance).
- **Java (0.1.0) :** trouve les Java installés, choisit le bon pour chaque version de Minecraft et télécharge Eclipse Temurin si aucun ne convient.
- **Affichage de la RAM (1.2.1) :** la mémoire utilisée par le launcher, chaque jeu et chaque serveur (aussi sur l’Accueil, si vous voulez).
- **Discord Rich Presence (1.2.3) :** nom du pack, version, temps de jeu, solo ou serveur. Des boutons pour vos amis (1.2.6) : **View modpack** (voir le modpack) et **Get ResinCraft** (obtenir ResinCraft).

## Instances

- **Organisation :** groupes (0.2.0), votre propre ordre (0.3.1, glisser pour ranger), icônes et bannières (0.3.0), et couleurs (0.1.0).
- **Onglets (0.1.0) :** Contenu, Mondes, Serveurs, Captures, Notes, Fichiers (gestionnaire et éditeur de fichiers intégré, 0.2.0), Journaux et Paramètres.
- **Copier depuis une autre instance (1.2.5) :** mondes, shaders, packs de ressources et réglages du jeu ; les mods et leurs réglages (dossier config) depuis 1.2.6.
- **Dupliquer (0.1.0), exporter** (Modrinth `.mrpack` ou CurseForge `.zip`, 1.0.5) et **en faire votre propre modpack (1.1.0)**.
- **Sélection multiple (1.1.0) :** déplacez, supprimez, dupliquez ou mettez à jour plusieurs instances à la fois.
- **Commandes et environnement (1.2.0) :** des commandes à lancer avant le démarrage et après la fermeture du jeu, et des variables d’environnement.
- **Interrupteurs instantanés (1.2.7) :** activer ou désactiver un mod est instantané ; le renommage se fait en arrière-plan.
- **Suppression et création instantanées (1.3.0) :** une instance supprimée disparaît aussitôt et part dans la Corbeille en arrière-plan. Un modpack en cours d’installation apparaît tout de suite dans la Bibliothèque, avec sa progression sur sa carte.

## Contenu et Découvrir

- **Sites (0.1.0) :** mods, modpacks, packs de ressources et shaders de Modrinth, CurseForge, FTB, ATLauncher, Technic et des releases GitHub (0.3.1), filtrés selon la version et le chargeur de l’instance.
- **Pages de modpack (1.2.0)** avec description, galerie, versions, journaux des modifications et **les mods que contient le pack (1.2.5)**.
- **Installations malignes (0.3.0) :** choisissez n’importe quelle version, installez-en plusieurs à la fois, les dépendances nécessaires sont ajoutées automatiquement, et le même mod venant de deux sites est repéré.
- **Mises à jour (0.1.0) :** recherchez et appliquez les mises à jour (une ou toutes) ; mises à jour de modpacks et changement de version (1.1.0). **Tout mettre à jour** télécharge jusqu’à quatre mises à jour à la fois (1.4.1).
- **Téléchargements manuels (1.0.1) :** les fichiers CurseForge dont les auteurs bloquent les launchers sont récupérés dans votre dossier Téléchargements ou dans d’autres **Dossiers surveillés** (1.0.5, Paramètres → Téléchargements).
- **Listes (0.4.0) :** exportez les mods, packs de ressources ou shaders sous forme de liste, et installez une liste dans une autre instance.
- **Trouver des serveurs (1.4.0) :** Découvrir → **Serveurs** liste les serveurs hébergés avec ResinCraft et ceux ajoutés par des joueurs. Recherchez et filtrez par catégorie, région, langue, modpack, prise en charge de Bedrock ou des profils hors ligne ; aimez et signalez.
- **Une page pour chaque serveur (1.4.1) :** la description complète et le MOTD, tous les détails, son pack client, des liens et un champ pour demander la whitelist.
- **Rejoindre en un clic (1.4.0) :** **Rejoindre** choisit l’instance qui convient, ou vous demande. **Installer et rejoindre** crée une instance avec exactement les mods du serveur et le rejoint.
- **Demander la whitelist (1.4.0) :** **Demander à rejoindre** envoie une demande signée avec votre compte Minecraft. Le bouton affiche **Waiting for the host** (en attente de l’hôte), **On the whitelist** (sur la whitelist) ou **Ask again** (redemander), et ResinCraft vous prévient (et fait clignoter la barre des tâches) quand l’hôte répond (1.4.1).

## Héberger des serveurs

- **Types de serveur (0.1.0) :** Vanilla, Paper, Purpur, Folia, Spigot, Fabric, Quilt, Forge et NeoForge ; Leaf, Arclight (mods et plugins ensemble) et Bedrock Dedicated Server depuis 0.3.1.
- **Depuis un modpack ou une instance (0.1.0) :** créez un serveur depuis Modrinth, CurseForge (le server pack officiel s’il existe) ou FTB, ou depuis l’une de vos instances.
- **Pas de server pack officiel ? (1.2.11) :** ResinCraft construit le serveur à partir de la liste de mods du pack, vous le signale à la création du serveur et affiche un badge **Built by ResinCraft** (construit par ResinCraft).
- **Fichiers à télécharger vous-même (1.2.9) :** les fichiers dont les auteurs n’autorisent le téléchargement que depuis leur site sont listés dans l’onglet **Console** du serveur. Avant chaque démarrage, ResinCraft les cherche dans votre dossier Téléchargements et dans vos instances du même pack, et les met en place.
- **Mémoire selon la taille du pack (1.2.9) :** un nouveau serveur moddé reçoit 6 Go à partir de 80 mods et 8 Go à partir de 150 (au plus la moitié de la RAM de votre PC).
- **Mods côté client désactivés seulement s’ils plantent (1.2.8) :** les mods client restent sur le serveur tant qu’il démarre correctement. Si un démarrage plante parce qu’un mod a voulu accéder au client du jeu, ce mod est désactivé (`.jar.disabled`) et le serveur redémarre tout seul. Un mod dont un autre mod a besoin reste actif.
- **Gestion (0.1.0) :** console, éditeur de propriétés, gestionnaire de plugins et de mods avec mises à jour (0.3.0), whitelist, ops et bannissements, icône du serveur (1.2.5), sauvegardes.
- **Mises à jour plus rapides (1.4.1) :** **Tout mettre à jour** télécharge jusqu’à quatre plugins ou mods à la fois et les installe ensemble, ou les met en attente pour le prochain redémarrage si le serveur tourne.
- **Référencer votre serveur (1.4.0) :** Settings → **Liste des serveurs** l’ajoute à Découvrir → Serveurs avec une description, des tags, une région et une langue. Les serveurs que vous faites tourner ailleurs peuvent être ajoutés avec **Proposer un serveur**.
- **Packs client (1.4.0) :** publiez ce dont les joueurs ont besoin pour rejoindre. Depuis 1.4.1 : le modpack du serveur (installé depuis son propre site), chaque autre mod depuis Modrinth ou CurseForge, et vos propres fichiers directement depuis votre PC ; les packs de ressources et les shaders avec ; les serveurs à plugins et vanilla aussi.
- **Redémarrer et menu « plus » (1.2.7) :** Redémarrer à côté de Stop ; un menu avec copier l’adresse, ouvrir le dossier, sauvegarder, paramètres et supprimer.
- **Online now** (en ligne maintenant, 1.2.7) : les joueurs sur votre serveur, vérifiés en direct toutes les quelques secondes.
- **Java flags** (flags Java, 0.2.0) : les flags optimisés de ResinCraft, ou ceux du modpack (`user_jvm_args.txt`, scripts de démarrage, 1.2.7).
- **Anti-xray (1.3.0) :** les serveurs Paper, Purpur, Folia et Leaf peuvent cacher les minerais aux mods et packs de textures de rayons X (Settings → Anti-xray).
- **Anti-cheat (1.4.0) :** un seul interrupteur installe l’anti-triche Grim sur les serveurs à plugins.
- **Régénérer le monde (1.4.0) :** recommencez un serveur avec un nouveau monde ; l’ancien part dans la Corbeille.
- **Réinstaller (1.3.0) :** réinstallez le logiciel d’un serveur, ou remettez les fichiers de son modpack. Le monde et les réglages restent.
- **Suppression et création instantanées (1.3.0) :** un serveur supprimé disparaît aussitôt ; un serveur en cours de création apparaît tout de suite dans la liste, avec sa progression.
- **Contrôle à distance (1.3.0) :** pilotez vos serveurs depuis n’importe quel navigateur ou votre téléphone avec la console web sur `console.rscraft.resincore.tech`, connecté avec le même compte Minecraft, tant que ResinCraft est ouvert ou dans la zone de notification (Paramètres → Contrôle à distance). Elle peut faire tout ce que fait le gestionnaire de serveurs. Le gestionnaire de fichiers, les réglages Java et les plugins depuis n’importe quel lien demandent un second interrupteur, **Autoriser le contrôle total**. Un bouton **Console web** sur la page Servers (1.3.1).
- **Vos autres PC (1.4.1) :** le menu des PC de l’onglet Servers ouvre les serveurs de vos autres PC où le contrôle à distance est activé : démarrer, arrêter, gérer les joueurs, les plugins et les réglages, et y créer ou importer des serveurs.
- **Laisser vos amis aider (1.4.1) :** Paramètres → Contrôle à distance → **People who can use your servers** (personnes qui peuvent utiliser vos serveurs) : donnez à un compte Minecraft un accès **Viewer** (observateur), **Operator** (opérateur) ou **Manager** (gestionnaire), sur tous vos serveurs ou ceux que vous choisissez. Les fichiers, les réglages Java, et la création ou la suppression de serveurs restent à vous.
- **Partager avec vos amis :**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`, 0.3.0)
  - playit.gg (0.2.0)
  - bore (0.2.0)
  - votre box ou routeur (UPnP, 0.2.0)
  - votre propre domaine, avec DNS Cloudflare automatisé (0.2.0), vérifié par un enregistrement TXT (**Vérification en attente**, **Vérifié**, 1.3.1)
  - **Automatic** (automatique) pour les nouveaux serveurs (1.3.2) : d’abord votre routeur, puis ResinCraft Relay, puis playit.gg
- **Bedrock players can join** (les joueurs Bedrock peuvent rejoindre, 0.3.1) : les joueurs Bedrock peuvent aussi rejoindre les serveurs Java (Geyser + Floodgate).
- **Whitelist requests** (demandes de whitelist, 1.2.7) : les joueurs demandent à rejoindre, et vous les acceptez ou les refusez dans l’onglet Players.
  - **Web page** (page web) : à l’adresse propre de votre serveur ou sur votre domaine vérifié (1.3.2, avec ResinCraft Relay).
  - **Your Discord bot** (votre propre bot Discord) : les joueurs tapent `/whitelist`.
  - **La liste des serveurs (1.4.0) :** les joueurs qui ont ResinCraft demandent depuis Découvrir → Serveurs.
  - **Votre réponse :** Accept (accepter) les ajoute à la whitelist ; le bot les prévient par message privé.
  - **Protection :** les joueurs bannis sont refusés ; **Reject and block** (refuser et bloquer) empêche quelqu’un de redemander, avec une liste **Blocked** (bloqués) et **Unblock** (débloquer) pour chaque joueur (1.4.1).

## Bedrock Edition (Windows)

- **Page Bedrock (0.3.1) :** installez (1.0.0), mettez à jour et réparez (1.2.0), lancez et désinstallez (1.2.3) Minecraft pour Windows et Minecraft Preview (les mondes sont sauvegardés avant).

## Comptes

- **Connexion Microsoft (0.1.0)**, avec des jetons stockés chiffrés. **Profils hors ligne (0.1.0)** pour le solo et le LAN. **Profils Bedrock (1.0.0)** pour les comptes Microsoft sans Java Edition.
- **Skins, capes et changement de pseudo (0.1.0).** Une bibliothèque de skins, des skins à partir d’un nom de joueur et une recherche en ligne (1.0.3) ; un visualiseur de skin en 3D (1.1.0).

## Venir d’un autre launcher

- **Importer (1.0.5) :** des instances depuis Prism Launcher, MultiMC, la Modrinth App, l’app CurseForge, ATLauncher, GDLauncher et le Minecraft Launcher officiel, avec leurs mods, mondes, packs et réglages. Les serveurs peuvent aussi être importés.
- **Passer à un autre PC (1.1.1) :** Paramètres → Passer à un autre PC enregistre tout dans un seul fichier `.resincraft` ; ouvrez-le sur le nouveau PC.

## L’application

- **Windows (0.1.0), macOS** (Apple Silicon et Intel) **et Linux** (x64 et ARM64 : AppImage, `.deb`, 1.2.0 ; `.tar.gz`, 1.2.4).
- **Installateurs Windows :** le `.exe` (0.1.0), un `.msi` pour les installations gérées (1.2.10), et **ResinCraft Setup (1.3.0)** : un petit installateur qui installe toujours la dernière version, via un lien qui ne change jamais.
- **Mises à jour (0.3.0) :** recherchées automatiquement.
  - Un bouton **Mettre à jour** dans la barre de titre (1.2.7).
  - **Les fermer et mettre à jour**, quand des jeux ou des serveurs tournent (1.2.7).
  - **Mettre à jour en silence** en arrière-plan (1.2.7).
- **Rester dans la zone de notification (1.2.7) :** fermer la fenêtre laisse tourner les serveurs, le bot Discord, le contrôle à distance et les téléchargements.
- **Réinstaller ResinCraft (1.3.0) :** Paramètres → Mises à jour réinstalle l’application par-dessus elle-même, pour la réparer. Les instances, comptes et réglages restent.
- **Thèmes :** Sombre, Clair et Système (0.1.0) ; Obsidienne, Ardoise des abîmes, Cerisier, Nether et L’End (1.3.0).
  - **Créer un thème (1.3.0) :** votre propre thème, avec des **Réglages fins** pour chaque couleur (sélecteurs de couleur) et du CSS pour des parties de l’application (1.3.2).
  - **Partager des thèmes (1.3.2) :** **Exporter** un fichier `.rctheme`, avec son image et son CSS ; **Importer un thème**, ou double-cliquez sur le fichier (1.4.0).
  - **CSS personnalisé (1.3.0) :** votre propre CSS par-dessus n’importe quel thème, en partant des couleurs actuelles, avec un guide intégré des couleurs, des parties de l’application et des exemples prêts à l’emploi.
  - Et une surprise cachée (1.3.0). Essayez le code Konami.
- **16 langues (0.1.0) :** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Gestionnaire de stockage (1.1.0) :** gardez les instances sur un autre disque. **Déplacer les instances** là-bas, ou **Utiliser sans déplacer** (1.4.1).
- **Réinitialiser les paramètres** et **Désinstaller ResinCraft (1.4.0)** dans les Paramètres.
