# Lot 32C — Apollo assoupli, et le JUGE : une image de la VM arrive au navigateur

Date : 30 août 2026. Branche `package-nivuus`.

🔴 **LE CHIFFRE-JUGE EST TOMBÉ, VERT, DEUX FOIS** — `framesDecoded` côté
navigateur, avec son témoin négatif mesuré par le même instrument.

| Bras | source | `framesDecoded` Δ | cadence | `bytesVideo` Δ | `bytesAudio` Δ | ICE |
| --- | --- | --- | --- | --- | --- | --- |
| **verte 1** (`juge-9`) | mire animée | **494** / 25,0 s | **19,75 i/s** | 1 286 242 | 402 822 | connected |
| **verte 2** (`juge-10`) | mire animée | **484** / 25,0 s | **19,35 i/s** | 1 178 889 | ~345 000 | connected |
| **rouge 1** (`juge-6`) | fenêtre **statique** | **0** / 20,0 s | 0,00 | **0** | 1 613 | connected |
| **rouge 2** (`juge-8`) | fenêtre **statique** | **0** / 20,0 s | 0,00 | **0** | 3 384 | connected |

🔵 **Le zéro des rouges est INTERPRÉTABLE, et c'est ce qui compte** : dans le
**même relevé**, `bytesAudio` est **non nul** et ICE est `connected`. Le
transport marche, la `PeerConnection` est la même, la session est établie —
seule la **vidéo** est absente, parce que **la source ne change pas**. C'est le
piège que ce dépôt a écrit : *Desktop Duplication n'émet qu'au CHANGEMENT du
bureau*, et une mire immobile rendrait zéro sur les deux bras. Ici elle rend
zéro sur le bras statique **et 19 i/s sur le bras animé**.

⚠️ **La cadence mesurée (≈19 i/s) n'est pas celle de la mire (10 Hz)**, et je
ne la présente pas comme telle : l'encodeur est configuré à `fps=90` et émet
sur d'autres changements que les repeints de la mire (curseur, décorations).
Le juge est le **delta strictement positif**, pas son égalité à une cadence
attendue.

---

## 1. Apollo assoupli — et ce que le contrôle N'établit PAS

### 1.1 Les valeurs réellement acceptées, établies et non devinées

L'énumération vient de la **source de l'interface d'Apollo**
(`assets/web/assets/config-cb85bda1.js`), pas de ma mémoire :

```
disabled | verify_only | ensure_active | ensure_primary | ensure_only_display
```

et leur **effet** vient du fichier de localisation de l'application
(`assets/web/assets/locale/en.json`) :

| Valeur | Ce qu'elle FAIT |
| --- | --- |
| `verify_only` | « Verify that the display is enabled » — **n'active pas** |
| `ensure_active` | « Activate the display automatically » — **sans toucher aux autres** |
| `ensure_primary` | active **et** rend primaire |
| `ensure_only_display` | « **Deactivate other displays** and activate only the specified display » |

**Retenu : `ensure_active`**, et le choix se justifie par l'effet. `verify_only`
serait moins agressif encore, mais il **n'active pas** — or la configuration
de cette VM dit elle-même que « the HDMI dummy plug is physically removed » :
sans activation, Apollo n'aurait aucun affichage. `ensure_active` est donc **la
moins agressive qui garde Moonlight fonctionnel**, et c'est exactement le
comportement retiré : Apollo cesse de **désactiver** les écrans des autres.

### 1.2 🔴 Le témoin négatif, et ce qu'il détruit

**L'écho du journal d'Apollo NE PROUVE RIEN.** Éprouvé plutôt que supposé :

```
dd_configuration_option = valeur_inexistante_lot32
[20:04:30] Info: config: 'dd_configuration_option' = valeur_inexistante_lot32
```

Apollo **écho une valeur inconnue verbatim, sans un avertissement**, et le
service démarre quand même. La ligne `= ensure_active` du journal est donc la
**chaîne brute du fichier**, jamais la preuve d'un enum analysé. Sans ce
témoin, j'aurais présenté un écho comme une confirmation — la rouge vacueuse
une cinquième fois.

### 1.3 🔴 Ce que je n'ai PAS pu établir, et que je n'affirme pas

**Le contrôle comportemental n'a pas pu être exercé.** Il a été **tenté** :
notre agent tenant trois sorties virtuelles (UID263-265), j'ai redémarré
`ApolloService` avec `ensure_only_display` **en vigueur** — **nos sorties ont
survécu**. La politique n'est donc pas appliquée au démarrage du service, mais
**au démarrage d'une session de streaming**, que je ne peux pas déclencher sans
un client Moonlight.

Donc :

- 🔴 **que `ensure_active` empêche effectivement la désactivation n'est PAS
  mesuré** — cela repose sur la sémantique documentée par l'application
  elle-même (§ 1.1) ;
- 🔴 **que Moonlight fonctionne encore n'est PAS établi.** C'est le vrai risque
  de ce changement, et je n'ai pas de client. **Il faut que le propriétaire
  l'essaie** — c'est son client, il en a un sous la main.

⚠️ **La mesure du juge a été jouée avec Apollo LANCÉ MAIS SANS SESSION**
(`ApolloService Running`, processus `sunshine` vivant, aucun client connecté).
**Ce n'est pas la preuve de la coexistence sous charge**, et c'est dit ici pour
que l'ambiguïté de la manche précédente ne revienne pas.

---

## 2. L'instrument du juge, et les quatre défauts qu'il a fallu corriger

`docs/superpowers/plans/journaux-lot32c/instrument/pilote-juge.mjs`, dérivé du
pilote du lot 31. Il vise la **plateforme de PRODUCTION** (`pomerium`) et
obtient son jeton par `X-Pomerium-Claim-Email` depuis l'hôte de confiance —
donc **sans monter de plateforme de recette**, et sans qu'un humain franchisse
un OAuth. Le jeton n'est jamais journalisé.

🔴 **QUATRE DÉFAUTS DE L'INSTRUMENT, tous découverts par la mesure, aucun par
le raisonnement** — et chacun rendait un faux ROUGE :

1. **`window.__pc` N'EXISTE PAS.** Le pilote du lot 31 s'appuyait dessus ; ni
   `client/src/` ni le bundle servi par la production ne le posent (vérifié par
   `grep` sur les deux). Le pilote rendait « aucune page de session n'est
   apparue » **alors que les pages étaient là** — ce que le dump des cibles CDP
   a montré. **Ne jamais inférer une absence : la dire.**
2. **Fermer la pop-up et la rouvrir tue la session.** Ma première parade
   fermait la fenêtre de la shell pour rouvrir la même URL dans une cible où
   l'amorce court. L'agent voit alors partir son pair, démonte la session, et
   la page rouverte reste à **`ice=new`** — mesuré. La bonne parade est
   `Target.setAutoAttach` + `waitForDebuggerOnStart`, qui **fait pauser** chaque
   cible neuve avant son premier script : on y pose le crochet, on la relâche,
   **et on ne ferme rien**.
3. **L'URL portée par `Target.attachedToTarget` est celle d'AVANT navigation**
   (`about:blank`) : l'apparier à l'URL de session ne peut pas marcher. Il faut
   **demander** son adresse à chaque session attachée.
4. **`-WindowStyle Normal` crée DEUX fenêtres** — la console *et* le
   formulaire. La session mesurée tombait sur la console, statique : capture à
   `images=0`, vidéo à zéro. `-WindowStyle Hidden` a suffi. **Compter les
   fenêtres, jamais les lancements.**

⚠️ **La mire est un instrument neuf** (`journaux-lot32c/instrument/mire.ps1`) :
celle du lot 31 n'existait plus sur la VM. Elle anime à 10 Hz **et peint sa
propre cadence dans l'image** — une source qui ne déclare pas sa cadence fait
mesurer au pilote sa propre croyance.

---

## 3. Ce que le chemin produit, côté agent

Relevé dans `agent.log` pendant le bras vert, sur la session mesurée :

```
agent::capture: duplication de sortie établie desktop_width=1860 desktop_height=1080
agent::encode_nvenc::porte: porte NVENC : pilote compatible version_pilote="0xd1"
agent::encode_nvenc::session: session NVENC native initialisée (P1, ultra faible
    latence, CBR) largeur=780 hauteur=492 fps=90 debit_bps=12000000
agent::encode: encodeur NVENC natif retenu adaptateur=NVIDIA GeForce RTX 4070
agent::capteur::fenetre: cadence du capteur images=37 endormie=false cadence="3.7"
```

🔵 **Les deux remèdes courent ensemble** : la **désignation** du lot 32 (la
sortie virtuelle appariée par son identifiant de cible) et l'**encodeur NVENC
natif** du lot 31 — et une image arrive au navigateur au bout.

---

## 4. Ce que ce lot N'établit PAS

- 🔴 **que Moonlight fonctionne encore** (§ 1.3). **À faire essayer par le
  propriétaire.**
- 🔴 **que la coexistence tient sous session Apollo active** : la mesure a été
  jouée Apollo **lancé sans session**.
- 🔴 **que `ensure_active` empêche la désactivation** : sémantique documentée,
  comportement non mesuré — le redémarrage du service ne déclenche pas la
  politique.
- **aucun jugement de QUALITÉ d'image** : le juge dit que des images arrivent
  et à quelle cadence, **rien sur ce qu'elles montrent**. Personne n'a regardé.
- **aucune latence de bout en bout** — toujours jamais mesurée, depuis D1.
- ⚠️ **Le vivier de dix sorties s'épuise vite** : une exécution a rendu
  « plus aucune sortie virtuelle disponible » avec 17 fenêtres annoncées. Ce
  n'est pas un défaut neuf, mais la recette y bute.

---

## 5. L'état dans lequel la VM est rendue

| Ce qui a été touché | État final | Preuve |
| --- | --- | --- |
| `sunshine.conf` | **`ensure_active`** — la décision du propriétaire | `Compare-Object` contre la copie nommée : **une seule ligne** |
| `ApolloService` | **Running** | `Get-Service` |
| `agent.exe` | **l'original** | sha256 `7DB1C0FA…74C3` |
| `SORTIE_DESIGNEE` | retirée | 0 occurrence |
| Tâche `lot32c-mire` + `mire.ps1` | **supprimées** | `Get-ScheduledTask` = 0 |
| Agent | 3 processus vivants | `Get-Process agent` |
| VM | en exécution, WinRM répond | `virsh list --all` |
| Définition libvirt | **jamais touchée** — le VGA est resté | le retrait n'était pas reconduit |

⚠️ **Copie nommée conservée** :
`C:\Program Files\Apollo\config\sunshine.conf.copie-nommee-avant-lot32c`.
Revenir en arrière est une copie.

⚠️ **Deux coupures WinRM transitoires** ont eu lieu (30 s de délai dépassé,
retour en ~5 s), l'une après que j'ai tué `explorer` — geste trop brutal de ma
part, que je n'ai pas répété. La VM n'a jamais cessé de tourner
(`virsh list --all` à chaque fois), et le VGA étant resté en place, la console
VNC est demeurée disponible tout du long.

---

## 6. Le DÉPLOIEMENT, et le fait qui contredit le § 1

Autorisation du propriétaire, donnée en connaissance de cause.

### 6.1 Moonlight — réserve LEVÉE, et par qui

🔵 **Moonlight fonctionne après l'assouplissement.** ⚠️ **Établi par le
PROPRIÉTAIRE, avec son propre client — pas par une mesure de ce lot.** C'est
exactement la vérification que le § 1.3 disait ne pas pouvoir faire.

🔴 **Cela ne lève PAS l'autre réserve**, et la suite la transforme en constat.

### 6.2 🔴 LE FAIT NEUF : `ensure_active` NE SUFFIT PAS, et c'est mesuré

L'A/B, sur la production, même binaire, même viewport, même instrument, à deux
minutes d'intervalle :

| Apollo | ouvertes | refus | tenues | verdict |
| --- | --- | --- | --- | --- |
| **`ApolloService` Running** (`ensure_active`) | 9 | **7** | **0** | ROUGE |
| **`ApolloService` Stopped** | 6 | **0** | **4** | **VERT** |

Le motif des refus est mot pour mot celui du défaut :
`aucune sortie d'affichage ne peut servir cette fenêtre`, avec `designee=""` et
`candidates=[]` — la sortie créée n'entre jamais dans la topologie.

🔴 **ET AUCUN CLIENT N'ÉTAIT CONNECTÉ** : `Get-NetTCPConnection` sur le
processus `sunshine` rend **0 connexion établie**. Ce n'est donc pas la
politique de session (`ensure_only_display`) qui mord ici.

🔵 **Le mécanisme, lu dans le journal d'Apollo** : il **relance sa sonde
d'encodeur à chaque changement de topologie d'affichage** — `Starting async
encoder teardown`, `Active GPU has HAGS enabled`, `Display refresh rate`,
`Client dynamicRange` —, **à une cadence de 5 s exactement**, celle de
`LIMITE_RATTACHEMENT`. Chaque sortie que notre agent crée déclenche une sonde
d'Apollo, qui crée et détruit sa propre sortie virtuelle temporaire ; la nôtre
ne s'attache jamais.

**Conséquence, dite sans ménagement : l'assouplissement décidé par le
propriétaire N'ATTEINT PAS son but.** Passer de `ensure_only_display` à
`ensure_active` ne suffit pas à la coexistence — **et la conclusion du lot
précédent, qui imputait l'échec du lot 31 à `ensure_only_display`, était
INCOMPLÈTE** : la sonde d'encodeur d'Apollo suffit à elle seule, sans session
et sans cette politique.

⚠️ **Ce qui reste à décider, et qui appartient au propriétaire** : les deux ne
peuvent pas tourner en même temps en l'état. `disabled` reste à éprouver, et
rien ne dit qu'il suffira — la sonde d'encodeur n'est pas gouvernée par
`dd_configuration_option`.

### 6.3 Ce qui a été livré, et par quel chemin

| Geste | Fait |
| --- | --- |
| Fabrication | `scripts/build-agent-croise.sh` depuis l'arbre courant — jamais sur la VM |
| Dépôt | **par `hooks/agent_payload.py::deposer_agent_console()` lui-même**, chemin **lu** et non deviné : `<NIVUUS_PACKAGES_DIR>/console/guest/payload/agent/agent.exe` |
| Copie nommée | sha256 **avant** `63491776…21eea` ; **et le fichier est suivi par git** dans le dépôt `installer` (commit `639c3cd`, arbre propre) — le retour est un `git checkout` |
| Empreinte livrée | `f0ee4f1477c30a16…dcac2`, **identique à ma fabrication** (`cmp`) |
| Les deux remèdes dans le binaire | `sortie DESIGNEE…` **1**, `porte NVENC` **1**, `session NVENC native initialisée` **1** ; **témoin négatif** (`aucune sortie apparue…`, la formule d'avant) **0** ; chaîne préexistante **1** |
| VM | binaire déposé (`F0EE4F14…`), relancé **par la tâche `guacamole-agent`**, jamais par `scripts/run-agent.sh` |
| Session | **`C:\nivuus\state\agent-session.txt` = `1`** — la session 1, attestée par l'appliance elle-même |
| `install` | 🔴 **NON rejoué** — il refrapperait `PLATEFORME_SECRET_JETON` |

**Les deux remèdes tournent sur le binaire de production**, mesuré au bras vert
(Apollo arrêté) : **6 sorties créées, chemin ① = 6, chemin ② = 0, 0 refus.**
⚠️ Les traces `porte NVENC` sont à **0** dans ce relevé, et c'est **normal** :
l'instrument du lot 22 n'ouvre aucune page de session, donc aucun encodeur
n'est créé. Elles ont été relevées au § 3, sous le juge.

### 6.4 `AGENT_VM` / `AGENT_SECRET` — le legs est PÉRIMÉ pour cette machine

🔵 **Le couple ATTEINT l'agent réel**, mesuré **sur le processus vivant**, dans
les deux sens :

```
agent enrôlé auprès de la plateforme url="ws://192.168.3.1:3445/agent"
    prefixe=3sxuA9dd56NpVdHi37R86g            ← 1 occurrence
"AGENT_VM ou AGENT_SECRET absent…"            ← 0 occurrence
```

⚠️ **Correction à `main.rs` telle qu'on me l'a décrite : il ne REFUSE pas.**
`SourceIdentite::Aucune` émet un `warn!` et continue **sans canal** — « la
plateforme REFUSERA la poignée de main et aucune session ne s'établira ». La
distinction compte : l'agent démarre quand même, et seul le journal le dit.

🔴 **CE QUI RESTE VRAI, ET QU'IL FAUT PORTER AU PROPRIÉTAIRE.** Le couple est
posé **à la main** dans `C:\nivuus\agent\run-agent.ps1` **de cette machine**.
L'**asset livré par le package `console`** —
`console/guest/provision/assets/run-agent.ps1` — ne pose toujours que
`SIGNALING_URL`, `LOCAL_IP` et `RUST_LOG`. **Une installation NEUVE
retomberait donc dans le legs.**

**Ce qu'il faudrait écrire, et où** : dans
`console/guest/provision/assets/run-agent.ps1`, **avant** la ligne
`& 'C:\nivuus\agent\agent.exe'` (l'ordre est ce qui compte — une variable
posée après l'invocation n'atteint rien, ce lot l'a payé), deux lignes
`$env:AGENT_VM` et `$env:AGENT_SECRET` alimentées par ce que `desk activate`
écrit déjà dans `desk.env` côté hôte. **Non fait : autre package, autre
dépôt.**

### 6.5 ⑤ Le service de production

| Contrôle | Résultat |
| --- | --- |
| `desk-plateforme.service` | **active (running)** |
| `NRestarts` | **0** — il n'a pas redémarré à cause de moi |
| `GET http://192.168.3.1:3445/` | **200** |
| `GET …/shell.html` | **200** |

⚠️ **Je me suis arrêté avant `app.allanic.me`** : l'OAuth exige un humain, et
c'est le propriétaire qui le franchira. Tout ce qui précède est mesuré **en
local sur l'hôte de la plateforme**.

### 6.6 L'état rendu

Apollo **Running**, `dd_configuration_option = ensure_active`, `sunshine.conf`
à **une ligne** de sa copie nommée. Agent : le **binaire neuf**
(`F0EE4F14…`), 3 processus, session 1. Copie nommée de l'ancien conservée sur
la VM. VM en exécution, WinRM répond. **Définition libvirt jamais touchée.**

⚠️ **Une coupure WinRM de plus** (connexion refusée, ~75 s), pendant l'arrêt
d'Apollo. La VM n'a jamais cessé de tourner, et Apollo a été remis en marche
dès le retour.

---

## 7. Rendre `desk` tolérant — le remède, et **il n'a PAS été exercé**

Décision du propriétaire : encaisser, ni `disabled`, ni arrêter Apollo.

### 7.1 🔴 Le fait qui a commandé la conception — et il renverse la prémisse

Prémisse de départ : « notre limite d'attente tombe dans la fenêtre de
perturbation ». **Mesuré, c'est plus précis que cela.** Une sortie virtuelle
**tenue** et relevée à 1 Hz (`MULTIFENETRE_VDD_VEILLE=45`, Apollo en marche) :

```
seconde=1  attachees=2  presente=Some(true)   nom=\\.\DISPLAY6
seconde=2..45 : idem
```

**Elle s'attache en moins d'UNE seconde.** Et dans la même heure, sur la même
machine, deux autres sondes ont vu la sortie **ne pas s'attacher du tout** en
3 s (`parues=[]`, `0 sorties DXGI neuves`).

🔴 **L'attachement n'est donc pas LENT, il est INTERMITTENT.** Allonger
`LIMITE_RATTACHEMENT` n'aurait fait qu'attendre plus longtemps **dans la même
fenêtre de perturbation** — d'où le refus de toucher la constante, et le choix
de **plusieurs chances espacées**.

### 7.2 Les trois décisions, et leur raison

**③ La reprise se fait sur la MÊME sortie, jamais sur une neuve.** Détruire
puis recréer change la topologie, donc **redéclenche la sonde d'Apollo** : la
reprise nourrirait exactement ce qu'elle attend. Et c'est ce qui consomme le
vivier — le bras rouge portait **9 sorties créées pour 7 refus**. Le pilote a
déjà accepté la création ; il n'y a rien à refaire.

**② Bornée par construction, épuisement lisible.** `superviseur/reprise.rs` :
`TOURS = 3`, `REPIT = 1 s`. `apres_un_tour` rend `Renoncer` dès
`tour >= tours` **et** pour `tours == 0` — un nombre de tours nul ne doit pas
se lire « à l'infini ». Un tour perdu est un **`warn!`** (« on RÉESSAIE ») ; un
abandon est un **`error!`** portant `tours_epuises` : les deux ne se lisent
plus pareil.

**② bis — l'interaction avec la borne des 30 s, VÉRIFIÉE et non supposée.**
`DELAI_ATTENTE_VIEWPORT_MAX` ne filtre que `Etat::AttendLeViewport`
(`table/orphelines.rs`), or `creer_sortie` court en `Etat::AttendLaSortie`
(`table/attribution.rs:62`). **La reprise est hors de sa portée**, donc aucun
garde-fou voisin ne l'annule.

**⑤ Ce qu'elle coûte, dit et figé par un test.** `creer_sortie` court DANS la
boucle du superviseur, **mono-fil** : pendant l'attente, rien d'autre n'est
traité. Le pire cas passe de **5 s à 17 s** (`3 × 5 s + 2 × 1 s`), et c'est du
temps d'attente **pur** avant que l'utilisateur voie son refus quand la sortie
ne viendra jamais. Un test l'affirme (`le_pire_cas_est_borne_et_calculable`).

⚠️ `TOURS = 3` est **dérivé d'une mesure, pas calibré** : l'attachement réussit
en < 1 s, la perturbation d'Apollo revient à ~5 s ; trois tours espacés
couvrent plusieurs cycles. **Ce n'est pas la même chose qu'une constante
calibrée**, et ce dépôt n'en a aucune.

**④ Six tests d'hôte** figent la règle, dont le cas dégénéré (`tours = 0`) et
**le produit d'AVANT la reprise** (`tours = 1` → `Renoncer` immédiat) — ce
dernier est le témoin qui rend la règle discriminante.

### 7.3 🔴 CE QUE LA MESURE N'ÉTABLIT PAS : la reprise n'a JAMAIS TIRÉ

Trois bras joués, binaire `D34213D8…` :

| Bras | Apollo | verdict | tenues | **reprise déclenchée** | refus « ne peut servir » |
| --- | --- | --- | --- | --- | --- |
| 1 | Running, **silencieux** (0 ligne) | VERT | 6 | **0** | **0** |
| 2 | Running **et SONDANT** (96 lignes, 2 sondes) | VERT | 3 | **0** | **0** |
| 3 témoin | **Stopped** | VERT | 3 | **0** | **0** |

🔴 **Le compteur `on RÉESSAIE` est à ZÉRO dans les trois bras.** Le remède est
livré, borné, testé sur l'hôte — **et jamais exercé**. Aucune de ces vertes ne
prouve qu'il répare quoi que ce soit ; elles établissent une **non-régression**,
rien d'autre.

🔵 **Et le bras 2 est celui qui aurait dû rougir.** J'ai **provoqué**
délibérément la condition — redémarrage d'`ApolloService` pendant que le
pilote ouvrait ses fenêtres, Apollo confirmé sondant dans la même fenêtre de
temps — et le produit a servi ses fenêtres **du premier coup** : **21 sorties
créées, chemin ① = 21, zéro refus du motif du défaut**. La condition qui
échouait à 18:47 **ne s'est pas reproduite**.

**Conséquence, dite sans l'arrondir : je ne peux pas affirmer que ce remède
corrige le défaut mesuré.** Il est plausible, borné et sans régression ; il
n'est pas démontré.

### 7.4 🔵 Un défaut DISTINCT, celui-là bien mesuré : le vivier s'épuise

Les refus des bras 2 et 3 ne portent **jamais** le motif du défaut, mais
**`plus aucune sortie virtuelle disponible`** : **27 puis 33 fenêtres
annoncées** en une minute (dont des `DesktopWindowXamlSource` transitoires)
pour un vivier de **dix**. C'est aujourd'hui le refus DOMINANT, et il n'a rien
à voir avec Apollo. **Non corrigé, nommé.**

### 7.5 Le déploiement, et l'état rendu

| | |
| --- | --- |
| Payload `console` | `f0ee4f14…` → **`d34213d8…`**, déposé par `hooks/agent_payload.py`, **identique à ma fabrication** (`cmp`) |
| VM | même binaire, relancé par la tâche `guacamole-agent`, **session 1** |
| `install` | **non rejoué** |
| Apollo | **Running**, `ensure_active` |
| Dépôt `installer` | **seul `console/guest/payload/agent/agent.exe` modifié**, non commité (travail concurrent) — retour par `git checkout` |

⚠️ **Le binaire de production porte donc un remède non démontré.** Il est
strictement additif — il n'ajoute que des tours là où il y avait un refus
immédiat — et les trois bras ne montrent aucune régression. **Revenir en
arrière est un `git checkout` plus un redéploiement.**
