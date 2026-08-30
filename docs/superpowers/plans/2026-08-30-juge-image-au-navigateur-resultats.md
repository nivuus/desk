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
