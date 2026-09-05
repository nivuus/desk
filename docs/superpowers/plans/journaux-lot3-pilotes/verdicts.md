# Lot 3, tâche 5 (item 4) — le rejeu des douze pilotes

**Mesuré le 5 septembre 2026.** ⚠️ **Aucun chiffre de ce document ne se
recopie sans relancer sa commande.**

Les douze sont ceux que la commande du plan énumère, **relancée** :
`grep -rln "signaling=" --include='*.mjs' --include='*.js' docs/superpowers/`
→ **12**.

---

## 1. Le verdict, en une phrase

🔴 **AUCUN DES DOUZE N'ÉTABLIT DE SESSION, ET LA CAUSE DOMINANTE N'EST PAS UN
DÉFAUT DE PILOTE : C'EST UNE DÉCISION D'EXPLOITATION.** Sept d'entre eux
s'arrêtent sur `POST /auth/connexion` → **404**, parce que le service tourne
en mode **`pomerium`**, où cette route est **retirée**.

| # | pilote | `node --check` | session | premier blocage, **mesuré** | nature |
| --- | --- | --- | --- | --- | --- |
| 1 | `pilote-accent-a1.mjs` | OK | non | `ENOENT … '/tmp/a1-identite.env'` | fichier d'amorçage de son propre harnais, absent |
| 2 | `pilote-recette-e2.mjs` | OK | non | `POST …/auth/connexion a rendu 404` | **mode `pomerium`** |
| 3 | `pilote-e3.mjs` | OK | non | `/auth/connexion a rendu 404` | **mode `pomerium`** |
| 4 | `injection-e3.js` | OK | — | **non exécutable** : c'est une injection, posée par `pilote-e3.mjs` | par nature |
| 5 | `pilote-f1.mjs` | OK | non | `/auth/connexion a rendu 404` | **mode `pomerium`** |
| 6 | `pilote-f2.mjs` | OK | non | `/auth/connexion a rendu 404` | **mode `pomerium`** |
| 7 | `pilote-f3.mjs` | OK | non | `/auth/connexion a rendu 404` | **mode `pomerium`** |
| 8 | `pilote-f4.mjs` | OK | non | `/auth/connexion a rendu 404` | **mode `pomerium`** |
| 9 | `pilote-f5.mjs` | OK | non | `/auth/connexion a rendu 404` | **mode `pomerium`** |
| 10 | `pilote-pp-p1.mjs` | OK | non | `obligatoire : sans identité, …` | paramètre d'identité non fourni |
| 11 | `pilote-pp-p2.mjs` | OK | non | `obligatoire : sans identité, …` | paramètre d'identité non fourni |
| 12 | `pilote-pp-p3.mjs` | OK | non | `obligatoire : sans identité, …` | paramètre d'identité non fourni |

🔵 **12/12 passent `node --check`** — le seul contrôle que le lot
`legs-sans-vm` avait pu jouer reste vrai aujourd'hui.

## 2. 🔴 CE QUI N'EST **PAS** LA CAUSE, ET QUE J'AI VÉRIFIÉ AVANT DE CONCLURE

**Neuf des douze portent `ws://192.168.3.1:8080` — le port d'avant le lot
10A — et le service écoute sur `3445`.** Il aurait été facile d'écrire « les
pilotes visent un port mort ». **C'est faux** : `:8080` est un **DÉFAUT**, et
`SIGNALING_WS` est un **paramètre** :

```js
const SIGNALING = process.env.SIGNALING_WS ?? 'ws://192.168.3.1:8080';
```

Tous les bras ci-dessus ont donc été joués avec les paramètres **imposés** —
`SIGNALING_WS`, `PLATEFORME_URL`, `CLIENT_URL`, `PREFIXE_VM`, `APP` — et ils
échouent **ailleurs**. ⚠️ **C'est exactement le piège que cette campagne
venait de payer sur quatre bras** : un paramètre qu'on ne passe pas n'échoue
pas, il prend une valeur. Le conclure ici aurait imputé aux pilotes un défaut
d'appel.

## 3. La cause dominante, mesurée sur le processus QUI ÉCOUTE

```
PLATEFORME_HOTE=192.168.3.1
PLATEFORME_AUTH=pomerium
PLATEFORME_PORT=3445

POST /auth/connexion  → HTTP 404   « introuvable »
GET  /auth/moi        → HTTP 200
```

Les sept pilotes concernés s'authentifient **par mot de passe**
(`RECETTE_EMAIL` / `RECETTE_MOTDEPASSE` → `POST /auth/connexion`). En mode
`pomerium` — **le mode retenu en production, sur décision du propriétaire**
(lots 10A à 13) — cette route rend le 404 générique, et l'identité passe par
`GET /auth/moi` avec l'en-tête `X-Pomerium-Claim-Email`.

🔴 **CE N'EST DONC NI UN DÉFAUT DES PILOTES NI UN DÉFAUT DU PRODUIT.** C'est
un **écart entre le montage pour lequel les pilotes ont été écrits**
(`motdepasse`) **et celui dans lequel le service tourne** (`pomerium`).
**Aucun pilote n'a été modifié pour le contourner** : les corriger reviendrait
à mesurer mes propres correctifs, pas les instruments d'origine.

## 4. 🔴 LA ROUGE PRESCRITE EST VACUEUSE ICI, ET C'EST MESURÉ

Le plan (étape 1) prescrit une **mutation** de `pilote-f1.mjs` — retirer
`/signal` de `SIGNALING_RELAIS` (ligne 32) — depuis une **copie nommée**, et
attend que **la session ne s'établisse pas**.

```
avant mutation : Error: /auth/connexion a rendu 404 (sans motif)
mutation posée : /signal retiré de SIGNALING_RELAIS   (ancre comptée : 1)
après mutation : Error: /auth/connexion a rendu 404 (sans motif)
restauration depuis /tmp/lot3-pilote-f1.copie → diff : identique
'/signal' présent à nouveau : 1
```

🔴 **LES DEUX ERREURS SONT IDENTIQUES : LA MUTATION NE CHANGE RIEN.**
L'exécution n'atteint **jamais** la ligne 32 — elle s'arrête avant, sur
l'authentification. **La rouge ne peut donc rien discriminer**, et l'écrire
verte aurait été un mensonge. ✅ La mécanique de la rouge, elle, est saine :
ancre comptée à 1, mutation posée, restauration **depuis la copie nommée**
(jamais `git checkout --`), et le garde de diff est vert.

## 5. Ce que cette tâche N'ÉTABLIT PAS

- 🔴 **AUCUN VERDICT MÉTIER D'ORIGINE N'A PU ÊTRE COMPARÉ.** L'étape 3 du plan
  demandait, pour chacun, `ice=connected` **et** le verdict métier retrouvé.
  **Aucune session ne s'établit**, donc **aucun des deux** n'est relevé, et
  **aucun écart au document de résultats d'origine n'est mesuré**.
- 🔴 **AUCUN PILOTE N'A ÉTÉ REJOUÉ DEUX FOIS.** La règle des deux exécutions
  n'a pas d'objet tant qu'aucune n'aboutit.
- ⚠️ **Ce relevé ne dit pas que les douze pilotes sont SAINS.** Il dit qu'ils
  passent `node --check` et qu'ils s'arrêtent tous **avant** d'avoir pu
  exercer quoi que ce soit du produit. Un défaut plus profond resterait
  invisible derrière le premier blocage.
- ⚠️ **Trois blocages ne sont pas le mode `pomerium`** et restent à traiter
  séparément : le `/tmp/a1-identite.env` d'`accent-a1` (son harnais le
  produit), et le paramètre d'identité des trois `presse-papier`.
- ⚠️ **Les voies mortes n'ont pas été atteintes.** Cinq de ces pilotes
  appellent `scripts/winrm.js` et sept touchent `/media/vm` ou `C:\dev` —
  tous **rendus bruyants** ce jour-là (sortie 78, successeur nommé) — mais
  **aucun n'a été exécuté assez loin pour y arriver** : ils s'arrêtent sur
  l'authentification, avant. Le panneau attend donc encore son premier
  lecteur.
- ⚠️ **Aucune ligne de produit ni d'instrument n'a été modifiée.**

## 6. La VM a-t-elle tenu ?

`etat-vm.sh` au début et à la fin de la tâche : **`extinctions=30`, inchangé**.
Aucune mesure n'est à cheval sur une extinction du domaine.

Journaux bruts versionnés : `rejeu-1.log` (les douze, avec la queue de sortie
de chacun), `rouge-f1.log` (la mutation, sa restauration et son garde).
