# Porte P0 de G5 — VERDICT : **V1 REÇUE**

**Date** : 21 août 2026. **Navigateur** : `Chrome/151.0.7922.169`, `--headless=new`,
`--no-sandbox`, `--disable-gpu`, profil neuf par exécution.
**Objet mesuré** : le NAVIGATEUR. Aucun agent, aucune VM, aucune plateforme.
**Instrument** : `instrument/porte-p0.mjs` (+ `instrument/png.mjs`).
**Six sondes, DEUX exécutions chacune. Aucun taux n'est revendiqué** — les douze
exécutions rendent des relevés identiques, ce qui établit la reproductibilité et
rien de plus.

---

## Le relevé

| Sonde | Ce qu'elle pose | Manifeste chargé | `getAppManifest().errors` | `getInstallabilityErrors` | `beforeinstallprompt` | SW |
| --- | --- | --- | --- | --- | --- | --- |
| **b** *(jouée EN PREMIER)* | `blob:` + icône **128×128** | **`blob:…` OUI** | **0** | 🔴 **`manifest-missing-suitable-icon`, `no-acceptable-icon`** (min **144**) | **ne se déclenche PAS** | 0 |
| **a** | `blob:` + icône 512×512 | **`blob:…` OUI** | 0 | **[]** | **se déclenche** | 0 |
| **c** *(témoin HTTP)* | manifeste servi par HTTP | `http://…` | 0 | **[]** | **se déclenche** | 0 |
| **d** | `blob:` 512, sans service worker | `blob:…` | 0 | **[]** | **se déclenche** | **0** |
| **e** | `blob:` 512 | `blob:…` | 0 | **[]** | **se déclenche** | 0 |
| **f** | `blob:` 512, **URL relatives** | `blob:…` | **2** | 🔴 **`start-url-not-valid`** | **ne se déclenche PAS** | 0 |

---

## ① Le verdict, selon la table écrite d'avance au §3.4 du plan

**Le `blob:` est chargé, analysé et jugé installable par Chromium.** La ligne du
plan « P0-b remplit `errors`, **et** P0-a la laisse vide » est celle qui
s'applique — à la réserve de nommage ci-dessous près :

> **V1 REÇUE.** La tranche D se joue par V1. **AUCUNE décision de sécurité n'est
> demandée**, aucune route ne change, aucun contrôle de porteur ne saute.
> **G5 est livrable seul.**

🔵 **LE TÉMOIN P0-c EST CE QUI DONNE SON POIDS AU VERDICT** : le manifeste servi
par HTTP ordinaire et le manifeste `blob:` rendent **exactement le même relevé**,
sur les quatre colonnes. **Ce qui diffère entre les deux est donc : RIEN.** Le
`blob:` ne coûte pas une erreur, pas un avertissement, pas une capacité.

---

## ② 🔴 UN DÉFAUT DU PLAN, TROUVÉ PAR LA MESURE : `errors` N'EST PAS LE JUGE

Le plan (§9, critère ①) écrit : « **Jugé sur** `Page.getAppManifest()` : […] et
**la liste `errors` verbatim** », et sa table §3.4 fait dépendre tout le verdict
de « P0-b remplit `errors` ».

**MESURÉ : P0-b laisse `errors` VIDE, aux deux exécutions.** Une icône trop
petite **n'est pas une erreur d'analyse de manifeste** — c'est un refus
d'**installabilité**, et les deux listes de Chromium sont distinctes :

| Liste | Ce qu'elle contient | Se remplit-elle sous P0-b ? |
| --- | --- | --- |
| `Page.getAppManifest().errors` | les erreurs d'**ANALYSE** du document manifeste | ❌ **non** — 0, 2 exéc. |
| `Page.getInstallabilityErrors` | le verdict d'**INSTALLABILITÉ** | ✅ **oui** — 2 entrées, 2 exéc. |

⚠️ **`errors` n'est PAS inutile pour autant, et c'est P0-f qui l'établit** : sur
un manifeste à URL relatives, elle porte **deux** entrées verbatim —
`property 'start_url' ignored, URL is invalid.` et
`property 'scope' ignored, URL is invalid.` **Les deux listes mesurent deux
choses, et il faut les deux.**

🔴 **CORRECTION APPORTÉE AU PLAN** : le critère ① se juge sur
**`Page.getInstallabilityErrors`** (vide ⇔ installable), corroboré par
**`beforeinstallprompt`**, avec `getAppManifest()` — son `url`, son `data`, ses
`errors` — versé en entier. **Pris à la lettre, le plan aurait rendu le
critère ① NON MESURABLE**, ce qui est sa propre ligne « `errors` vide aux deux ».

---

## ③ 🔵 `beforeinstallprompt` SE DÉCLENCHE, contre le pronostic du plan

Le plan écrivait, §3.4 : « **P0-e ne se déclenche pas** — attendu, et sans
conséquence », et §9 : « aucune invite ne peut être montrée à personne ».

**La seconde moitié reste vraie ; la première est RÉFUTÉE.** L'événement se
déclenche en `--headless=new`, **aux dix exécutions où l'application est
installable, et à aucune des quatre où elle ne l'est pas** (P0-b et P0-f).

⚠️ **Ce que cela n'établit PAS** : que quiconque puisse *voir* ou *accepter* une
invite. L'hôte n'a ni `DISPLAY`, ni `Xvfb`, ni `xdotool` (mesuré par F1 et D8).
`beforeinstallprompt` dit que **Chromium tient l'application pour installable** ;
il ne dit rien de l'installation. **C'est un second témoin du même fait, pas un
fait de plus.**

---

## ④ 🔵 AUCUN SERVICE WORKER N'EST EXIGÉ — la décision 2.3 reste un CHOIX

P0-d : `navigator.serviceWorker.getRegistrations().length` vaut **0** aux douze
exécutions, et l'application est **installable quand même** —
`installabilityErrors` vide, `beforeinstallprompt` déclenché.

**La ligne du plan « P0-d exige un service worker → la décision 2.3 change de
nature » ne s'applique pas.** Le service worker reste **hors périmètre par
décision**, et non par improvisation. Legs inchangé, destinataire inchangé : le
chantier de retrait du legacy, verrou **C5**.

---

## ⑤ 🔴 DEUX CONTRAINTES DE V1, TROUVÉES PAR P0 ET QUE RIEN N'ANNONÇAIT

### (a) Sous un manifeste `blob:`, les URL doivent être ABSOLUES

**Le premier jet de l'instrument écrivait `start_url: '/p0.html…'`, `scope: '/'`**
— la forme de n'importe quel manifeste servi par HTTP. Chromium l'a refusée : la
base de résolution d'un manifeste est **son propre URL**, ici `blob:http://…`,
qui n'est pas une base utilisable. Journal du défaut versé :
`p0-0-instrument-defectueux-url-relatives.json`, et le cas est **rejouable** —
c'est la sonde **f**, conservée dans l'instrument pour cela.

**Conséquence pour la tranche D** : `manifeste.ts` compose ses `start_url`,
`scope` et `id` sur `location.origin`. **Ce n'est pas un détail d'écriture :
c'est la différence entre un manifeste installable et un manifeste refusé.**

### (b) Le seuil d'icône est **144**, et non 192

Chromium le NOMME dans son relevé : `minimum-icon-size-in-pixels: 144`. La
conception de ④ écrit « la servir en dessous de **192 px** » pour la ROUGE du
critère ①.

**Sans conséquence sur la décision D8** : l'icône témoin de 128×128 est sous les
deux seuils, donc la ROUGE reste valide — **mais un successeur qui poserait 160
en croyant être sous le seuil obtiendrait un VERT et lirait une rouge ratée
comme un produit correct.** Le nombre juste est **144**, mesuré, deux fois.

---

## ⑥ Ce que P0 n'établit PAS

- **Un seul navigateur** (Chrome 151), **sans interface**, **sans profil
  installé**. Rien de Firefox, de Safari, d'Edge, de ChromeOS.
- **Rien d'une PWA réellement installée** : ni son cycle de vie, ni la
  re-recherche de son manifeste **après** la mort de l'onglet qui a créé le
  `blob:`. **C'est le coût déclaré de V1 (plan D1), et il reste entier.**
- **Rien du produit** : ni jeton, ni route, ni icône réelle de la base. P0 mesure
  le navigateur ; c'est la recette qui mesurera le produit.
- **Rien de l'origine `https:`** : les sondes tournent en `http://127.0.0.1`,
  que Chromium traite comme un contexte sécurisé. Le déploiement réel est en
  `https:` derrière nginx — **plus favorable, mais non mesuré ici**.
