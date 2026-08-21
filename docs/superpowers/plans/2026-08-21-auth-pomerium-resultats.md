# Chantier auth-pomerium — résultats

**Date** : 21 août 2026.
**Plan** : `docs/superpowers/plans/2026-08-21-auth-pomerium.md` (`b90e5d2`).
**Conception** : `docs/superpowers/specs/2026-08-21-auth-pomerium-design.md` (`0d5f8f3`).
**Journaux SDD** : `.superpowers/sdd/2026-08-21-auth-pomerium/` (huit briefs, huit
rapports de tâche, six diffs de revue).

> 🔴 **Toute sortie de ce document est celle de sa propre commande, relancée
> pour ce document.** Rien n'est recopié d'un rapport de tâche antérieur sans
> avoir été rejoué ici — précédent du § « Ce que ce document N'ÉTABLIT PAS »
> plus bas, qui nomme ce qui n'a PAS été rejoué et pourquoi.

---

## 0. Ce que les tâches 1 à 7 ont livré

| Tâche | Commit | Ce qu'elle a posé |
| --- | --- | --- |
| 1 | `f5dc097` | `PLATEFORME_AUTH` (mode, défaut `pomerium`, lève sur valeur inconnue) et le refus de l'écoute universelle en mode `pomerium` |
| 2 | `faf570c` | `GET /auth/moi` : l'en-tête `X-Pomerium-Claim-Email` échangé contre le MÊME jeton interne que le mot de passe |
| 3 | `3ca25d8` | `POST /auth/connexion` et `/auth/rafraichir` retirés (404 générique) en mode `pomerium` |
| 4 | `823834f` | La page cliente demande son identité avant de montrer un formulaire |
| 5 | `0ac7522` + `d29870e` | Le relais de signaling quitte `/` pour `/signal` |
| 6 | `f7f5133` | `nginx.conf` : la table `map` du legacy disparaît avec la racine partagée |
| 7 | *(hors dépôt, `/opt/nivuus/Pomerium/config.yaml`)* | Les trois routes Pomerium : `/signal` et `/agent` publiques, la route nue authentifiée avec `pass_identity_headers: true` |

Ce document est la tâche **8** : la recette et la clôture.

---

## 1. La VM et l'agent — recompilation

Séquence exécutée, dans l'ordre :

```
$ virsh list --all
 ID   Nom       État
-----------------------
 -    Windows   fermé

$ virsh start Windows
Domaine 'Windows' démarré

[attente WinRM (port 5985), puis attente /media/vm/dev — les deux atteints]

$ set -a && source .env && set +a
$ git status --porcelain agent/ proto/
[vide — arbre propre]
```

**`Get-Process agent` AVANT le build** — trois survivants trouvés, arrêtés par
`scripts/stop-agent.sh` avant toute compilation (sans quoi `link.exe` échoue
en 1104 / `os error 5`) :

```
Id ProcessName StartTime
-- ----------- ---------
1056 agent     21/08/2026 17:52:14
2656 agent     21/08/2026 17:52:17
9808 agent     21/08/2026 21:44:32
```

**`cargo clean --release -p proto -p agent`, sur les DEUX crates, exécuté DANS
`C:\dev` sur la VM** (c'est là que l'horloge en avance sur l'hôte fait sauter
le rlib de `proto` — nettoyer le `target/` de l'hôte n'aurait rien changé au
`target/` de la VM, qui sont deux répertoires disjoints) :

```
cargo :      Removed 20 files, 40.0MiB total
```

**`scripts/build-agent.sh`**, `.env` sourcé au préalable :

```
    Finished `release` profile [optimized] target(s) in 19.34s
```

**19,34 s : un vrai travail** — une compilation de 0,13 s aurait été l'aveu
d'un `cargo` qui n'a rien reconstruit.

---

## 2. Prouver que le binaire est bien le neuf

🔴 **La taille ne prouve rien, dans les deux sens.** Le discriminant est une
chaîne posée par la tâche 5 elle-même (`/signal`), cherchée sur le binaire que
`run-agent.sh` lance, **avec un témoin négatif** — une chaîne qu'on sait
absente de tout le dépôt :

```
$ node scripts/winrm.js 'Select-String -Path C:\dev\target\release\agent.exe -Pattern "/signal" -Encoding ascii -SimpleMatch | Measure-Object | % Count'
1

$ node scripts/winrm.js 'Select-String -Path C:\dev\target\release\agent.exe -Pattern "chaine-jamais-posee-par-nous-xyz" -Encoding ascii -SimpleMatch | Measure-Object | % Count'
0
```

`/signal` → **1** (non nul), le témoin négatif → **0**. Les deux, comme
prescrit : le binaire fraîchement compilé porte bien la chaîne posée par la
tâche 5, et la méthode de recherche ne rend pas systématiquement « trouvé ».

⚠️ **Écart avec le brief** : `-Encoding Byte` n'existe plus dans le jeu de
valeurs de ce `Select-String` (`unicode;utf7;utf8;utf32;ascii;bigendianunicode;
default;oem`) — `Byte` a été retiré des versions récentes de PowerShell.
`-Encoding ascii` produit le même effet recherché (`/signal` est de l'ASCII
pur dans le binaire) et a été substitué.

---

## 3. Redémarrer la plateforme dans sa configuration neuve

**Processus AVANT**, relevé sur le port en écoute :

```
PID 3832468, /root/.nvm/…/node … src/index.ts
Environnement (extrait) :
  PLATEFORME_HOTE=0.0.0.0
  (PLATEFORME_AUTH absent de CET environnement de lancement — mais le code
   qui tournait alors ne le connaissait pas encore : c'est l'ANCIEN code,
   d'avant la tâche 1)
```

**Arrêté par PID**, jamais par motif :

```
$ kill 3832468        # SIGTERM — n'a pas suffi, le process restait vivant 5 s
$ kill -9 3832468      # SIGKILL
$ ps -p 3832468
    PID CMD
3832468 [MainThread] <defunct>
$ ss -ltnp | grep ':8080 '
[rien — port 8080 libre]
```

**PID arrêté et consigné : `3832468`.**

**Relancé** depuis l'arbre courant (`git worktree` non nécessaire, branche
`auth-pomerium` déjà active), avec `PLATEFORME_HOTE=192.168.3.1` et
`PLATEFORME_AUTH` **absent** (donc son défaut `pomerium`) :

```
plateforme à l'écoute sur 192.168.3.1, le port 8080
```

**Nouveau PID : `725102`.** Environnement relu depuis `/proc/725102/environ`,
le processus qui ÉCOUTE et non celui qu'on croit avoir lancé :

```
PLATEFORME_HOTE=192.168.3.1
PLATEFORME_BASE=sqlite
PLATEFORME_PORT=8080
PLATEFORME_ORIGINE_CLIENT=http://192.168.3.1:5173
(PLATEFORME_AUTH : absent de la liste — donc son défaut, 'pomerium')
```

---

## 4. La chaîne d'identité, sans navigateur — critère ①

Les quatre bras, tous par `curl` sur `http://192.168.3.1:8080`, tous rejoués
pour ce document (21 août 2026, ~19h49 UTC) :

### Bras 1 — avec l'en-tête d'identité

```
$ curl -sS -i http://192.168.3.1:8080/auth/moi -H 'X-Pomerium-Claim-Email: maxime.g.allanic@gmail.com'
HTTP/1.1 200 OK
content-type: application/json; charset=utf-8
X-Content-Type-Options: nosniff
Cache-Control: no-store

{"acces":"***RETIRE-DE-L-HISTORIQUE***"}
```

**200, et un jeton.** Le compte est créé au premier passage (branche ①
de `identifiantDe`, tâche 2).

### Bras 2 (la ROUGE) — sans l'en-tête

```
$ curl -sS -i http://192.168.3.1:8080/auth/moi
HTTP/1.1 401 Unauthorized
content-type: application/json; charset=utf-8

{"refus":"identite-absente"}
```

**401, motif `identite-absente`.** Aucun repli sur un utilisateur par
défaut : sans l'en-tête, aucun jeton n'est délivré à personne.

### Bras 3 — la porte du mot de passe

```
$ curl -sS -i -X POST http://192.168.3.1:8080/auth/connexion -H 'Content-Type: application/json' -d '{"email":"x@x.com","motDePasse":"x"}'
HTTP/1.1 404 Not Found
content-type: text/plain; charset=utf-8

introuvable
```

**404** — le mode `pomerium` a bien fermé la route de mot de passe (tâche 3) :
elle rend le 404 générique du serveur, pas une réponse propre à
`/auth/connexion`.

### Bras 4 — la montée WebSocket

```
$ curl -i --http1.1 -H "Connection: Upgrade" -H "Upgrade: websocket" \
    -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: ***RETIRE-DE-L-HISTORIQUE***" \
    http://192.168.3.1:8080/
HTTP/1.1 404 Not Found
Connection: close

$ curl -i --http1.1 -H "Connection: Upgrade" -H "Upgrade: websocket" \
    -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: ***RETIRE-DE-L-HISTORIQUE***" \
    http://192.168.3.1:8080/signal
HTTP/1.1 101 Switching Protocols
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=
```

**`/` refuse la montée (404, connexion fermée) ; `/signal` l'accepte (101
Switching Protocols).** C'est la tâche 5, mesurée sur le processus réellement
relancé.

### Contrôle supplémentaire — aucun fichier statique servi par la plateforme

Mesure qui motive la restriction de périmètre du § 6 :

```
$ curl -sS -i http://192.168.3.1:8080/
HTTP/1.1 404 Not Found
content-type: text/plain; charset=utf-8

introuvable
```

Rejoué sur le processus **neuf** (PID `725102`, config `pomerium` +
`192.168.3.1`) : le résultat est identique à la mesure qui a motivé la
restriction — la plateforme ne sert `/` à personne, avec ou sans identité.

---

## 5. Un en-tête FORGÉ, à travers `https://app.allanic.me`

```
$ curl -sS -i https://app.allanic.me/auth/moi -H 'X-Pomerium-Claim-Email: maxime.g.allanic@gmail.com'
HTTP/2 302
location: https://authenticate.allanic.me/.pomerium/sign_in?...
x-pomerium-intercepted-response: true
server: envoy

$ curl -sS -i https://app.allanic.me/auth/moi
HTTP/2 302
location: https://authenticate.allanic.me/.pomerium/sign_in?...
x-pomerium-intercepted-response: true
server: envoy
```

**Ce qui est observé** : les deux requêtes — avec et sans l'en-tête forgé —
rendent la **même** réponse : un `302` vers la page de connexion Google
d'`authenticate.allanic.me`, servi par Envoy et marqué
`x-pomerium-intercepted-response`. Aucune requête n'atteint le backend
`192.168.3.1:8080` dans les deux cas ; Pomerium tranche l'authentification
**avant** de router quoi que ce soit, et un en-tête `X-Pomerium-Claim-Email`
posé par le CLIENT, sans session Pomerium valide, n'a produit **aucun effet
observable** — ni accès, ni différence de comportement avec son absence.

⚠️ **Ce que cette mesure établit, et ce qu'elle n'établit pas** : elle établit
que Pomerium ne laisse pas passer une requête non authentifiée jusqu'au
backend, en-tête forgé ou non — donc que le point d'entrée public
(`app.allanic.me`, sans session) est fermé. Elle n'établit **rien** sur ce qui
se passerait avec une session Pomerium **valide** doublée d'un en-tête forgé
en plus de celui posé par Pomerium (l'arbitrage ② de la spec — « aucune
signature vérifiée » — reste entier, voir § 7) : ce test n'a pas de session
Google, donc ne peut pas l'éprouver.

---

## 6. La passe complète

```
$ env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh
[…]
 Test Files  59 passed (59)
      Tests  591 passed (591)

==> plateforme : npm run typecheck
[…]
Les 10 étapes sont passées.
```

**Exit 0.** Le script **annonce dix étapes** et affiche **dix-huit
en-têtes `==>`** — les deux sont vrais de choses différentes :

```
cargo test --workspace
cargo clippy --workspace
client : npm test
client : npm run typecheck
client : npm run design:verifier
npm run build (sans quoi §7.3 et §7.7 jugeraient le build d'avant)
§7.4  les trois blocs de thème ne divergent pas
§7.1  les contrastes tiennent les seuils WCAG
§7.2  aucune couleur littérale hors de tokens.css
§7.6  aucun token orphelin, aucun var() non déclaré
§7.9  toute classe employée est déclarée, et une primitive atteint le produit
§7.3  toute surface bâtie porte les tokens
§7.7  le poids CSS ne dérive pas
proto : npm test
proto : npm run typecheck
plateforme : npm run test:sqlite
plateforme : npm run test:postgres
plateforme : npm run typecheck
```

**Dix-huit en-têtes** (`client : npm run design:verifier` en imprime huit à
lui seul, §7.1 à §7.9 moins un). **J'annonce les DIX ÉTAPES** comme le
critère de réussite du script (`Les 10 étapes sont passées.`, code de sortie
0) ; les dix-huit en-têtes sont listés ci-dessus pour qui voudrait les
recompter.

---

## 7. Ce que cette recette N'ÉTABLIT PAS

### 7.1 — Le critère ⑦ n'a pas été joué dans un navigateur

Le brief de tâche demande de jouer, **dans un navigateur réel derrière
Pomerium** : la page ne montre jamais le formulaire, et le bureau s'affiche.
**Deux blocages, mesurés, l'empêchent — et aucun des deux n'a été contourné,
simulé, ni remplacé par un succédané présenté comme le critère :**

🔴 **LES DEUX BLOCAGES NE SONT PAS DE MÊME NATURE, ET LES RANGER ENSEMBLE ÉTAIT
LE DÉFAUT DE CADRAGE DE CE §.** Le second (§ ci-dessous, la session Google) est
bien une limite de recette : le mécanisme est sain, seul l'instrument manque.
**Le premier est un défaut de CONCEPTION du déploiement, déjà appliqué au
`config.yaml` réel** — et il ne se solde pas par une mesure de plus.

1. 🔴 **CE BLOCAGE N'EST PAS UN EMPÊCHEMENT DE RECETTE : C'EST UN DÉFAUT DE
   CONCEPTION, ET LE CADRAGE DE CE PARAGRAPHE A ÉTÉ CORRIGÉ LE 21 AOÛT 2026
   (revue transverse).** La rédaction d'origine le rangeait parmi « ce que
   cette recette n'établit pas », d'où un successeur conclurait « il reste à
   MESURER ». **Ce qu'il faut comprendre est « il reste à CONCEVOIR ».**

   **La plateforme ne sert AUCUN fichier statique.** Mesuré au § 4 ci-dessus,
   sur le processus neuf : `GET /` rend `404 introuvable`. C'est **nginx**
   qui sert la page (`root /usr/share/nginx/html`).

   🔴 **OR LA SPEC §7.2 FAIT POINTER LA ROUTE NUE DE POMERIUM — celle que son
   propre commentaire nomme « La page et l'API » — VERS CE BACKEND-LÀ**, en
   sautant nginx. **Et ce bloc a été APPLIQUÉ au `config.yaml` réel.** La
   conséquence n'est pas qu'un critère n'a pas été joué : c'est que, tel qu'il
   est déployé aujourd'hui, **le chemin de la page ne peut servir aucune page**
   — `https://app.allanic.me/` traverse Pomerium et rencontre un `404` du
   backend. Le §5 le montre d'ailleurs sans le nommer : ce qui répond derrière
   l'authentification est l'API, jamais un document.

   ⚠️ **AUCUN DES DEUX CHEMINS DE REMÈDE N'EST TRIVIAL, ET C'EST POURQUOI
   C'EST UNE CONCEPTION ET NON UNE CORRECTION** :
   - **router Pomerium vers nginx** — son `listen 80` est un `return 301` vers
     HTTPS, et le viser depuis un Pomerium qui a déjà terminé TLS ferait une
     boucle de redirection ; il faudrait lui apprendre à ne pas rediriger ce
     qui vient de Pomerium. Son `listen 443` exige `deploiement/tls/`,
     **gitignoré et absent** de cette machine ;
   - **doter la plateforme d'un servant de fichiers statiques** — du code
     neuf, avec son périmètre, ses en-têtes et sa CSP, qu'aucune tâche de ce
     chantier n'a cadré.

   ⚠️ **Aucune de ces deux voies n'était du ressort de la tâche de recette**,
   et ce document ne les a pas départagées ; ce qu'il change ici est le
   CADRAGE, pas le constat. Le legs est inscrit à `CLAUDE.md`, § « Legs
   ouverts », et le §7 de la spec porte l'annotation correspondante.
2. **Le critère ⑦ exige une session Google réelle.** Pomerium authentifie
   contre Google (vu au § 5 : `authenticate.allanic.me`, `.pomerium/sign_in`).
   Aucun Chrome sans interface ne franchit ce flux OAuth complet (mot de
   passe, éventuel 2FA, consentement) sans qu'un humain ne le conduise.
   **Remède** : un humain, dans un vrai navigateur, avec sa session Google —
   ce que ce document ne peut pas fabriquer sans mentir sur ce qu'il a vu.

**Ce document ne prétend PAS avoir joué le critère ⑦.** Ce qu'il établit à sa
place — la chaîne d'identité complète, testée à la fois du côté service (§ 4,
injection directe de l'en-tête) et du côté proxy (§ 5, tentative de forge) —
est la part **mesurable** du même mécanisme, et elle est verte des deux
côtés.

🔴 **ET IL NE FAUT PAS EN CONCLURE « IL RESTE À MESURER ».** Le blocage n°1
ci-dessus n'attend aucun instrument : il attend une **décision de conception**
— router Pomerium vers nginx, ou doter la plateforme d'un servant statique.
Tant qu'elle n'est pas prise, jouer le critère ⑦ est impossible **et le chemin
de la page reste servi par un backend qui rend `404`**. C'est inscrit au § « Legs
ouverts » de `CLAUDE.md`, et annoté au §7 de la spec.

### 7.2 — Ce qui n'a pas changé

- **`TURNS sur 443` n'est pas livré** — Pomerium ne le solde pas, et ce
  chantier n'y touche pas (legs déjà consigné dans « Legs ouverts »,
  sous-projet ⑤).
- **Aucune signature n'est vérifiée** sur `X-Pomerium-Claim-Email` —
  arbitrage ② de la spec, assumé : l'en-tête reste falsifiable par quiconque
  atteint le port `8080`, VM Windows comprise. § 5 mesure seulement que le
  point d'entrée PUBLIC (`app.allanic.me`, sans session) ne laisse rien
  passer — pas que le port `8080` lui-même serait protégé contre un accès
  direct.
- **La machinerie de mots de passe reste vivante** (`mot-de-passe.ts`,
  `depot/jeton.ts`, `jeton_rafraichissement`, `npm run admin:utilisateur`) —
  arbitrage ④, exercée par ses seuls tests dans la passe du § 6.
- **Le hub reste non installable** (`<img src>` sans `Authorization`) — legs
  ④ inchangé, la politique Pomerium lève l'obstacle côté proxy, pas côté
  service.
- **Aucune latence n'est mesurée** — le legs « jamais mesurée depuis D1 »
  reste entier.

---

## 8. Statut

**DONE_WITH_CONCERNS.** Tout ce qui est mesurable sans navigateur et sans
session Google réelle est **vert** : la recompilation de l'agent (§ 1-2), les
quatre bras de la chaîne d'identité (§ 4), la tentative de forge à travers
Pomerium (§ 5), et la passe complète à dix étapes (§ 6). Le seul critère non
joué — ⑦, dans un navigateur — l'est pour deux raisons mesurées et nommées
(§ 7.1), pas par omission.

**PID à connaître pour la suite** : la plateforme tourne sous **`725102`**,
`PLATEFORME_HOTE=192.168.3.1`, `PLATEFORME_AUTH` absent (mode `pomerium`).
