# Sous-bloc D5 — le vivier d'encodeurs : résultats

**Date** : 3 août 2026
**Conception** : `docs/superpowers/specs/2026-08-02-multifenetres-vivier-encodeurs-design.md`
**Plan** : `docs/superpowers/plans/2026-08-02-multifenetres-vivier-encodeurs.md`
**Journaux** : `docs/superpowers/plans/journaux-multifenetres-d5/` — UTF-8, avec les
séquences ANSI de `tracing` (`sed 's/\x1b\[[0-9;]*m//g'` pour lire à plat).

---

## 1. La mesure pivot — verdict : `concurrence`

### 1.1 Ce qu'elle éprouvait

La conjecture ouverte depuis le 31 juillet 2026, et jamais jouée par aucun
chantier : **détruire un encodeur libère-t-il la place ?** Et la question que les
trois documents qui posent la première n'avaient jamais posée : le plafond de 8
porte-t-il sur la **concurrence** ou sur les **créations cumulées** ?

### 1.2 Le montage

`agent/src/diagnostics/multifenetre/recyclage.rs`, mode
`MULTIFENETRE_NVENC_CYCLES=10`. Un processus, **un périphérique D3D11 par
encodeur**, 1280×720 à 60 Hz et 8 Mb/s — les paramètres exacts de la seconde
recette de D4, pour que le chiffre soit opposable au sien.

Séquence : monter jusqu'au refus et le **nommer** ; puis, dix fois de suite,
détruire un encodeur et en construire un neuf.

### 1.3 Le relevé

**Quatre exécutions indépendantes**, horodatages de début et de fin tous
distincts, chacune complète :

| Journal | Refus du 9ᵉ | Cycles réussis | Verdict |
| --- | --- | --- | --- |
| `recyclage-1.log` | oui | **10 / 10** | `concurrence` |
| `recyclage-2.log` | oui | **10 / 10** | `concurrence` |
| `recyclage-3.log` | oui | **10 / 10** | `concurrence` |
| `recyclage-4.log` | oui | **10 / 10** | `concurrence` |

**Le témoin de phase 1 est exact et reproduit aux quatre exécutions** : le
neuvième encodeur est refusé à la **configuration du type de sortie** de
l'encodeur H.264 (transform matériel), en `0xC00D6D76`
(`MF_E_UNSUPPORTED_D3D_TYPE`) — **le même appel et le même code** que la mesure ②
du 31 juillet 2026 et que le défaut ouvert de D4. Sans ce témoin, la suite ne
prouverait rien : un 9ᵉ qui réussit après une destruction ne dirait rien s'il
réussissait déjà avant.

Puis, aux dix cycles : `relâchement d'un encodeur` → `relâchement terminé` →
`reconstruction RÉUSSIE`, ramenant à chaque fois le vivier à 8 vivants.

**Conclusion : détruire un encodeur libère la place, et le recyclage tient sur
dix cycles consécutifs.** Le plafond de 8 porte donc sur la **concurrence**, pas
sur les créations cumulées.

### 1.4 La ligne de la table de décision qui s'applique

La règle avait été écrite **avant** la mesure (conception §3.4). Le résultat
active sa première ligne, sans arbitrage :

> Le 9ᵉ réussit, et les dix cycles tiennent → **cible « dépasser 8 » maintenue.
> C2 se remédie en détruisant avant de construire.**

### 1.5 Ce que cette mesure N'établit PAS

- **La couche qui impose le plafond de 8 reste inconnue** — NVENC, pilote
  NVIDIA, Media Foundation, ou virtualisation. D5 la contourne, il ne l'explique
  pas. Question ouverte depuis le 30 juillet 2026.
- **Aucune image n'a été soumise.** Seules la construction et la destruction sont
  mesurées, pas la tenue en cadence.
- **Aucune duplication DXGI n'est ouverte par ce banc.** L'arrangement de
  production en tient N en plus des N encodeurs ; la réserve de « deux variables
  confondues » qui pesait sur la comparaison partagé/séparé du 31 juillet **n'est
  donc pas levée par cette mesure-ci**, elle est seulement contournée d'un autre
  côté.
- **Rien d'autres résolutions ni d'autres débits.**
- **Quatre exécutions à un seul rang de cycles (10).** Un plafond cumulé qui
  n'apparaîtrait qu'au-delà de dix recyclages ne serait pas vu par ce montage.

### 1.6 Un écart de protocole, et pourquoi il est dit ici

Une première tentative d'enchaîner trois exécutions a produit **deux journaux
identiques à la microseconde près**. L'attente cherchait la ligne de fin du banc
dans `agent.log` — **or elle y était déjà**, écrite par l'exécution précédente :
la boucle sortait aussitôt et recopiait l'ancien journal.

Corrigé en **supprimant `agent.log` avant chaque lancement**, puis vérifié par un
contrôle d'indépendance : horodatages de début **et** de fin tous distincts,
présence du refus de phase 1 et des dix cycles dans chacun.

C'est une variante du piège que `CLAUDE.md` documente déjà (« un journal d'agent
s'écrase facilement ») : ici il ne s'écrasait pas trop tôt, il **survivait** trop
longtemps. **Attendre un fait ne suffit pas quand ce fait peut être celui de la
mesure précédente : il faut d'abord détruire la trace de celle-ci.**

### 1.7 Fraîcheur du binaire mesuré

`scripts/build-agent.sh` a rendu `Finished release profile in 19.50s` — une
compilation réelle, et non le `0.13s` qui trahirait un binaire non rebâti.
Binaire : **9 129 984 octets**, daté de la compilation.

Contrôle supplémentaire, que les chantiers précédents n'avaient jamais versé :
`strings` sur le binaire rend **21** occurrences des chaînes propres au banc de
D5 (`recyclage`, `VERDICT`). La fraîcheur n'est donc pas adossée au seul
horodatage.
