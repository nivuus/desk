# Lot 3, item 5 (3.3) — l'extinction du superviseur, et le compte d'orphelins

**Mesuré le 5 septembre 2026**, VM `Windows` (appliance), agent
`C:\nivuus\agent\agent.exe` (sha256 `3C270C6C…940C`, bit à bit identique au
produit de `scripts/build-agent-croise.sh` sur `ffdc839`).

⚠️ **Aucun chiffre de ce document ne se recopie sans relancer sa commande.**

---

## 1. Le chiffre, qui est le livrable

🔴 **UNE EXTINCTION PAR LE CHEMIN LIVRÉ, TROIS FENÊTRES OUVERTES, LAISSE
TROIS SORTIES VIRTUELLES ORPHELINES.** Deux exécutions, même chiffre :

| exécution | topologie « avant purge » | orphelins | après purge |
| --- | --- | --- | --- |
| **témoin, à froid, aucune fenêtre** | `nombre=1 attachees=1` | **0** | `nombre=1` |
| **1ʳᵉ, trois fenêtres** | `nombre=4 attachees=4` | **3** | `retirees=3 avant=4 apres=1` |
| **2ᵈᵉ, trois fenêtres** | `nombre=4 attachees=4` | **3** | `retirees=3 avant=4 apres=1` |

Les trois orphelines sont nommées dans le journal : `\\.\DISPLAY6`,
`\\.\DISPLAY7`, `\\.\DISPLAY8`, toutes sur `NVIDIA GeForce RTX 4070`,
`1860x1080`, `attachee=true`. La seule sortie légitime est `\\.\DISPLAY1`
(`Microsoft Basic Render Driver`, 1280×800).

🔴 **LE TÉMOIN EST DANS LE MÊME RELEVÉ, ET C'EST LUI QUI REND LE 3
INTERPRÉTABLE.** Un démarrage à froid rend **0** orphelin avec le même
instrument, la même commande, le même journal. Le compte n'est donc pas
structurellement non nul — et, symétriquement, le `0` du témoin n'est pas
structurellement nul, puisque les deux autres bras rendent 3.

## 2. L'instrument, et pourquoi c'est le produit qui mesure

Le compte n'est pas relevé par une sonde extérieure : **l'agent le publie
lui-même** à chaque démarrage, avant et après sa propre purge
(`agent::diagnostics::multifenetre::montee`, puis
`agent::moniteurs_virtuels::purge`). Le relevé consiste donc à repérer le
journal, éteindre, redémarrer, et lire ce que l'agent trouve.

⚠️ **Une condition qui n'est pas évidente et qui a coûté un bras** : sans
page-shell connectée, le superviseur **ne crée aucune sortie**. Trois
`Notepad` lancés par `POST /application/:id/lancer` sans navigateur ne
produisent que trois `raccourci lancé` et rien d'autre — mesuré. Le porteur de
session est donc obligatoire pour que l'item ait un objet.

## 3. Le chemin d'extinction, relevé et non supposé

- `grep -rn "SetConsoleCtrlHandler\|ctrlc\|set_handler\|signal_hook" agent/src/ | wc -l`
  → **0**. **Aucun gestionnaire de signal console n'existe.**
- `scripts/stop-agent.sh` fait `schtasks /end` puis
  `Stop-Process -Name agent -Force`, c'est-à-dire `TerminateProcess` : la pile
  **n'est pas déroulée**.
- Deux `impl Drop` existent et sont **structurellement inatteignables** par ce
  chemin : `agent/src/moniteurs_virtuels.rs:134` (`Sorties`, qui détruit les
  sorties virtuelles en ordre inverse) et
  `agent/src/superviseur/lanceur.rs:481` (`LanceurDeProcessus`).

🔴 **ET UN CONSTAT QUE LE CADRAGE N'AVAIT PAS : `scripts/stop-agent.sh` EST
LUI-MÊME INOPÉRANT AUJOURD'HUI.** Il passe par `node scripts/winrm.js`, dont
le transport Basic est refusé par l'invité depuis le basculement du 29 août
2026. Le « chemin d'extinction livré » n'est donc pas seulement sans
gestionnaire de signal : **il ne s'exécute plus du tout**. L'extinction
mesurée ici emprunte la même paire de gestes (`Stop-ScheduledTask` puis
`Stop-Process -Force`) par la voie WinRM qui, elle, répond.

## 4. Ce que l'extinction laisse ENCORE derrière elle

`"notepad restants : " + (@(Get-Process notepad …)).Count` → **3**.

Les trois applications **survivent** à la mort du superviseur. Elles sont donc
orphelines au sens du legs déjà inscrit (« un redémarrage de l'agent orpheline
toutes les fenêtres ouvertes ») — et, depuis la règle d'appartenance, elles ne
seront jamais réadoptées. **Les deux legs se composent** : l'extinction laisse
trois sorties virtuelles ET trois fenêtres qu'aucun redémarrage ne récupère.

## 5. Ce que cet item N'ÉTABLIT PAS

- ⚠️ **Aucun remède n'a été écrit.** Le plan (étape 5) laissait le choix
  d'ajouter un gestionnaire de signal console qui laisserait courir les `Drop`
  déjà écrits. **Ce lot mesure, il ne répare pas** : la décision appartient au
  propriétaire du dépôt, et le legs part chiffré (**3 sorties orphelines pour
  3 fenêtres**, deux exécutions).
- ⚠️ **La purge fonctionne, et ce n'est pas la question.** `retirees=3` aux
  deux exécutions : le mécanisme de rattrapage est sain. Ce que l'item mesure
  est ce que l'**extinction** laisse, pas ce que le **démarrage suivant**
  rattrape.
- ⚠️ **Le rapport « une fenêtre → une sortie » n'est pas éprouvé au-delà de 3.**
- ⚠️ **`ProjFS` n'est pas mesuré ici** : le plan nommait « sorties virtuelles,
  racines ProjFS, processus survivants » ; seuls les deux premiers termes
  (sorties, processus) portent un chiffre. Aucune racine ProjFS orpheline n'a
  été comptée — non mesuré, pas « zéro ».

## 6. La VM a-t-elle tenu ?

`etat-vm.sh` au début de la campagne : `extinctions=30`.
`etat-vm.sh` à la fin : `extinctions=30`. **Inchangé** — aucune mesure n'est à
cheval sur une extinction du domaine, aucune n'est à disqualifier.

Journal brut : `orphelins-20260905T0051.log`.
