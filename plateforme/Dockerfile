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
FROM node:24-alpine

WORKDIR /opt/plateforme

# Les manifestes d'abord, et l'installation ensuite : la couche d'installation
# n'est refaite que si les dépendances changent, pas à chaque édition de source.
COPY package.json package-lock.json ./

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

COPY tsconfig.json ./
COPY src ./src

# ⚠️ AUCUN `EXPOSE`, ET AUCUN PORT PUBLIÉ dans le fichier de composition. Le
# service écoute sur `PLATEFORME_HOTE`, qui vaut le NOM DU SERVICE : Node
# résout ce nom et se lie à l'adresse du conteneur sur le réseau interne. Seul
# le proxy l'atteint, et c'est ce que la spec §9 exige — la plateforme ne
# termine jamais TLS.
#
# ⚠️ `PLATEFORME_HOTE` n'a AUCUN défaut : si l'environnement ne le pose pas,
# `lireConfig` LÈVE au démarrage plutôt que d'écouter partout.
CMD ["npm", "start"]
