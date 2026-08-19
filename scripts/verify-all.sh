#!/usr/bin/env bash
#
# Enchaîne toutes les vérifications du projet en une seule commande, dans
# l'ordre : tests Rust, lint Rust, tests et typage TypeScript (client, proto,
# puis plateforme). S'arrête au premier échec et dit lequel.
#
# Pourquoi ce script existe : c'est le seul endroit du projet qui vérifie le
# typage TypeScript strict. `npm test` (Vitest) et `npm run build` (Vite)
# reposent tous deux sur esbuild, qui transpile sans jamais vérifier les
# types — un fichier peut donc avoir des tests entièrement verts avec un
# typage cassé. Pendant le chantier B (« Input jeu »), deux revues
# successives ont approuvé une tâche sur cette seule base, alors que
# `tsc --noEmit` échouait avec deux erreurs situées dans du code de
# production. Ce script existe pour que ça ne se reproduise pas.
#
# ⚠️ CE SCRIPT DÉPEND D'UNE INSTANCE POSTGRES, et c'est VOULU. `plateforme`
# éprouve son sous-ensemble SQL contre les DEUX moteurs, et un saut est un
# échec (spec §7.1 du sous-projet ⑤) : si l'instance est absente, l'étape
# `plateforme : npm run test:postgres` ÉCHOUE — elle ne se saute pas avec un
# avertissement. Un test qui disparaît quand sa dépendance manque rend vert un
# état qu'il n'a pas mesuré. La lancer :
#
#     docker compose -f docker-compose.plateforme.yml up -d
#
# Jusqu'au sous-bloc P1 (19 août 2026), le service de signaling était HORS de
# ce filet : ses tests n'étaient joués par aucune étape, et son paquet ne
# déclarait même pas de script `typecheck`. C'est la lacune que les trois
# étapes `plateforme` ferment.

set -euo pipefail

racine="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$racine"

etape() {
    echo
    echo "==> $1"
}

echec() {
    echo "ÉCHEC : $1" >&2
    exit 1
}

etape "cargo test --workspace"
cargo test --workspace || echec "cargo test --workspace"

etape "cargo clippy --workspace"
# Sans -D warnings : le workspace porte 33 avertissements dead_code
# préexistants, mesurés le 30 juillet 2026 (chantier de réduction de la dette
# de taille des fichiers) — le chiffre de 31 hérité du chantier B n'avait
# jamais été remesuré depuis. Tous dus à du code compilé pour Windows ou par
# les tests mais invisible à un lint Linux ordinaire (`geometry.rs`,
# `opus.rs`, `rebuild.rs`, `gamepad.rs`, `cursor.rs`, `audio.rs`,
# `diagnostics/entree.rs`, `transport/piste_audio.rs::set_audio_source`...).
# Les corriger n'est pas le rôle de ce script ; les masquer avec -D warnings
# le serait encore moins — on les laisse visibles, sans bloquer sur eux.
cargo clippy --workspace || echec "cargo clippy --workspace"

etape "client : npm test"
(cd client && npm test) || echec "client : npm test"

etape "client : npm run typecheck"
(cd client && npm run typecheck) || echec "client : npm run typecheck"

etape "proto : npm test"
(cd proto && npm test) || echec "proto : npm test"

etape "proto : npm run typecheck"
(cd proto && npm run typecheck) || echec "proto : npm run typecheck"

etape "plateforme : npm run test:sqlite"
(cd plateforme && npm run test:sqlite) || echec "plateforme : npm run test:sqlite"

etape "plateforme : npm run test:postgres"
(cd plateforme && npm run test:postgres) || echec "plateforme : npm run test:postgres"

etape "plateforme : npm run typecheck"
(cd plateforme && npm run typecheck) || echec "plateforme : npm run typecheck"

echo
echo "Toutes les vérifications sont passées."
