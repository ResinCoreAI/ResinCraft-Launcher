# ResinCraft Launcher : fonctionnalités

Un launcher Minecraft moderne et convivial, créé par **Puirkle**, pour Windows, macOS et Linux. Aucune télémétrie et aucun compte ResinCraft.

> NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.

Les éléments marqués **(1.2.7)** sont nouveaux dans la version 1.2.7.

## Jouer

- **Instances :** des installations séparées par version ou par modpack, chacune avec ses propres mods, mondes, Java, mémoire et arguments JVM.
- **Toutes les versions de Minecraft :** chaque version et chaque snapshot, jusqu’aux alphas classiques.
- **Chargeurs de mods :** Fabric, Quilt, Forge (jusqu’à Minecraft 1.2.5), NeoForge et LiteLoader, avec choix de la version du chargeur. OptiFine et les mods jar aussi.
- **Changer de version et de chargeur (1.2.7) :** changez la version de Minecraft ou le chargeur de mods d’une instance depuis son onglet Paramètres, comme dans Prism. La pastille de version sur la page de l’instance y mène directement.
- **Jouer :** un clic. **Jouer cette fois en tant que** pour utiliser un autre compte, ou **Jouer aussi en tant que** pour lancer la même instance deux fois avec deux comptes.
- **Connexion rapide :** vos serveurs et mondes solo récents ; un clic et vous y êtes.
- **Mondes :** jouez directement à un monde, renommez-le, sauvegardez-le, importez un monde depuis un zip.
- **Réparation des plantages (1.2.7) :** quand Minecraft plante, ResinCraft lit le rapport de plantage et les journaux. Il vous explique simplement ce qui s’est passé et propose une correction en un clic. Corrections : installer un mod manquant, désactiver un mod qui a planté, choisir le bon Java, donner plus de mémoire au jeu.
- **Java :** trouve les Java installés, choisit le bon pour chaque version de Minecraft et télécharge Eclipse Temurin si aucun ne convient.
- **Affichage de la RAM :** la mémoire utilisée par le launcher, chaque jeu et chaque serveur (aussi sur l’Accueil, si vous voulez).
- **Discord Rich Presence :** nom du pack, version, temps de jeu, solo ou serveur. Des boutons pour vos amis : **View modpack** (voir le modpack) et **Get ResinCraft** (obtenir ResinCraft).

## Instances

- **Organisation :** groupes, votre propre ordre (glisser pour ranger), icônes, bannières et couleurs.
- **Onglets :** Contenu, Mondes, Serveurs, Captures, Notes, Fichiers (gestionnaire et éditeur de fichiers intégré), Journaux et Paramètres.
- **Copier depuis une autre instance :** mondes, mods et leurs réglages (dossier config), shaders, packs de ressources et réglages du jeu.
- **Dupliquer, exporter** (Modrinth `.mrpack` ou CurseForge `.zip`) et **en faire votre propre modpack**.
- **Commandes et environnement :** des commandes à lancer avant le démarrage et après la fermeture du jeu, et des variables d’environnement.
- **Interrupteurs instantanés (1.2.7) :** activer ou désactiver un mod est instantané ; le renommage se fait en arrière-plan.

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
- **Mods côté client écartés :** les mods qui ne tournent que sur le client du jeu sont repérés et désactivés automatiquement, à chaque démarrage du serveur. Un mod dont un autre mod a besoin reste actif. Si l’un d’eux fait quand même planter le serveur, il est désactivé et le serveur redémarre tout seul.
- **Gestion :** console, éditeur de propriétés, gestionnaire de plugins et de mods avec mises à jour, whitelist, ops et bannissements, icône du serveur, sauvegardes.
- **Redémarrer et menu « plus » (1.2.7) :** Redémarrer à côté de Stop ; un menu avec copier l’adresse, ouvrir le dossier, sauvegarder, paramètres et supprimer.
- **Online now** (en ligne maintenant) : les joueurs sur votre serveur, vérifiés en direct toutes les quelques secondes **(1.2.7)**.
- **Java flags (1.2.7)** (flags Java) : les flags optimisés de ResinCraft, ou ceux du modpack (`user_jvm_args.txt`, scripts de démarrage).
- **Partager avec vos amis :**
  - ResinCraft Relay (`<name>.rscraft.resincore.tech`)
  - playit.gg
  - bore
  - votre box ou routeur (UPnP)
  - votre propre domaine, avec DNS Cloudflare automatisé
- **Bedrock players can join** (les joueurs Bedrock peuvent rejoindre) : les joueurs Bedrock peuvent aussi rejoindre les serveurs Java (Geyser + Floodgate).
- **Whitelist requests (1.2.7)** (demandes de whitelist) : les joueurs demandent à rejoindre, et vous les acceptez ou les refusez dans l’onglet Players.
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
- **Mises à jour :** recherchées automatiquement.
  - **(1.2.7)** Un bouton **Mettre à jour** dans la barre de titre.
  - **(1.2.7)** **Les fermer et mettre à jour**, quand des jeux ou des serveurs tournent.
  - **(1.2.7)** **Mettre à jour en silence** en arrière-plan.
- **Rester dans la zone de notification (1.2.7) :** fermer la fenêtre laisse tourner les serveurs, le bot Discord et les téléchargements.
- **16 langues :** English, Deutsch, Español, Français, Bahasa Indonesia, Italiano, 日本語, 한국어, Polski, Português (Brasil), Русский, ไทย, Türkçe, Tiếng Việt, 简体中文, 繁體中文.
- **Stockage** (déplacer les données vers un autre disque), **sauvegarde et restauration** des paramètres, thèmes **clair et sombre**.
