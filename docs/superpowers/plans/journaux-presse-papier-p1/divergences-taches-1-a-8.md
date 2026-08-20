# P1 (presse-papier) — divergences relevées en exécutant les tâches 1 à 8

Toutes relevées **par la commande**, dans l'arbre courant, en jouant les
tâches. Rien ici n'est recopié d'un document antérieur.

## 🔴 D1 — `agent/src/main.rs` est à 505 lignes AVANT P1, et le plan ne le budgète pas

Le tableau des 500 lignes du plan (§« La règle des 500 lignes ») liste seize
fichiers touchés par P1. **`agent/src/main.rs` n'y figure pas**, alors que la
tâche 4 le modifie explicitement (« Modify: `agent/src/main.rs` (une ligne
`mod presse_papier;` + son commentaire) »).

Relevé par la commande :

| Commit | `wc -l agent/src/main.rs` |
| --- | --- |
| `b13e08e~1` | 463 |
| `264c275~1` | **470** |
| `bb5a40f` (dernier commit avant P1) | **505** |
| `e5bfb4b` (tâche 4 de P1) | **513** |

**C'est `264c275` (« corrige(identite): un seul canal /agent par VM ») qui a
fait franchir le plafond**, de 470 à 505, +42/−7 — et rien ne l'a déclaré :
le fichier n'est **pas** dans le tableau de dette de `CLAUDE.md`, qui n'a
toujours que `encode.rs` (1536) et `windows_source.rs` (630).

**Ce que la tâche 4 y a ajouté : +8, dont 1 seule ligne de code**
(`mod presse_papier;`) et 7 de commentaire justifiant la racine nue et
l'absence de `#[cfg(windows)]`.

**Aucune extraction ne l'accompagne, et ce n'est pas une omission** : une
déclaration `mod` ne peut vivre que dans `main.rs`. Le remède réel est le
découpage de `main.rs` lui-même, hors du périmètre de P1. **Déclaré plutôt que
dissimulé**, sur le précédent des +7 de D6 et des +2 de D10 sur
`windows_source.rs`. La compression du commentaire a été écartée : ce dépôt
écrit noir sur blanc qu'elle échangerait une vérité contre un nombre.

**Pour la tâche 18** : `agent/src/main.rs` mérite une ligne au tableau de
dette, et sa cause n'est pas P1.

## 🔴 D2 — la sonde P0 rendait un verdict éliminatoire FAUX (corrigé)

Voir le commit `d4a3499` et le journal `p0-sonde-0-instrument-defectueux.log`.
Trois zéros au compteur ont **deux** causes — l'appel a échoué, ou rien n'a
jamais été copié depuis le démarrage de la station de fenêtres — et la
première version les confondait. Corrigé : la phase A **désambiguïse** par une
écriture au lieu de conclure.

## ⚠️ D3 — la tâche 6 laisse l'arbre `agent` NON COMPILABLE sans un bras hors périmètre

`AgentControl::Clipboard` rend non exhaustif le `match` de journal de
`agent/src/transport/controle.rs:98` (`error[E0004]`). Le plan range ce
fichier dans la famille 4 (tâches 9 à 14), mais **le compilateur l'exige dès la
tâche 6** : sans lui, tout commit entre la tâche 6 et la tâche 13 laisse
`cargo test -p agent` en échec de compilation. Le bras (une ligne, aucun
comportement) a donc été posé dans la tâche 8.

## 🔴 D4 — `pont_media.rs` n'a PAS son bras, et l'oubli y est SILENCIEUX

Hors périmètre (tâche 9, qui apporte aussi `Recu::PressePapier` de la tâche
10). L'avertissement et son contrôle vivent dans le doc-comment de
`DepuisCapteur::PressePapier`. **Contrôle, en une commande :**

```
grep -n 'DepuisCapteur::PressePapier' agent/src/capteur/pont_media.rs
```

**Il rend 0 aujourd'hui — relevé.** Il doit rendre 1 après la tâche 9. Tant
qu'il rend 0, la première trame de presse-papier tuerait le fil
`lire_le_media` **sans panne apparente**.

## ⚠️ D5 — l'empreinte de la sonde est un SHA-1, pas un SHA-256

Le plan écrit « SHA-256 tronqué à 12 hexadécimaux ». `sha1` est déjà une
dépendance de la caisse `agent` (bail TURN), `sha2` ne l'est pas, et le plan
pose en tête que **P1 n'ajoute aucune dépendance**. La propriété recherchée —
distinguer deux textes sans jamais en écrire un — est rendue identiquement.

## ⚠️ D6 — les copies de la sonde ne sont pas faites « à la main »

Le plan prescrit de copier à la main dans le Bloc-notes. Quatre des cinq
copies sont faites par `Set-Clipboard` depuis un processus PowerShell
**distinct**, en session 1 ; **la cinquième est une vraie copie Bloc-notes**
(`SendKeys` Ctrl+A / Ctrl+C), observée aux deux exécutions. L'instrument est
versé (`instrument-p0-copieur.ps1`).

**Nécessité de la session 1, vérifiée et non supposée** : `Get-Clipboard` par
WinRM (session 0) rend `-1`. Le presse-papier est bien par station de
fenêtres, et une mesure faite en session 0 n'aurait rien mesuré.

## ⚠️ D7 — E10 : aucun `env_remove`, et la dissymétrie est déclarée

Le plan refuse de modifier `lanceur.rs` (ce serait une convention que les huit
`MULTIFENETRE_*` ne suivent pas). Or `lanceur.rs` porte **dans son propre
code** la doctrine inverse : « un ordre de test est une propriété qui change,
un `env_remove` non ». La décision du plan est suivie, **et la tension est
inscrite dans le commentaire du bras d'aiguillage** plutôt que laissée
dormante. Relevé exact de ce que `lancer_capteur` retire : `SUPERVISEUR`,
`PONT`, `TEST_FILE`, `WINDOW_TITLE`, `AGENT_VM`, `AGENT_SECRET`,
`AGENT_JETON` — **aucune variable de diagnostic**.

## Comptes de tests, relevés

| Suite | Départ | Après les tâches 1 à 8 |
| --- | --- | --- |
| `cargo test -p agent` | 650 | **663** (+13, tâche 4) |
| `cargo test -p proto` | 79 | **83** (+4, tâche 6) |
| `cd proto && npx vitest run` | 138 | **142** (+4, tâche 7) |
| `cd client && npx vitest run` | 279 | **289** (+10, tâche 5) |

⚠️ **Ces comptes de départ ne sont pas ceux du plan** (550 / 52 / 70 / 187,
relevés le 19 août) : le dépôt a avancé entre l'écriture du plan et son
exécution. Les comptes ci-dessus sont ceux mesurés au démarrage de ce lot.

`cargo check --target x86_64-pc-windows-gnu` : **sortie 0**, 26
avertissements — 16 préexistants, plus **10 `dead_code` sur
`presse_papier.rs`**, qui n'a encore aucun appelant : les tâches 12 et 13 le
câblent.
