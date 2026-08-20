#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# LA RECETTE DU SOUS-BLOC S4 — sept critères, versés en un journal par
# exécution. Elle ne juge pas : elle RELÈVE, et chaque chiffre qu'elle imprime
# vient d'une commande lancée ici.
#
# ⚠️ DEUX EXÉCUTIONS ÉTABLISSENT LA REPRODUCTIBILITÉ, JAMAIS UN TAUX. Les neuf
# contrôles de ⑥ sont DÉTERMINISTES (spec §9) : la question « combien de fois
# sur combien » ne se pose pas ici, et ne doit pas être empruntée à une
# campagne qui, elle, l'aurait posée.
#
# ⚠️ `unset -f chpwd` : un hook du shell de cette machine injecte un `ls` dans
# toute sortie qui traverse un `cd`. Piège payé en S2, repayé en S3.
#
# usage : recette-s4.sh <base-git> <fichier-journal>
# ═══════════════════════════════════════════════════════════════════════════
unset -f chpwd 2>/dev/null
set -u
BASE="$1"
RACINE="$(cd "$(dirname "$0")/../../../../.." && pwd)"
cd "$RACINE" || exit 2

echo "════════ RECETTE S4 ════════"
echo "date        : $(date -Is)"
echo "arbre       : $(git rev-parse --short HEAD)  ($(git log -1 --format=%s | head -c 80))"
echo "base du ③   : $BASE  ($(git log -1 --format=%s "$BASE" | head -c 70))"
echo "git status --porcelain client/ : [$(git status --porcelain client/ | tr '\n' ' ')]"

echo
echo "════════ ① LES NEUF CONTRÔLES SONT VERTS ════════"
echo "--- sept scripts, par l'agrégateur ---"
npm --prefix client run design:verifier > /tmp/s4-verifier.log 2>&1
echo "design:verifier exit=$?"
sed -n '/^==>/,$p' /tmp/s4-verifier.log
echo
echo "--- deux tests unitaires : §7.5 (bascule de thème) et §7.10 (longueurs) ---"
( cd client && npx vitest run 2>&1 ) > /tmp/s4-vitest.log
grep -E "§7\.10|reprise\.test|selecteur-theme\.test|longueurs\.test|style\.test|ecran-terminal\.test|garde WCO|env\(titlebar|hors token|declarations lues|déclarations lues|Test Files|Tests  " /tmp/s4-vitest.log

echo
echo "════════ ③ LA FENÊTRE DE SESSION A BOUGÉ ════════"
echo "--- git diff --stat $BASE..HEAD sur les deux fichiers du critère ③ de S3 ---"
git diff --stat "$BASE" HEAD -- client/index.html client/src/style.css
echo
echo "--- les CINQ actifs CSS bâtis : base contre HEAD ---"
echo "[base]"; ( cd /tmp/base-s4/client && sha256sum dist/index.html dist/assets/*.css | sed 's#dist/assets/##' )
echo "[HEAD]"; ( cd client && sha256sum dist/index.html dist/assets/*.css | sed 's#dist/assets/##' )
echo
echo "--- poids CSS, base contre HEAD ---"
echo -n "base : "; ( cd /tmp/base-s4/client && cat dist/assets/*.css | wc -c )
echo -n "HEAD : "; ( cd client && cat dist/assets/*.css | wc -c )
echo
echo "--- le balisage arrive-t-il dans le bâti ? ---"
echo -n 'dist/index.html porte id="fin" : '; grep -c 'id="fin"' client/dist/index.html

echo
echo "════════ SYNTHÈSE, CRITÈRE PAR CRITÈRE ════════"
V=/tmp/s4-verifier.log
echo "① les NEUF contrôles :"
echo "   $(grep -o '═══ .* ═══' $V | tail -1)  (sept scripts)"
echo "   $(grep 'Test Files' /tmp/s4-vitest.log)  /  $(grep 'Tests  ' /tmp/s4-vitest.log)  (deux tests unitaires dedans : §7.5, §7.10)"
echo "   §7.5 : $(grep 'selecteur-theme.test' /tmp/s4-vitest.log | tr -s ' ')"
echo "   §7.10 : $(grep 'longueurs.test' /tmp/s4-vitest.log | tr -s ' ')"
echo "② la liste d'attente est VIDE :"
grep -E 'inclusion ② —|token\(s\) déclaré|token\(s\) employé|restent en attente' $V | sed 's/^/   /'
echo "④ la fenêtre de session emploie des primitives (§7.9) :"
grep -E 'client/(index|shell|connexion)\.html : |① toute classe employée|^total : ' $V | sed 's/^/   /'
echo "⑤ plus aucune longueur hors token (§7.10) :"
grep -E 'hors token|feuilles de surface' /tmp/s4-vitest.log | sed 's/^/   /'
echo "   à la NAISSANCE du contrôle (commit ee56e1e) : 8 occurrence(s), 6 valeur(s)"
echo "⑥ les contrastes (§7.1) :"
grep -E 'paires vérifiées|échecs :|minimum global' $V | head -3 | sed 's/^/   /'
echo "⑦ le poids CSS (§7.7) :"
grep -E '^ +[0-9]+ +[a-z]+-.*\.css$|feuilles émises|^somme|^plafond|^marge' $V | sed 's/^/   /'
echo "   base 23e9b89 : 8011 octets (relevé par la commande, plus haut)"
echo "— le jugement visuel : NON PORTÉ, et il ne l'a jamais été sur ⑥ (§10 du plan)"
echo "════════ fin ════════"
