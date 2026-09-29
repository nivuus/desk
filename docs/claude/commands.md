# Commandes

> Extrait de [`CLAUDE.md`](../../CLAUDE.md) (lignes 322-427 de l'original), déplacé tel quel. Liens relatifs à ce dossier.

## 🔨 Commandes

**Tout commence par l'environnement.** `.env` est **gitignoré** et porte les
identifiants Windows et TURN :

```bash
set -a && source .env && set +a
```

⚠️ **`scripts/build-agent.sh` lancé sans avoir sourcé `.env` s'arrête EN
SILENCE**, après sa ligne « sources synchronisées », **sans message ni statut
d'erreur** — et le symptôme se lit exactement comme une compilation réussie et
muette. Voir les pièges.

⚠️ **`.env` ne porte AUCUNE variable `PLATEFORME_*`** : la plateforme ne
démarre donc pas après un simple `source .env`, et elle dit pourquoi.

### L'agent, sur la VM

⚠️ **Ce tableau documente le workflow de développement D'AVANT le 29 août
2026 — voir la réserve en tête de « Cycle de vie de la VM Windows » ci-dessus :
`C:\dev`, le montage CIFS et `scripts/winrm.js` (transport Basic) visent une
machine qui n'existe plus sous cette forme ; leur sort n'est pas tranché.**

| Commande | Ce qu'elle fait |
| --- | --- |
| `scripts/sync-agent.sh` | synchronise les sources Rust vers `C:\dev` (via `/media/vm`) |
| `scripts/build-agent.sh` | synchronise **puis compile sur la VM** |
| `scripts/run-agent.sh` | génère `C:\dev\run-agent.ps1` et lance l'agent en **session interactive** (tâche planifiée `/it`) |
| `scripts/check-session.sh` | 🔴 vérifie que l'agent tourne en **session 1** et non en session 0 — un agent en session 0 ne peut ni capturer une fenêtre ni injecter d'entrées |
| `scripts/stop-agent.sh` | arrête l'agent |
| `scripts/sonde-multifenetre.sh` | enchaîne les sondes du chantier D, **une par exécution du binaire** (ces API échouent par plantage de processus) |
| `node scripts/winrm.js '<PowerShell>'` | exécute une commande sur la VM — ⚠️ **en session 0** |

⚠️ **Ce piège aussi vise la compilation SUR la VM de développement d'avant le
29 août 2026 — même réserve qu'en tête de section : le sort de ce chemin
n'est pas tranché.** Trouvé par une recherche par le SENS (« l'horloge de la
VM », « le rlib ») après qu'une première recherche par motifs littéraux
l'avait manqué — voir
[`docs/superpowers/plans/2026-08-29-package-nivuus-resultats.md`](../superpowers/plans/2026-08-29-package-nivuus-resultats.md).

🔴 **Avant toute compilation qui touche `proto/`** — l'horloge de la VM avance
sur celle de l'hôte, et cargo saute alors le rlib de `proto` :

```bash
cargo clean --release -p proto -p agent   # les DEUX crates, jamais -p agent seul
```

### Les tests

```bash
cargo test --workspace                              # agent + proto
cargo check --target x86_64-pc-windows-gnu          # vérifie le code #[cfg(windows)] SUR L'HÔTE
cd client && npx vitest run                         # client/src/ SEUL
cd client && npx vitest run --dir ../proto          # 🔴 proto/ts/ — DEUX commandes, jamais une
cd plateforme && npm run test:sqlite                # et test:postgres — la double passe
./scripts/verify-all.sh                             # les 10 étapes
```

🔴 **`./scripts/verify-all.sh` N'EST PAS HERMÉTIQUE** : avec `TURN_URL` et
`TURN_SECRET` dans l'environnement — c'est-à-dire **après le `source .env` que
tout travail sur la VM exige** — six tests de signaling échouent. Le lancer
depuis un shell propre :

```bash
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

⚠️ **Le script compte DIX étapes et l'exécution affiche DIX-HUIT en-têtes
`==>`** (les huit de plus viennent de `client : npm run design:verifier`).
**Les deux sont vrais de choses différentes : dire lequel on annonce.**

⚠️ **`cargo check` : vérifier la NATURE des avertissements, jamais leur
nombre** — il dérive avec la fraîcheur du build. La formule « tous
`dead_code` » **n'est plus vraie du dépôt** (un `unused_variables` dans
`micro.rs`, quatre `clippy` hors famille). ⚠️ **Ne jamais compter par
`grep -c '^warning'`** : cela compte aussi la ligne de résumé.

### Les services

```bash
cd plateforme && npm start                    # exige PLATEFORME_HOTE et PLATEFORME_SECRET_JETON, sans défaut
docker compose -f docker-compose.plateforme.yml up -d          # Postgres de test
docker compose -f docker-compose.yml -f docker-compose.coturn.yml up -d coturn
scripts/netem.sh <lan|adsl|4g|congestionné|effondrement|off>   # dégradation réseau, TOUJOURS reposer `off`
```

**Administration de la plateforme** (`cd plateforme`) :
`npm run admin:utilisateur` (mot de passe **sur stdin seul**, jamais en `argv`),
`npm run admin:agent` (⚠️ `--vm` attend l'**identifiant**, pas le nom, et
`--adresse` n'a aucun défaut), `npm run admin:attribuer`.

### Les recettes navigateur

| Instrument | Ce qu'il établit |
| --- | --- |
| `client/verify-webrtc.mjs` | que le média traverse — **jamais par quel chemin** |
| `client/recette/paire-candidats.mjs` | la paire de candidats réellement employée, le RTT, le débit. `FORCER_RELAIS=1` impose `iceTransportPolicy: 'relay'` |

⚠️ **Toute mesure de plus de 5 minutes** exige `--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows` et `--disable-renderer-backgrounding` :
sans eux, Chrome gèle une page jamais mise au premier plan, et la session tombe
vers 331–340 s.

---
