# Retrait du legacy — EXÉCUTÉ le 21 août 2026

**Décision** : celle du propriétaire du dépôt, prise le 21 août 2026 en réponse
explicite à la question posée. `CLAUDE.md` rangeait ce retrait parmi les
« décisions qui appartiennent au propriétaire du dépôt » depuis le 20 août ;
il cesse d'y figurer.

⚠️ **Ce document DÉRIVE, comme tous ceux de ce dépôt.** Chaque nombre porte sa
commande. **Ne recopier aucun chiffre d'ici sans la relancer.**

---

## 0. 🔴 Ce que ce retrait n'a PAS attendu

**Sur les dix verrous de `…/specs/2026-08-20-retrait-legacy-design.md`, DEUX
étaient satisfaits** au dernier relevé
(`…/plans/2026-08-21-retrait-legacy-etat-des-verrous.md`, § 1) : C1 et C2.
Trois étaient **non satisfaits** (C5, C6, C10), deux **partiels** (C4, C7),
deux **non évaluables** (C3, C8), un **dépendant de la lecture** (C9).

**Le retrait a néanmoins été exécuté.** C'est une décision, pas une mesure, et
elle est consignée comme telle : les fonctions que personne ne reprend
deviennent des **régressions assumées** (§ 3), pas des oublis. Le seul verrou
que ce chantier a effectivement refermé au passage est **C6** — non par
l'écoulement des sept jours d'arrêt qu'il exigeait, mais parce que le
conteneur et l'image **n'existent plus** (§ 2.3).

---

## 1. 🔴 Le fait qui a gouverné l'ordre des gestes : 890 lignes hors de git

La spec § 2.2 l'avait établi et ce chantier l'a **rejoué avant de supprimer
quoi que ce soit** — c'était la seule question dont une mauvaise réponse
aurait été irréversible :

```bash
for f in $(find index.js Dockerfile docker-compose.yml src web assets dist \
                test_winrm_fixed.js test_winrm_nodejs.js -type f | sort); do
  git ls-files --error-unmatch "$f" >/dev/null 2>&1 \
    && printf 'SUIVI    %6s  %s\n' "$(wc -l < "$f")" "$f" \
    || printf 'HORS-GIT %6s  %s\n' "$(wc -l < "$f")" "$f"
done
```

**Les trois fichiers HORS-GIT porteurs de source** — le reste des hors-git
étant des bundles générés (`dist/`, `src/dist/`) :

| Fichier | Lignes | Sort |
| --- | --- | --- |
| `web/index.js` | **804** | déjà archivé, **versionné**, en `docs/legacy/web-index.js` |
| `index.js` | **61** | déjà archivé, **versionné**, en `docs/legacy/racine-index.js` |
| `docker-compose.yml` | **25** | 🔴 **PAS archivé dans `docs/`, et à dessein** — voir § 1.1 |

**61 + 804 + 25 = 890.** Le compte de la spec tombe exactement, et il désigne
`docker-compose.yml` comme son troisième membre — ce que le commit
d'archivage `98cfd0b` n'avait pas pris (il en a versé 865).

### 1.1 🔴 Pourquoi `docker-compose.yml` ne pouvait pas entrer dans `docs/`

**Il porte les mots de passe Windows en clair**, et `docs/` est **versionné** :
l'y déposer aurait commité les identifiants — exactement la faute que le commit
`aad98d9` a dû défaire cinq commits plus tôt sur un `AGENT_SECRET`.
`docs/legacy/README.md` § 3 en garde donc la **structure sans une seule
valeur**, et c'est le bon arbitrage.

**Mais « ne pas l'archiver » ne pouvait pas vouloir dire « le détruire ».** Un
contrôle l'a montré, et c'est le fait neuf de ce chantier :

```bash
# comparaison par EMPREINTE, jamais par valeur affichée
for v in WINDOWS_HOSTNAME WINDOWS_ADMIN_USERNAME WINDOWS_ADMIN_PASSWORD \
         WINDOWS_USERNAME WINDOWS_PASSWORD; do
  a=$(grep -E "^ *- ${v}=" docker-compose.yml | sed -E "s/^ *- ${v}=//" | sha256sum | cut -c1-12)
  b=$(grep -E "^${v}="      .env              | sed -E "s/^${v}=//"      | sha256sum | cut -c1-12)
  [ "$a" = "$b" ] && echo "$v IDENTIQUE" || echo "$v DIFFERENT"
done
```

| Variable | Verdict |
| --- | --- |
| `WINDOWS_HOSTNAME` | IDENTIQUE |
| `WINDOWS_ADMIN_PASSWORD` | IDENTIQUE |
| `WINDOWS_USERNAME` | IDENTIQUE |
| `WINDOWS_ADMIN_USERNAME` | 🔴 **DIFFÉRENT** — 13 caractères dans `docker-compose.yml`, **14** dans `.env` |
| `WINDOWS_PASSWORD` | 🔴 **DIFFÉRENT** — **47** caractères dans `docker-compose.yml`, **8** dans `.env` |

🔴 **`WINDOWS_PASSWORD` n'est lu par AUCUN script du produit vivant** —
`grep -rn 'WINDOWS_PASSWORD' scripts/ agent/src client/src plateforme/src proto
deploiement` ne rend qu'une ligne, et c'est
`plateforme/src/securite/secrets.test.ts:37`, **le test qui vérifie qu'il n'est
pas commité**. Sa valeur de 47 caractères — le mot de passe du compte Windows
`guacamole` — **ne vivait donc plus que dans le fichier qu'on s'apprêtait à
supprimer.**

**Geste posé** : copie hors du dépôt, avant toute suppression.

```bash
mkdir -p ~/.guacamole-legacy && chmod 700 ~/.guacamole-legacy
cp -p docker-compose.yml ~/.guacamole-legacy/docker-compose.yml.legacy
chmod 600 ~/.guacamole-legacy/docker-compose.yml.legacy
cmp docker-compose.yml ~/.guacamole-legacy/docker-compose.yml.legacy   # identique
git -C ~/.guacamole-legacy rev-parse --show-toplevel                   # fatal: pas un dépôt git
```

⚠️ **Cette copie n'est PAS une sauvegarde du dépôt** : elle vit sur cette
machine seule, hors de tout historique, et disparaîtra avec elle. C'est un
choix — le seul autre serait de committer des identifiants.

### 1.2 Ce que `.env` établit, et qui a rendu le geste sûr

`.env` (gitignoré, `.gitignore:5`) porte les **cinq** `WINDOWS_*`. Supprimer
`docker-compose.yml` ne coupe donc **aucun** accès à la VM :
`scripts/winrm.js:5-9` lit `WINDOWS_HOSTNAME`, `WINDOWS_ADMIN_USERNAME` (défaut
`Administrateur`) et `WINDOWS_ADMIN_PASSWORD` — les trois que `.env` porte, et
dont la seule qui diverge (`WINDOWS_ADMIN_USERNAME`) est celle où **`.env` est
la valeur juste** : 14 caractères, `Administrateur`, le nom du compte sur une
installation Windows française. `docker-compose.yml` portait `Administrator`,
la forme anglaise, **et le legacy ne s'en plaignait pas parce qu'il ne s'en
servait que pour son propre chemin, aujourd'hui mort**.

---

## 2. Les gestes, dans l'ordre où ils ont été posés

### 2.1 L3 — retirer la surface morte

`assets/`, `web/`, `dist/`, `src/dist/`, `src/session.js`, `src/file.js`,
`Dockerfile`, `docker-compose.yml`.

### 2.2 L4 — retirer la découverte

`src/app.js`, `src/asset.js`, `src/cleanup.js`, `src/iconExtractor.js`,
`src/lnkParser.js`, `index.js`. **`src/` n'existe plus.**

### 2.3 L3/L4 — l'état Docker, qui n'est pas dans l'arbre

```bash
docker rm  -f guacamole-web-1     # → guacamole-web-1
docker rmi -f guacamole-web       # → 3 couches Deleted
docker ps -a | grep -i guacamole-web    # → aucun conteneur
docker images | grep -i guacamole-web   # → aucune image
```

🔴 **C'est ce geste qui referme C6, et pas l'écoulement du délai.** Le relevé
des verrous § 0.2 avait établi que l'arrêt **n'était pas durable** : la
politique du conteneur *existant* restait `always` quoi qu'on écrive dans le
fichier de composition, parce qu'un conteneur porte la politique reçue **à sa
création**. Un conteneur détruit ne redémarre pas.

### 2.4 L5 — solder

- **`test_winrm_fixed.js`, `test_winrm_nodejs.js`** supprimés. Leurs seules
  mentions restantes sont dans `docs/` — archive et relevés, aucun usage vivant.
- **`package.json`** réduit de **20 dépendances à UNE**, `nodejs-winrm`, la
  seule que `scripts/` requiert (`grep -rhoE "require\('[^']+'\)" scripts/*.js`
  → `require('nodejs-winrm')`, seul résultat).
- **`npm prune --omit=dev`** — élagage **hors ligne**, pour ne dépendre d'aucun
  registre : `removed 712 packages`. `node_modules/` racine : **141 Mo → 1,2 Mo**,
  16 entrées. `package-lock.json` a suivi tout seul : **8 entrées**.
- **Contrôle qui vaut** : `node -e "require('nodejs-winrm')"` résout toujours.
  L'élagage n'a pas cassé `scripts/winrm.js`.
- **Les core dumps** : **7 fichiers, 985,6 Mio** — ⚠️ **et non les 390 Mo que la
  spec § 7.1 annonçait**, le chiffre avait dérivé de plus du double. Supprimés.
- **`.gitignore`** : le bloc `index.js` / `docker-compose.yml` (lignes 14-19)
  retiré avec son commentaire, `src/dist/` retiré, et le **doublon `core.*`**
  (il figurait deux fois, lignes 4 et 23) ramené à une occurrence.

🔴 **La mine du motif `index.js` est désamorcée.** Le motif n'avait pas d'ancre
de début de chemin : il attrapait `index.js` **à toute profondeur**, si bien
qu'un futur `plateforme/src/index.js` aurait été ignoré **en silence**.
Contrôle après retrait : `git status --porcelain --untracked-files=all` ne
révèle **aucun** fichier nouvellement démasqué.

---

## 3. 🔴 Ce que le retrait emporte — les régressions assumées

⚠️ **Le § 4.1 de la spec listait onze fonctions « reprises par personne » et
donnait trois balayages rendant tous ZÉRO. CES ZÉROS SONT PÉRIMÉS** : ils
datent du 20 août, avant P1-P3, E2-E3, F2-F5, G1-G5 et S3-S4. Relancés le
21 août, aux mêmes chemins :

| Balayage du § 4.1 | Spec (20 août) | Relancé (21 août) |
| --- | --- | --- |
| `clipboard\|presse.papier` | 0 | **586** |
| `IShellLink\|\.lnk\|installeur\|catalogue` | 0 | **409** |
| `webmanifest\|serviceWorker` | 0 | **2** |

**Le pont fichiers ne reprend plus trois verbes sur huit mais SEPT** —
`proto/src/fichiers.rs` déclare `LISTER`, `ATTRIBUTS`, `LIRE`, `ECRIRE`,
`CREER`, `RENOMMER`, `SUPPRIMER`. Le seul manquant est **Existence**. L'argument
de la spec (« le sous-ensemble sans suppression ne fonctionne pas, l'idiome
Windows est écrire-temporaire → renommer → supprimer ») **ne tient donc plus**
sur son inventaire — ⚠️ mais il tient toujours sur sa mesure : le legs de ③ dit
que **cet idiome n'a jamais été exercé sur un éditeur réel**, et c'est *le seul
chemin par lequel une sauvegarde peut se perdre en silence*.

**Ce qui reste réellement perdu, vérifié fichier par fichier le 21 août :**

- 🔴 **Le service worker.** `find client plateforme -name 'sw.js' -o -name
  '*service-worker*'` ne rend **rien**. `web/sw.js` (45 l.) est supprimé et
  **personne ne le remplace**. Cité au cadrage § 5 ②, jamais planifié.
- 🔴 **Window Controls Overlay.** Jamais rendu — c'est déjà un legs ouvert de
  ⑥ (« le WCO n'a jamais été rendu »), le retrait ne fait que supprimer
  l'implémentation qui existait, dans `web/index.js`, **désormais lisible en
  `docs/legacy/web-index.js` et nulle part ailleurs**.
- ⚠️ **Le tactile.** `src/session.js:216` posait `enable-touch: true` et
  `web/index.js:183` instanciait `Guacamole.Mouse.Touchscreen`. **Aucune spec
  du nouveau produit ne nomme le tactile.** Un utilisateur sur tablette perd
  une fonction qui marchait.
- ⚠️ **L'impression.** `src/session.js:197` posait `enable-printing: true`. Le
  cadrage range l'impression en **v2+** : régression **connue, datée, assumée**.
- 🟡 **Le manifeste PWA par application** n'est PAS perdu : G5 l'a livré
  (`client/src/hub/manifeste.ts`, `client/dist/hub.webmanifest`). ⚠️ **Mais le
  hub n'est pas installable pour autant** — le legs de ④ tient : un `<img src>`
  ne porte pas d'`Authorization`, donc l'icône ne charge pas, donc le critère
  d'installabilité échoue et les `file_handlers` sont inertes.
- ⚠️ **Le microphone n'est plus une régression.** Le § 4.2 de la spec le classait
  ainsi le 20 août ; **E2 et E3 ont livré le sens montant** depuis. ⚠️ Et sa
  propre réserve tient toujours : que le sens montant du legacy ait réellement
  fonctionné n'est établi par **aucune pièce de ce dépôt** — c'était un réglage
  lu dans un fichier, jamais une mesure.

---

## 4. Ce que ce chantier N'ÉTABLIT PAS

- 🔴 **Que le nouveau produit couvre le legacy.** Huit verrous sur dix n'ont pas
  été refermés ; ils ont été **écartés par décision**. Ce document ne les
  déclare pas satisfaits, il déclare qu'on a choisi de passer outre.
- 🔴 **Que C3 soit vrai.** La comparaison de catalogue legacy/nouveau n'a
  **jamais** été jouée, et elle ne le sera plus : le service qui en était la
  moitié gauche n'existe plus. **Ce verrou est désormais définitivement non
  évaluable**, et c'est une conséquence directe du retrait.
- ⚠️ **Que la copie hors dépôt survive.** `~/.guacamole-legacy/` est sur une
  machine, pas dans un historique.

---

## 5. Contrôles de non-régression

```bash
env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
```

**Avant le retrait** (commit `a62467c`) : sortie **0**, « Les 10 étapes sont
passées », **18** en-têtes `==>`.
**Après le retrait** : sortie **0**, « Les 10 étapes sont passées », **18**
en-têtes `==>`. ⚠️ **Les deux comptes sont vrais de choses différentes** — dix
étapes du script, dix-huit en-têtes dont huit viennent de
`client : npm run design:verifier`.

**Comptes de tests au commit `a62467c`**, datés parce qu'un compte de tests
n'est attribuable qu'assorti de son commit : Rust **1 007** + **114**, client
**543**, `proto/ts` **308**, plateforme **566** en SQLite et **566** en Postgres.

**Règle des 500 lignes, relancée après la dernière édition** :
`agent/src/encode.rs` **1 536**, `agent/src/windows_source.rs` **630**. Le
tableau de dette de `CLAUDE.md` porte les deux, et ils sont justes. ⚠️ **Le
legacy en était invisible** (spec § 2.3) : `web/index.js`, 804 lignes, n'était
attrapé par aucune de ces commandes. Le problème s'éteint avec lui.
