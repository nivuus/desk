# Lot 3 — la campagne sur la VM Windows : résultats **PARTIELS**

**5 septembre 2026.** Branche `campagne-vm-mesures`, quatre commits, de
`ffdc839` **exclu** à `75c6200`.

> 🔴 **CE DOCUMENT NE CLÔT PAS LE LOT.** Onze items sur douze portent un chiffre
> daté ; **un seul** n'a pas été joué. Écrire « lot 3 clos » serait faux, et le § 5
> nomme ce qui reste dû, item par item.
>
> ⚠️ **Aucun chiffre de ce document ne se recopie sans relancer sa commande.**

---

## 1. Ce qui a été levé avant de pouvoir mesurer quoi que ce soit

🔴 **LE PLAN ET SON HARNAIS DATENT DU 28 AOÛT 2026. LA VM A BASCULÉ LE
29 AOÛT.** Le chantier `package-nivuus` a fait de la cible une **appliance**, et
a retiré — délibérément — les trois appuis sur lesquels tout le lot repose.
Relevé le 5 septembre 2026, commande par commande :

| Appui du plan | État mesuré aujourd'hui |
| --- | --- |
| `/media/vm` (CIFS `//192.168.3.2/c`) | `mount \| grep media/vm` ne rend **rien** ; le répertoire est **vide**. `vm_prete` y attendait 300 s puis rendait 1 — **il ne pouvait plus rendre 0** |
| `C:\dev` | `Get-ChildItem C:\` ne le liste pas. Le binaire vit dans `C:\nivuus\agent\agent.exe`, son journal dans `C:\nivuus\agent.log` |
| `scripts/winrm.js` (Basic) | « Failed to process the request, status Code: ». La voie qui répond est `installer/console/guest/winrm_exec.py`, en **NTLM** |
| `scripts/run-agent.sh` | écrit dans `/media/vm/dev/run-agent.ps1` et lance `C:\dev\target\…\agent.exe` : **deux chemins qui n'existent plus** |
| `scripts/stop-agent.sh` | passe par `scripts/winrm.js` : **inopérant** |

`CLAUDE.md` énonçait ces faits et en tirait que **le lot 3 est SUSPENDU**.
La suspension a été levée par un harnais re-situé,
`journaux-lot3/instrument/harnais-appliance.sh`, **posé À CÔTÉ de `harnais.sh`
qui reste intact** — c'est une pièce datée du 28 août, et le sort de
`winrm.js`, `/media/vm` et `build-agent.sh` appartient au propriétaire du
dépôt, pas à ce lot.

🔵 **La rouge que la tâche 0 n'avait pas pu voir a été vue** — son commit disait
« `agent_absent` durci mais NON vu vert (WinRM refuse l'auth) » :

```
agent_absent  -> « 🔴 3 agent(s) survivant(s) », code=1     ROUGE
agent_arreter -> « agents apres arret : 0 »
agent_absent  -> « aucun agent survivant », code=0          VERTE
```

🔵 **Le binaire mesuré est celui du dépôt.** `scripts/build-agent-croise.sh`
sur `ffdc839` produit un `agent.exe` **bit à bit identique** à celui déployé :
sha256 `3C270C6C2C65D6DB7CCA261E42AFB57902B63D2A76DC2EEFC8C3185A6713940C`,
20 583 167 octets des deux côtés. Aucun binaire tiers n'a été réutilisé, et
rien n'a été redéployé.

## 2. Les onze items qui portent un chiffre

| Item | Verdict | Le chiffre |
| --- | --- | --- |
| **7 (3.1)** la latence capture → affichage | **méthode du plan RÉFUTÉE, substitut mesuré** | `captureTime` absent de **9 084 / 9 084** trames ; substitut **9,18 et 9,04 ms** au nominal, **42,27 et 39,72 ms** sous `netem adsl` |
| **5 (3.3)** l'extinction du superviseur | **mesuré, 2 exécutions** | **3 orphelins** pour 3 fenêtres ; **0** au témoin à froid ; purge `retirees=3 avant=4 apres=1` |
| **6** la file du capteur | **partiel** | `PROFONDEUR_MAX = 64` ; **0** refus pour **457** attachements |
| **9 (3.4)** les replis micro | **partiel, une affirmation réfutée** | repli `Introuvable` couru **455** fois ; `Ambigu` **inexerçable** (un seul endpoint actif) ; nominal **0** |
| **11 (3.9)** les applications non mesurées | **le « 71 » RÉFUTÉ** | **41** au catalogue, dont **34** `non-mesuree`, 6 `pixels:256`, 1 `pixels:48` |
| **12 (3.10)** le maillon fautif du `Resize` | **aucun maillon fautif aujourd'hui** | 3 viewports (1024×700, 1400×500, 640×900) servis **exactement**, jusqu'à `videoWidth×videoHeight` |
| **1 (3.7)** « temporaire + renommage » | **aucune perte, aucun temporaire** | 44→**70** o et 44→**96** o au poste local, **1** acquittement chacun ; ROUGE : **44 o inchangé, 0** acquittement |
| **3 (3.8)** le répertoire frère | **ne se reproduit plus** | `D sous-dossier` dans **5 listages / 5**, aux **trois** bras (cache armé ×2, `PONT_CACHE=0`) ; témoin négatif qui tire |
| **2 (3.6)** les trois murs | **deux confirmés, un DÉPLACÉ** | débit **32,82 Kio/s** / 78 s ; lecture **128 Kio OK, 192 ÉCHOUE** ; énumération **3 200 COMPLET** (F4 : échouant), 4 000 échoue |
| **8 (3.2)** propriétaire mono-fenêtre | **n'existe pas — cohérent avec le legs** | livré **3 msg + 1 accent** ; mono **0 + 0** ; rouge **0 + 0** avec ses deux traces de désarmement |
| **10 (3.5)** mort de capture audio | **2 causes tentées, aucune ne tue** | témoin : **15** fautes, **1** mort VUE, **1** reconstruction ; 3ᵉ cause non tentable (C7) |

Chaque item a son répertoire `journaux-lot3-<item>/`, avec son `verdict.md`,
ses journaux bruts **versionnés**, et son instrument quand il en a un.

## 3. 🔴 TROIS CONTRÔLES DU PLAN NE POUVAIENT PAS ÉCHOUER

C'est le constat le plus transférable de cette campagne : **le plan lui-même
était une source du patron que ce dépôt punit**.

1. **Item 11.** `grep -ac "NonMesuree" agent.log` rend 0 — et **ne peut rendre
   que 0**. `NonMesuree` est un variant d'énumération Rust, jamais une chaîne
   journalisée (34 occurrences dans `agent/` et `proto/`, **aucune** dans un
   `tracing::`) ; sur le fil, la sérialisation est `"source_max":"non-mesuree"`,
   figée par un test de `proto`. Le chiffre existe — dans le catalogue servi par
   la plateforme — mais pas là où le plan le cherchait.
2. **Item 6.** Le **TÉMOIN** prescrit, `grep -ac "attache au capteur"`, rend 0 :
   le produit écrit `"attaché au capteur"`, **avec l'accent**
   (`capteur/tube.rs:153`), et le motif juste rend **457**. Sans cette
   vérification, l'item se concluait sur « zéro refus, témoin zéro » —
   c'est-à-dire sur un zéro **strictement ininterprétable**.
3. **Item 7.** Le commentaire de sonde que le plan **dicte mot pour mot** est
   faux dans sa seconde moitié : le sender report RTCP alimente
   `remote-outbound-rtp.remoteTimestamp`, **jamais `metadata.captureTime`**,
   lequel vient de l'extension d'en-tête RTP `abs-capture-time` que l'agent ne
   négocie pas (`grep` : 0). L'agent fait bien ce que le plan lui prête —
   204 sender reports sur le fil, 63 lus par Chrome — et la mesure échoue quand
   même.

## 4. Ce que les mesures disent du § « Legs ouverts » de `CLAUDE.md`

**CONFIRMÉ :**
- « **LE CHEMIN D'EXTINCTION PROPRE DU SUPERVISEUR N'A JAMAIS ÉTÉ EXERCÉ** » —
  et il n'existe pas : **0** gestionnaire de signal, deux `Drop`
  inatteignables sur `TerminateProcess`. **Chiffré : 3 sorties orphelines.**
- « **LA LATENCE DE BOUT EN BOUT — jamais mesurée** » : elle ne l'est
  **toujours pas**. Ce lot mesure un segment, et dit lequel.
- « **un redémarrage de l'agent orpheline toutes les fenêtres** » : les
  3 `Notepad` survivent au superviseur, mesuré.
- « `MICRO_FAUTE_ECRITURE` **jamais armée** » : **0** occurrence — et ce lot
  ajoute qu'elle est **inarmable** ici, faute de câble.

**RÉFUTÉ :**
- 🔴 « **71 applications restent `NonMesuree`** » — **FAUX**. Le catalogue
  compte **41** entrées au total, dont **34** `non-mesuree`. Le nombre 71 n'a
  aucune source dans le produit d'aujourd'hui.
- 🔴 « **deux replis livrés et JAMAIS COURUS** » (chantier E) — **faux de l'un
  des deux** : le repli `Introuvable` court **455 fois**, une par session,
  depuis cinq jours, et se comporte exactement comme la spec l'exige.
- 🔴 « **le maillon fautif du `Resize` non identifié** » — sur le chemin
  mesuré aujourd'hui, **il n'y a pas de maillon fautif** : les cinq maillons
  passent la valeur intacte, à trois rapports d'aspect différents.
- ⚠️ « **le lot 3 est SUSPENDU** » (§ Cycle de vie de la VM) — la raison
  invoquée était le basculement de la VM ; elle est levée par le harnais
  re-situé.

**AFFINÉ :**
- ⚠️ Le legs d'appartenance du lot 32I dit qu'une application **du Windows
  Store** ne serait pas adoptée. Mesuré : **`Microsoft Edge` non plus**, alors
  que desk le lance lui-même (`raccourci lancé chemin="…Microsoft Edge.lnk"`,
  puis 60 ms plus tard `fenêtre ÉCARTÉE : desk ne l'a pas lancée`). Edge n'est
  pas une application du Store : **le legs est plus large que ce qui est écrit**.
- ⚠️ `CLAUDE.md` dit qu'un marqueur posé par `Add-Content` dans un journal tenu
  par un agent vivant est « **silencieusement perdu** ». Mesuré : il **lève**
  (`The process cannot access the file … because it is being used by another
  process`). Le remède reste le bon ; la description du symptôme ne l'est pas.

## 5. Ce qui reste dû — un item, et les moitiés manquantes des autres

| Item | Ce qui reste |
| --- | --- |
| **1 (3.7)** | **JOUÉ** — reste : VS Code, Word, LibreOffice, **aucun installé sur cette appliance** (dette C7) |
| **2 (3.6)** | **JOUÉ** — reste : encadrer le mur d'entrées entre 3 200 et 4 000, et la CAUSE de son déplacement |
| **3 (3.8)** | **JOUÉ** — ne se reproduit plus ; reste : le renommage de RÉPERTOIRE, et un jeu plus grand |
| **4** le rejeu des douze pilotes | **entier, non joué** |
| **8 (3.2)** | **JOUÉ** — reste : séparer « pas de propriétaire » de « pas de session », bloqué par le `0x8000FFFF` du lot 31 |
| **10 (3.5)** | **JOUÉ** — reste : la 3ᵉ cause (bascule du rendu par défaut), bloquée par **C7** |
| 6 | le bras **« boucher »** (`SuspendThread` sur une session) n'a pas été monté : le refus au palier, la progression en puissances de deux et `session_cible` ne sont **pas** mesurés |
| 9 | le cas **nominal** et le repli **`Ambigu`** sont inatteignables sur cette VM (pas de VB-Audio, un seul endpoint de rendu actif) |
| 11 | l'**icône mutée sans changement de raccourci** n'a pas été éprouvée ; la rouge **`APPS_SURVEILLANCE=0`** n'a pas été jouée (son témoin est pourtant non nul : 352 / 91) |
| 12 | la rouge prescrite — un `Resize` en mode **`SortieEntiere`** — n'a pas été jouée |
| 7 | la latence **verre à verre** reste hors d'atteinte ; `captureTime` en **multi-fenêtres** n'est pas mesuré |

## 6. Legs NEUFS, chiffrés

- 🔴 **`metadata.captureTime` est inatteignable tant que l'agent ne négocie pas
  l'extension d'en-tête RTP `abs-capture-time`.** Mesuré : 0 sur 9 084 trames,
  alors que les sender reports arrivent et sont lus. **Le remède est un
  changement de produit** (`agent/src/transport/`) — **non fait, décision du
  propriétaire du dépôt.**
- 🔴 **`scripts/stop-agent.sh` est inopérant** (transport Basic). Ce n'est pas
  seulement « le chemin d'extinction ne déroule pas les `Drop` » : **il ne
  s'exécute plus du tout**.
- 🔴 **Une extinction laisse 3 sorties virtuelles ET 3 fenêtres orphelines**
  pour 3 fenêtres ouvertes, deux exécutions.
- ⚠️ **`Microsoft Edge` n'est jamais adopté** (voir § 4).

## 7. Ce que ce lot n'établit PAS

- **Aucun jugement humain** n'a été porté : personne n'a regardé une image,
  personne n'a écouté un son.
- **Aucune constante n'a été calibrée.**
- **Aucun mur n'a été déplacé** : les trois murs du pont (débit, taille de
  lecture, rang d'énumération) n'ont même pas été re-situés.
- **Aucune ligne de produit n'a été modifiée.** Les quatre commits de cette
  branche ne touchent que `docs/` — vérifié :
  `git diff --name-only ffdc839..HEAD | grep -v '^docs/'` rend **vide**.
- **La suite de tests est verte et n'a rien à voir avec ces mesures** :
  665 tests `client/src`, 308 tests `proto/ts`, `tsc --noEmit` code 0.
- **`etat-vm.sh` rend `extinctions=30` au début ET à la fin** de la campagne :
  aucune mesure n'est à cheval sur une extinction du domaine.

## 8. 🔴 DEUX MESURES SONT BLOQUÉES PAR LE PROVISIONNEMENT, PAS PAR LE PRODUIT

L'invité porte `provision_version=B1`, `completed=2026-08-26T18:39:36` —
relevé dans `C:\nivuus\state\PROVISION.done`, que l'appliance écrit
elle-même. Le dépôt voisin déclare `PROVISION_VERSION = "B4"`
(`packages/installer/console/guest/payload.py:32`) : **trois versions
d'écart**, c'est-à-dire la dette **C7** de `nivuus/installer` — *« L'invité
tourne trois versions de provisionnement en retard, et rien ne le crie »*
(`docs/console-dettes.md` § C7), dont le remède est une reconstruction
complète de l'invité.

**Ce qui en découle, et qu'il faut imputer à C7 et non à `desk` :**

| Ce qui n'a pas pu être mesuré | Pourquoi |
| --- | --- |
| le cas **nominal** du micro (item 9) | VB-Audio absent — `Win32_SoundDevice` filtré sur `VB-Audio\|CABLE` rend **0** |
| le repli **`Ambigu`** du micro (item 9) | un seul point de terminaison de rendu **actif** |
| `MICRO_FAUTE_ECRITURE` (item 9) | injecte des fautes d'écriture **sur le câble**, qui n'existe pas |

🔴 **Ne pas rejouer ces bras avant que C7 soit levée** : ils rendront le même
résultat, pour la même raison, et ce ne sera toujours pas une mesure de `desk`.
Sans cette section, quelqu'un les rejouera indéfiniment en croyant que le
produit est en cause.
