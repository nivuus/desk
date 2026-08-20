# L'image du service `plateforme`, employée par le profil `deploiement` de
# `docker-compose.plateforme.yml`.
#
# ⚠️ CE FICHIER N'EST PAS PRESCRIT PAR LE PLAN DE P5, et c'est déclaré plutôt
# que passé sous silence : la « Structure des fichiers » du plan liste
# `deploiement/nginx.conf` et `deploiement/README.md`, mais aucune image pour le
# service `plateforme` qu'elle demande pourtant à la tâche 13 de déclarer. Sans
# elle, le profil `deploiement` se décrit mais ne se lance pas. C'est un manque
# du plan, comblé au plus petit.
#
# 🔴 LA VERSION MAJEURE SUIT `package.json`, qui déclare
# `"node": ">=24.0.0 <25.0.0"`. Une image d'une autre majeure démarrerait
# peut-être, et divergerait du moteur sur lequel les 423 tests sont verts.
# 🔴 LE CONTEXTE DE CONSTRUCTION EST LA RACINE DU DÉPÔT, ET NON `plateforme/`.
# CE N'EST PAS UNE COMMODITÉ, C'EST UNE NÉCESSITÉ MESURÉE.
#
# Trois fichiers de PRODUCTION importent `../../../proto/ts/plateforme` —
# `agents/canal.ts`, `agents/registre.ts`, `apps/catalogue.ts` —, c'est-à-dire
# un chemin qui SORT de `plateforme/`. Avec un contexte borné à `plateforme/`,
# ce répertoire ne peut pas entrer dans l'image, et le service MEURT au
# chargement sur `ERR_MODULE_NOT_FOUND … file:///opt/proto/ts/plateforme`.
#
# ⚠️ RIEN NE LE VOYAIT AVANT LE CRITÈRE ⑤ DE LA RECETTE : les 433 tests
# tournent depuis l'arbre, où le chemin relatif existe ; `docker build`
# RÉUSSIT, puisque rien n'est résolu à la construction ; `docker compose
# config` et `nginx -t` n'ont pas d'opinion. **L'image se construisait sans
# erreur et ne démarrait pas.** Trouvé le 20 août 2026 en lançant le profil
# `deploiement` pour la première fois.
#
# La contrepartie est nommée : un contexte large enverrait tout le dépôt au
# démon. `plateforme/Dockerfile.dockerignore` le borne — BuildKit lit le
# `.dockerignore` NOMMÉ D'APRÈS LE DOCKERFILE quand il existe, ce qui évite de
# poser un `.dockerignore` à la racine, qui s'appliquerait AUSSI à l'image du
# Guacamole historique (`./Dockerfile`) et changerait un build qui ne nous
# appartient pas.
FROM node:24-alpine

WORKDIR /opt/plateforme

# Les manifestes d'abord, et l'installation ensuite : la couche d'installation
# n'est refaite que si les dépendances changent, pas à chaque édition de source.
COPY plateforme/package.json plateforme/package-lock.json ./

# 🔴 `npm ci` ET NON `npm install` : il exige que `package-lock.json` soit
# cohérent avec `package.json` et refuse de le RÉÉCRIRE. Une image bâtie avec
# `install` pourrait embarquer des versions que personne n'a testées.
#
# ⚠️ LES DÉPENDANCES DE DÉVELOPPEMENT SONT INSTALLÉES, et ce n'est pas un
# oubli : `npm start` lance `tsx src/index.ts`, et `tsx` est une
# `devDependency`. Un `--omit=dev` produirait une image qui échoue au démarrage
# avec « tsx: not found ». Le service tourne donc depuis les SOURCES
# TypeScript, sans étape de compilation — le même chemin que
# `npm run start` en développement, ce qui supprime toute divergence entre ce
# qui est éprouvé et ce qui est déployé.
RUN npm ci

COPY plateforme/tsconfig.json ./
COPY plateforme/src ./src

# 🔴 `proto/ts` EST POSÉ À `/opt/proto/ts`, ET L'ADRESSE N'EST PAS LIBRE : les
# sources vivent à `/opt/plateforme/src/<dossier>/<fichier>.ts` et remontent de
# trois crans (`../../../proto/ts/plateforme`), ce qui désigne exactement
# `/opt/proto/ts/plateforme`. C'est l'URL que Node nommait dans l'erreur.
# Déplacer `WORKDIR` sans déplacer ceci rouvrirait le même défaut.
#
# ⚠️ SEUL `proto/ts` ENTRE, pas `proto/src` (le crate Rust) : le service n'en a
# aucun besoin, et une image qui embarque ce qu'elle n'exécute pas se contente
# d'élargir sa surface.
COPY proto/ts /opt/proto/ts

# ⚠️ AUCUN `EXPOSE`, ET AUCUN PORT PUBLIÉ dans le fichier de composition. Le
# service écoute sur `PLATEFORME_HOTE`, qui vaut le NOM DU SERVICE : Node
# résout ce nom et se lie à l'adresse du conteneur sur le réseau interne. Seul
# le proxy l'atteint, et c'est ce que la spec §9 exige — la plateforme ne
# termine jamais TLS.
#
# ⚠️ `PLATEFORME_HOTE` n'a AUCUN défaut : si l'environnement ne le pose pas,
# `lireConfig` LÈVE au démarrage plutôt que d'écouter partout.
CMD ["npm", "start"]
