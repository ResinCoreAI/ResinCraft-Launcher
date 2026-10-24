# ResinCraft Launcher : fonctionnalités

Un launcher Minecraft moderne et convivial, créé par **Puirkle**, pour Windows, macOS et Linux. Aucune télémétrie et aucun compte ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Les éléments marqués **(nouveau)** sont nouveaux dans la dernière version.

## Jouer

- **Instances :** des installations séparées par version ou par modpack, chacune avec ses propres mods, mondes, Java, mémoire et arguments JVM.
- **Toutes les versions de Minecraft :** chaque version et chaque snapshot, jusqu’aux alphas classiques.
- **Chargeurs de mods :** Fabric, Quilt, Forge (jusqu’à Minecraft 1.2.5), NeoForge et LiteLoader, avec choix de la version du chargeur. OptiFine et les mods jar aussi.
- **Changer de version et de chargeur :** changez la version de Minecraft ou le chargeur de mods d’une instance depuis son onglet Paramètres, comme dans Prism. La pastille de version sur la page de l’instance y mène directement.
- **Jouer :** un clic. **Jouer cette fois en tant que** pour utiliser un autre compte, ou **Jouer aussi en tant que** pour lancer la même instance deux fois avec deux comptes.
- **Connexion rapide :** vos serveurs et mondes solo récents ; un clic et vous y êtes.
- **Mondes :** jouez directement à un monde, renommez-le, sauvegardez-le, importez un monde depuis un zip.
- **Réparation des plantages :** quand Minecraft plante, ResinCraft lit le rapport de plantage et les journaux. Il vous explique simplement ce qui s’est passé et propose une correction en un clic. Corrections : installer un mod manquant, désactiver un mod qui a planté, choisir le bon Java, donner plus de mémoire au jeu.
- **Réinstaller (nouveau) :** retéléchargez les fichiers du jeu d’une instance, ou remettez les fichiers d’un modpack tels que le pack les a. Depuis le menu de l’instance ou **Paramètres → Réparer**. Les mondes restent.
- **Flags Java optimisés :** les jeux démarrent avec les flags G1GC de ResinCraft pour des images plus fluides (Paramètres → Java, ou par instance).
- **Java :** trouve les Java installés, choisit le bon pour chaque version de Minecraft et télécharge Eclipse Temurin si aucun ne convient.
- **Affichage de la RAM :** la mémoire utilisée par le launcher, chaque jeu et chaque serveur (aussi sur l’Accueil, si vous voulez).
- **Discord Rich Presence :** nom du pack, version, temps de jeu, solo ou serveur. Des boutons pour vos amis : **View modpack** (voir le modpack) et **Get ResinCraft** (obtenir ResinCraft).

## Instances

- **Organisation :** groupes, votre propre ordre (glisser pour ranger), icônes, bannières et couleurs.
- **Onglets :** Contenu, Mondes, Serveurs, Captures, Notes, Fichiers (gestionnaire et éditeur de fichiers intégré), Journaux et Paramètres.
- **Copier depuis une autre instance :** mondes, mods et leurs réglages (dossier config), shaders, packs de ressources et réglages du jeu.
- **Dupliquer, exporter** (Modrinth `.mrpack` ou CurseForge `.zip`) et **en faire votre propre modpack**.
- **Commandes et environnement :** des commandes à lancer avant le démarrage et après la fermeture du jeu, et des variables d’environnement.
- **Interrupteurs instantanés :** activer ou désactiver un mod est instantané ; le renommage se fait en arrière-plan.
- **Suppression et création instantanées (nouveau) :** une instance supprimée disparaît aussitôt et part dans la Corbeille en arrière-plan. Un modpack en cours d’installation apparaît tout de suite dans la Bibliothèque, avec sa progression sur sa carte.

## Contenu et Découvrir

- **Sites :** mods, modpacks, packs de ressources et shaders de Modrinth, CurseForge, FTB, ATLauncher, Technic et des releases GitHub, filtrés selon la version et le chargeur de l’instance.
- **Pages de modpack** avec description, galerie, versions, journaux des modifications et **les mods que contient le pack**.
- **Installations malignes :** choisissez n’importe quelle version, installez-en plusieurs à la fois, les dépendances nécessaires sont ajoutées automatiquement, et le même mod venant de deux sites est repéré.
- **Mises à jour :** recherchez et appliquez les mises à jour (une ou toutes) ; mises à jour de modpacks et changement de version.
- **Téléchargements manuels :** les fichiers CurseForge dont les auteurs bloquent les launchers sont récupérés dans votre dossier Téléchargements ou dans d’autres **Dossiers surveillés** (Paramètres → Téléchargements).
- **Listes :** exportez les mods, packs de ressources ou shaders sous forme de liste, et installez une liste dans une autre instance.

## Héberger des serveurs

- **Types de serveur :** Vanilla, Paper, Purpur, Folia, Spigot, Leaf, Fabric, Quilt, Forge, NeoForge, Arclight (mods et plugins ensemble) et Bedrock Dedicated Server.
- **Depuis un modpack ou une instance :** créez un serveur depuis Modrinth, CurseForge (le server pack officiel s’il existe) ou FTB, ou depuis l’une de vos instances.
- **Pas de server pack officiel ? (nouveau) :** ResinCraft construit le serveur à partir de la liste de mods du pack, vous le signale à la création du serveur et affiche un badge **Built by ResinCraft** (construit par ResinCraft).
- **Fichiers à télécharger vous-même (nouveau) :** les fichiers dont les auteurs n’autorisent le téléchargement que depuis leur site sont listés dans l’onglet **Console** du serveur. Avant chaque démarrage, ResinCraft les cherche dans votre dossier Téléchargements et dans vos instances du même pack, et les met en place.
- **Mémoire selon la taille du pack (nouveau) :** un nouveau serveur moddé reçoit 6 Go à partir de 80 mods et 8 Go à partir de 150 (au plus la moitié de la RAM de votre PC).
- **Mods côté client désactivés seulement s’ils plantent :** les mods client restent sur le serveur tant qu’il démarre correctement. Si un démarrage plante parce qu’un mod a voulu accéder au client du jeu, ce mod est désactivé (`.jar.disabled`) et le serveur redémarre tout seul. Un mod dont un autre mod a besoin reste actif.
- **Gestion :** console, éditeur de propriétés, gestionnaire de plugins et de mods avec mises à jour, whitelist, ops et bannissements, icône du serveur, sauvegardes.
- **Redémarrer et menu « plus » :** Redémarrer à côté de Stop ; un menu avec copier l’adresse, ouvrir le dossier, sauvegarder, paramètres et supprimer.
- **Online now** (en ligne maintenant) : les joueurs sur votre serveur, vérifiés en direct toutes les quelques secondes.
- **Java flags** (flags Java) : les flags optimisés de ResinCraft, ou ceux du modpack (`user_jvm_args.txt`, scripts de démarrage).
- **Anti-xray (nouveau) :** les serveurs Paper, Purpur, Folia et Leaf peuvent cacher les minerais aux mods et packs de textures de rayons X (Settings → Anti-xray).
- **Réinstaller (nouveau) :** réinstallez le logiciel d’un serveur, ou remettez les fichiers de son modpack. Le monde et les réglages restent.
- **Suppression et création instantanées (nouveau) :** un serveur supprimé disparaît aussitôt ; un serveur en cours de création apparaît tout de suite dans la liste, avec sa progression.
- **Contrôle à distance (nouveau) :** pilotez vos serveurs depuis n’importe quel navigateur ou votre téléphone avec la console web sur `console.rscraft.resincore.tech`, connecté avec le même compte Minecraft, tant que ResinCraft est ouvert ou dans la zone de notification (Paramètres → Contrôle à distance). Elle peut démarrer, arrêter, surveiller et créer des serveurs, mais n’accède jamais à vos fichiers ni à vos autres réglages.
- **Partager avec vos amis :**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - votre box ou routeur (UPnP)
  - votre propre domaine, avec DNS Cloudflare automatisé
- **Bedrock players can join** (les joueurs Bedrock peuvent rejoindre) : les joueurs Bedrock peuvent aussi rejoindre les serveurs Java (Geyser + Floodgate).
- **Whitelist requests** (demandes de whitelist) : les joueurs demandent à rejoindre, et vous les acceptez ou les refusez dans l’onglet Players.
  - **Web page** (page web) : `https://rscraft.resincore.tech/join/<name>` (avec ResinCraft Relay).
  - **Your Discord bot** (votre propre bot Discord) : les joueurs tapent `/whitelist`.
  - **Votre réponse :** Accept (accepter) les ajoute à la whitelist ; le bot les prévient par message privé.
  - **Protection :** les joueurs bannis sont refusés ; **Reject and block** (refuser et bloquer) empêche quelqu’un de redemander.

## Bedrock Edition (Windows)

- Installez, lancez et désinstallez Minecraft pour Windows et Minecraft Preview (les mondes sont sauvegardés avant).

## Comptes

- **Connexion Microsoft**, avec des jetons stockés chiffrés. **Profils hors ligne** pour le solo et le LAN.
- **Skins, capes et changement de pseudo.**

## Venir d’un autre launcher

- Importez des instances depuis Prism Launcher, MultiMC, la Modrinth App, l’app CurseForge, ATLauncher, GDLauncher et le Minecraft Launcher officiel, avec leurs mods, mondes, packs et réglages. Les serveurs peuvent aussi être importés.

## L’application

- **Windows, macOS** (Apple Silicon et Intel) **et Linux** (x64 et ARM64 : AppImage, `.deb`, `.tar.gz`).
- **Installateurs Windows :** le `.exe`, un `.msi` pour les installations gérées, et **ResinCraft Setup (nouveau)** : un petit installateur qui installe toujours la dernière version, via un lien qui ne change jamais.
- **Mises à jour :** recherchées automatiquement.
  - Un bouton **Mettre à jour** dans la barre de titre.
  - **Les fermer et mettre à jour**, quand des jeux ou des serveurs tournent.
  - **Mettre à jour en silence** en arrière-plan.
- **Rester dans la zone de notification :** fermer la fenêtre laisse tourner les serveurs, le bot Discord, le contrôle à distance et les téléchargements.
- **Réinstaller ResinCraft (nouveau) :** Paramètres → Mises à jour réinstalle l’application par-dessus elle-même, pour la réparer. Les instances, comptes et réglages restent.
- **Thèmes (nouveau) :** Sombre, Clair, Obsidienne, Ardoise des abîmes, Cerisier, Nether, L’End, Halloween et Noël, ou Système.
  - Halloween s’active tout seul du 16 au 31 octobre, Noël du 10 au 25 décembre. Choisissez un autre thème et il reste le vôtre ; ou désactivez les thèmes de fêtes.
  - **CSS personnalisé :** votre propre CSS par-dessus n’importe quel thème, en partant des couleurs actuelles.
  - Et une surprise cachée. Essayez le code Konami.
- **16 langues :** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Stockage** (déplacer les données vers un autre disque), **sauvegarde et restauration** des paramètres.
