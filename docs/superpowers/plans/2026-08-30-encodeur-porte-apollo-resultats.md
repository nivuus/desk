# Lot 31 — par quelle porte Apollo obtient ses encodeurs, et laquelle reste ouverte à `desk`

**30 août 2026.** Dépôt `desk`, branche `package-nivuus`. VM `Windows`
(libvirt), **définition inchangée, VGA QEMU en place** — c'est la condition
dans laquelle Apollo réussit, donc la condition de toute mesure de ce lot.

> 🔴 **CE DOCUMENT NE MODIFIE AUCUN CODE DE PRODUIT.** Les remèdes sont
> **nommés avec leur `fichier:ligne`** et laissés à la décision du
> propriétaire du dépôt. Les seuls artefacts écrits sont des **sondes
> jetables** (`/var/tmp/lot31/`, et `C:\nivuus\lot31\` sur la VM).

---

## 0. Le résultat, et l'état LIVRÉ

**L'enquête.** Apollo n'empruntait **pas** la porte que `desk` empruntait : il
appelle l'**API NVENC native** (`nvEncodeAPI64.dll`), là où `desk` passait par
la **MFT Media Foundation de NVIDIA** — et cette MFT refuse de s'activer en
**session 1** sur cette machine (`0x8000FFFF`) alors qu'elle s'active en
**session 0**, dans le même binaire, à la même minute, sur les quatre
arrangements que l'API Media Foundation permet d'essayer.

**Ce qui est livré.** `desk` emprunte désormais **la même porte qu'Apollo**
quand un adaptateur NVIDIA est présent, et garde la MFT comme dos
**générique** (Intel Quick Sync, AMD VCE, session 0). Mesuré sur la VM le
30 août 2026, au même point de code et sur le même périphérique de capture que
l'échec du lot 30 : **1195 unités d'accès en 10 s**, contre **aucune**.

> 🟢 **LE CHIFFRE-JUGE A ÉTÉ RELEVÉ — PAR LE LOT 32, PAS PAR CELUI-CI.**
> `framesDecoded` **+494** et **+484** sur 25 s (deux exécutions, mire
> animée), contre **0** sur une source **statique** dont l'audio coulait
> pourtant et dont l'ICE était `connected` dans le même relevé. **Des images
> produites par ce chemin natif traversent jusqu'à un navigateur.**
> 🔴 **L'ATTRIBUTION EST LA MOITIÉ DU FAIT** : cette mesure a été montée et
> jouée par le **lot 32**, avec **DEUX** remèdes en place — la porte NVENC de
> ce lot-ci *et* la désignation de sortie du sien. Elle n'est pas une mesure
> du lot 31, et l'écrire autrement serait fabriquer une pièce. Voir § 11.2.
>
> 🔴 **CE QUE CE DOCUMENT N'ÉTABLIT TOUJOURS PAS, ET QU'AUCUNE LIGNE NE DOIT
> LAISSER CROIRE.** Le plafond d'encodeurs **à N fenêtres** n'est **pas**
> mesuré : le banc n'en ouvre qu'un (protocole § 13, **préparé et non joué**).
> Et **personne n'a regardé une image** — `verdicts_faux=0` dit qu'aucun
> verdict n'est faux, **pas qu'une image est juste**, et `framesDecoded`
> compte des images décodées sans rien dire de la **justesse** de ce qui
> s'affiche.

> ⚠️ **Ce document est chronologique.** Les §§ 1 à 10 sont l'enquête et la
> conception, écrites AVANT la mesure ; les §§ 11 et suivants sont l'état
> livré. Là où une section antérieure a été dépassée, elle le dit sur place
> plutôt que d'être réécrite — ce dépôt préfère un relevé daté et annoté à une
> histoire lissée.

---

## 1. Ce qui a été MESURÉ

### 1.1 L'instrument

Une sonde jetable, `sonde-mft.exe`, écrite pour ce lot, **hors du dépôt**
(`/var/tmp/lot31/sonde-mft/`, crate autonome, `windows` 0.62, bâtie en croisé
mingw `x86_64-pc-windows-gnu`). Elle n'écrit rien : elle énumère, active,
relâche, imprime. Elle **imprime sa propre session** (`ProcessIdToSessionId`)
plutôt que de la supposer.

⚠️ **Un piège payé, et consigné parce qu'il se repaiera** : la première
version liait `MFTEnum2` **statiquement** (`windows_core::link!`). Le symbole
n'étant pas celui que je croyais, le binaire **ne se chargeait plus du tout**
— `LastExitCode = -1073741511` (`0xC0000139`, `STATUS_ENTRYPOINT_NOT_FOUND`),
avant la première ligne de `main`. Le symptôme se lit comme un plantage de la
sonde, pas comme un import manquant. La version qui a mesuré résout
`MFTEnum2` par `GetProcAddress`.

Exécution en **session 1** par une tâche planifiée `Register-ScheduledTask`
`-LogonType Interactive` (pas de mot de passe sur l'argv, à la différence du
`schtasks /rp` de `scripts/run-agent.sh:160`), et en **session 0** par WinRM
NTLM (`installer/console/guest/winrm_exec.py`).

### 1.2 La matrice, même binaire, deux sessions

Relevés bruts archivés sur la VM : `C:\nivuus\lot31\lot31-mft-s0.txt` et
`C:\nivuus\lot31\lot31-mft-s1.txt`.

| Épreuve | Ce qu'elle fait | **session 0** | **session 1** |
| --- | --- | --- | --- |
| **A** | `MFTEnumEx(VIDEO_ENCODER, HARDWARE\|SORTANDFILTER, NV12→H264)` puis `ActivateObject` — **exactement ce que `desk` fait** | **OK**, `async=1` | **ÉCHEC `0x8000FFFF`** |
| **B** | idem, `HARDWARE` seul (sans `SORTANDFILTER`) | **OK** | **ÉCHEC `0x8000FFFF`** |
| **C** | idem A, mais énumération **bornée à l'adaptateur NVIDIA** par `MFT_ENUM_ADAPTER_LUID` (via `MFTEnum2`) | **OK** | **ÉCHEC `0x8000FFFF`** |
| **D** | idem A, mais **un périphérique D3D11 NVIDIA vivant** au moment de l'activation | **OK** | **ÉCHEC `0x8000FFFF`** |
| **E** | 🔵 **TÉMOIN** — encodeur H.264 **LOGICIEL** (`MFT_ENUM_FLAG_SYNCMFT`) | **OK**, `async=0` | **OK**, `async=0` |
| **F** | convertisseur de couleur **MATÉRIEL** (`VIDEO_PROCESSOR`, ARGB32→NV12) | **0 énuméré** | **0 énuméré** |
| **G** | 🔵 **TÉMOIN** — convertisseur de couleur **LOGICIEL** | **OK** | **OK** |
| **H** | `LoadLibrary("nvEncodeAPI64.dll")` + `GetProcAddress("NvEncodeAPICreateInstance")` | **OK, présent** | **OK, présent** |

🔴 **C'EST LE TÉMOIN E QUI DONNE SON SENS AU ROUGE.** Sans lui, « `0x8000FFFF`
en session 1 » serait indiscernable d'une session 1 où **toute** activation de
MFT échouerait. Le même processus, dans la même exécution, active sans peine
la MFT H.264 **logicielle** et la MFT de traitement vidéo **logicielle** : la
machinerie COM/Media Foundation fonctionne en session 1. **Seule la MFT
matérielle NVIDIA refuse.**

Dans les deux sessions, une **seule** MFT H.264 matérielle est énumérée :

```
[0] nom="NVIDIA H.264 Encoder MFT" url="NVIDIA H.264 Encoder MFT"
    luid=<absent HRESULT(0xC00D36E6)> vendeur="VEN_10DE"
```

⚠️ **`MFT_ENUM_ADAPTER_LUID` n'est pas LISIBLE sur l'activateur**
(`MF_E_ATTRIBUTENOTFOUND`, `0xC00D36E6`) — il est seulement **posable** en
entrée d'énumération. Une lecture de code qui espérerait s'en servir pour
« voir sur quel adaptateur la MFT est branchée » ne trouverait rien.

### 1.3 La topologie DXGI, et pourquoi elle ne suffit pas à expliquer

Session 1, au moment de la mesure (aucune sortie virtuelle vivante) :

```
[0] Microsoft Basic Render Driver vendeur=0x1414 luid=00000000:0000753D
      sortie[0] \\.\DISPLAY1 bureau=true
[1] NVIDIA GeForce RTX 4070 vendeur=0x10DE luid=00000000:000076D7
[2] NVIDIA GeForce RTX 4070 vendeur=0x10DE luid=00000000:000103A0
[3] Microsoft Basic Render Driver vendeur=0x1414 luid=00000000:000075E4
```

En session 0, les **mêmes quatre adaptateurs**, **aucune sortie** sur aucun.

🔴 **ET VOICI LA RÉFUTATION QUI COMPTE, parce qu'elle tue l'explication la
plus séduisante.** L'hypothèse naturelle — « la MFT NVIDIA refuse parce que le
GPU NVIDIA ne pilote aucun affichage » — est **fausse telle quelle** : le
journal du produit, `C:\nivuus\agent.log`, montre qu'à **10:21:04 aujourd'hui**,
en session 1, `desk` capturait

```
agent::capture::ouverture: sortie retenue pour la duplication
  adaptateur=NVIDIA GeForce RTX 4070 index_adaptateur=1
  index_sortie=0 nom_sortie=\\.\DISPLAY6 attachee=true
```

— c'est-à-dire une sortie **attachée au bureau** et **portée par l'adaptateur
NVIDIA** — et la ligne suivante est `encodeur matériel retenu
encodeur=NVIDIA H.264 Encoder MFT`, immédiatement suivie du chemin d'erreur.
**Le GPU NVIDIA pilotait un affichage attaché, et l'activation a échoué quand
même.**

🔵 **Conséquence de conception, non triviale** : les sorties virtuelles de
`desk` (SudoVDA) sont **déjà** rattachées par DXGI à l'adaptateur NVIDIA. Le
`IOCTL_SET_RENDER_ADAPTER` que `agent/src/moniteurs_virtuels/sudovda.rs:33-37`
déclare volontairement ne pas employer — et qui est **exactement** le levier
d'Apollo (`virtual_display.cpp::setRenderAdapterByName`) — **n'a rien à
corriger ici** : l'effet qu'il produirait est déjà obtenu. C'est une piste
tentante, et elle est **refermée par la mesure**.

Ce qui **reste** différent entre les deux mondes : l'affichage **PRIMAIRE**.
Il est resté `\\.\DISPLAY1` (VGA QEMU) dans tous les relevés. Apollo, lui,
pose `dd_configuration_option = ensure_only_display`, qui **désactive tous les
autres affichages**. ⚠️ **Cette différence n'est PAS mesurée comme cause** :
le lot 28 a déjà relevé (`C:\nivuus\lot28\primaire.log`) qu'un
`ChangeDisplaySettingsEx` avec `CDS_SET_PRIMARY` vers une sortie virtuelle
**n'a pas pris** (`BASCULE_EFFECTIVE=False`), et personne n'a joué la variante
« plus qu'un seul affichage ». Elle entre par ailleurs en conflit direct avec
la consigne qui gouverne ce lot : **le VGA QEMU doit rester**.

### 1.4 Apollo, mesuré sur la machine qui marche

**Ses modules, processus vivant** (`sunshine.exe`, **PID 7304, session 1**,
`Get-Process -Id 7304 | %{$_.Modules}`, 79 modules) :

- **présents** : `nvapi64.dll`, `nvapi64_impl.dll`, `nvcuda.dll`,
  `nvcuda64.dll`, `nvdxgdmal64.dll`, `nvobjectloader64.dll`, `nvppex.dll`,
  `d3d11.dll`, `dxgi.dll`, `dxcore.dll` ;
- 🔴 **ABSENTS, et c'est le fait** : `mfplat.dll`, `mfreadwrite.dll`,
  `mfcore.dll`, `nvEncMFTH264x.dll` — **aucun module Media Foundation, du
  tout**.

⚠️ **Ce relevé est au repos, et il faut dire ce qu'il vaut et ce qu'il ne vaut
pas.** Il ne vaut pas « pendant une session Moonlight » — voir §4. Mais il
n'est pas « avant tout encodage » non plus : ce processus, démarré à
**17:13:12**, avait **déjà créé et détruit six encodeurs NVENC** à
**17:13:23–17:13:24** (son journal, ci-dessous), et **quatorze minutes plus
tard il ne portait toujours aucun module Media Foundation**. Un processus qui
avait fabriqué des encodeurs n'a jamais chargé `mfplat.dll` : c'est une preuve
d'absence, pas une absence de preuve.

⚠️ `nvEncodeAPI64.dll` n'est **pas** dans la liste au repos non plus. C'est
cohérent avec un `LoadLibraryEx` / `FreeLibrary` autour de la fabrication des
encodeurs (le code amont le fait), mais **je ne l'ai pas mesuré chargé** :
c'est une déduction, elle est marquée comme telle en §3.

**Son journal**, `C:\Program Files\Apollo\config\sunshine.log`, processus
courant, **17:13:23–17:13:24** :

```
Info: config: 'adapter_name' = NVIDIA GeForce RTX 4070
Info: Creating a temporary virtual display to probe for encoders...
Info: Trying encoder [nvenc]
Device Description : NVIDIA GeForce RTX 4070
Device Vendor ID   : 0x000010DE
Device Device ID   : 0x00002786
Device Video Mem   : 12012 MiB
Info: Creating encoder [h264_nvenc]
Info: NvEnc: created encoder H.264 P1 async two-pass rfi
Info: NvEnc: created encoder HEVC P1 async two-pass rfi
Info: NvEnc: created encoder AV1 P1 async two-pass rfi
Info: NvEnc: created encoder H.264 P1 async yuv444 two-pass rfi
Error: NvEnc: gpu doesn't support YUV444 encode
Error: NvEnc: NvEncUnregisterAsyncEvent() failed: NV_ENC_ERR_DEVICE_NOT_EXIST
Info: Found H.264 encoder: h264_nvenc [nvenc]
```

🔴 **`NvEnc:` et `NvEncUnregisterAsyncEvent()` sont des noms de l'API NVENC
NATIVE**, pas de Media Foundation. Et **`h264_nvenc` n'est PAS un nom de codec
ffmpeg ici** : sur Windows, cette chaîne n'est qu'un identifiant passé à
`is_codec_supported`, la fabrication passant par
`display_vram.cpp::make_nvenc_encode_device` — jamais par
`avcodec_find_encoder_by_name`.

**Sa configuration**, `C:\Program Files\Apollo\config\sunshine.conf`
(Apollo 0.4.6, relevée intégralement) — les clés qui décident :

| Clé | Valeur relevée | Ce qu'elle fait |
| --- | --- | --- |
| `adapter_name` | `NVIDIA GeForce RTX 4070` | 🔴 **le cœur** — filtre l'adaptateur DXGI **par sa `Description` exacte** |
| `dd_configuration_option` | `ensure_only_display` | désactive tous les autres affichages pendant la session |
| `isolated_virtual_display_option` | `disabled` | — |
| `dd_hdr_option` | `auto` | — |
| `dd_config_revert_delay` | `3000` | — |
| `sunshine_name` | `Nivuus` | — |
| `native_pen_touch`, `gamepad` | `enabled`, `x360` | — |
| `encoder` | **absente** | sonde dans l'ordre : `nvenc` d'abord |

Aucune clé de choix d'encodeur : c'est la sonde de démarrage qui tranche, et
elle a tranché `nvenc` — l'API native — sur les trois codecs.

---

## 2. Ce qui a été ÉTABLI PAR LECTURE DE SOURCE AMONT

Sources lues à `LizardByte/Sunshine@5cbb44d3` et `ClassicOldSong/Apollo@adc5c5a0`.
**Ce n'est pas une mesure sur cette machine** — c'est une lecture, elle est
dite comme telle, et elle est **corroborée** par le journal du §1.4.

1. **`MFTEnumEx`, `IMFActivate`, `ActivateObject`, `mfplat`, `nvEncMFT`
   n'apparaissent NULLE PART** dans `src/` des deux dépôts. Sunshine ne touche
   jamais l'API Media Foundation lui-même.
2. **La porte est `nvEncodeAPI64.dll`**, chargée par
   `src/nvenc/nvenc_dynamic_factory.cpp:21,49`
   (`LoadLibraryEx(nvenc_dll_name, nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32)`),
   puis `GetProcAddress("NvEncodeAPICreateInstance")`
   (`src/nvenc/nvenc_d3d11.cpp:30-32`), puis `nvEncOpenEncodeSessionEx` avec
   `NV_ENC_DEVICE_TYPE_DIRECTX` (`src/nvenc/nvenc_base.cpp:592-597`). Le
   chemin natif est arrivé dans Sunshine 0.21.0 (commit `68fa43a61c`,
   25 avril 2023) ; avant, c'était ffmpeg `h264_nvenc`.
3. **L'encodeur tourne TOUJOURS sur l'adaptateur de CAPTURE.** `adapter_name`
   filtre l'adaptateur dans la boucle de recherche de sortie
   (`display_base.cpp:488-541`), et `make_nvenc_encode_device`
   (`display_vram.cpp:2133`) crée un **second** périphérique D3D11 **sur ce
   même adaptateur**. ⚠️ **`D3D11_RESOURCE_MISC_SHARED_CROSSADAPTER`
   n'apparaît nulle part** : il n'y a **aucun** pont entre deux GPU.
4. **Apollo ne déplace pas l'encodeur vers le GPU NVIDIA : il y déplace
   l'AFFICHAGE.** `virtual_display.cpp:624-652::setRenderAdapterByName` envoie
   un IOCTL au pilote SudoVDA pour lier l'affichage virtuel à un GPU **par
   LUID**, appelé depuis `main.cpp:370` **avant** la sonde d'encodeurs.
   🔵 **C'est le levier — et le §1.3 montre qu'il est déjà en place chez
   `desk`, sans que cela suffise.**
5. **Rien dans Sunshine ne choisit un adaptateur par identifiant de vendeur.**
   `0x10de` n'y apparaît que deux fois, et les deux sont des **garde-fous**
   réagissant à un adaptateur déjà choisi, jamais des sélecteurs.
6. **Ce que tout le monde fait** : Sunshine, Apollo, Parsec → NVENC natif ;
   OBS a **retiré** son implémentation Media Foundation de NVENC après 0.14.0 ;
   `h264_mf` de ffmpeg n'a **aucun** code spécifique NVIDIA. Chromium emploie
   bien la MFT, mais avec des listes d'exclusion NVIDIA et une boucle
   d'activation qui **s'attend à ce que certaines échouent**.

**Le seul témoignage public qui décrit exactement notre panne** est un billet
de blog de Roman Ryltsov (2018, `alax.info/blog/1830`) : la MFT H.264 de
NVIDIA rend `0x8000FFFF` « quand l'affichage principal n'est pas celui qui est
connecté à l'adaptateur NVIDIA ». ⚠️ **C'est un blog, pas une position de
l'éditeur** ; le fil NVIDIA d'avril 2025 qui rapporte le même `E_UNEXPECTED`
sur un GPU portable est **resté sans réponse**. **Aucune documentation NVIDIA
ou Microsoft n'explique ce code de retour.** Et notre §1.3 le contredit
partiellement : chez nous le GPU NVIDIA pilotait **une** sortie attachée, sans
que cela suffise.

---

## 3. Les cinq réponses

**① Quelles DLL `sunshine.exe` charge-t-il ?** — **MESURÉ, avec une réserve
nommée.** En session 1 : `nvapi64`, `nvcuda`, `nvcuda64`, `nvdxgdmal64`,
`nvobjectloader64`, `nvppex`, `d3d11`, `dxgi`, `dxcore`. **Aucun module Media
Foundation**, quatorze minutes après avoir fabriqué six encodeurs NVENC.
`nvEncodeAPI64.dll` n'y figure pas au repos — **déduit** (non mesuré) : chargé
puis relâché autour de la fabrication, ce que fait le code amont. ⚠️ **Le
relevé « pendant une session Moonlight active » n'a PAS été obtenu** — §4.

**② Quel adaptateur, et comment ?** — **MESURÉ (sa configuration et son
journal) et LU (sa source).** `adapter_name = NVIDIA GeForce RTX 4070`, filtre
par `Description` **exacte**. Apollo choisit donc l'adaptateur NVIDIA
**explicitement**, mais **pas indépendamment du primaire** : il choisit un
adaptateur qui porte une sortie utilisable, et il **fabrique** cette sortie —
un affichage virtuel SudoVDA lié au GPU NVIDIA par LUID — puis désactive les
autres (`ensure_only_display`).

**③ Sa configuration ?** — **RELEVÉE**, tableau du §1.4. Aucun choix
d'encodeur ; le choix d'adaptateur est là, et c'est le seul qui compte.

**④ La porte de `desk`, et ce qu'elle suppose** — **LUE, et éprouvée** :

| Où | Quoi |
| --- | --- |
| `agent/src/encode/fabrique.rs:292` | `find_hardware_encoder()` |
| `agent/src/encode/fabrique.rs:306` | `MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE \| MFT_ENUM_FLAG_SORTANDFILTER, NV12 → H264)` — **aucun CLSID n'est nommé** : c'est une énumération, et elle rend **une** entrée, `NVIDIA H.264 Encoder MFT` |
| `agent/src/encode/fabrique.rs:363` | 🔴 **LA PORTE** : `first.ActivateObject()` → `0x8000FFFF` en session 1 |
| `agent/src/encode/mft.rs:119` | l'appel, depuis `H264Encoder::new` |
| `agent/src/encode/mft.rs:123-125` | `bail!` si la MFT n'est **pas asynchrone** — ce qui **exclut d'office** la MFT logicielle (épreuve E, `async=0`) |
| `agent/src/encode/mft.rs:136-143` | `share_device` + `MFT_MESSAGE_SET_D3D_MANAGER` — **APRÈS** l'activation |
| `agent/src/capture/ouverture.rs:125-131` | le périphérique D3D11, créé **sur l'adaptateur qui porte la sortie capturée** |

**Ce que cette porte suppose et que l'autre ne suppose pas** : que le pilote
NVIDIA veuille bien instancier **son objet COM d'encodeur enregistré dans le
magasin Media Foundation**, dans le contexte de session/station où on le lui
demande. La porte native ne suppose que `LoadLibrary` + `GetProcAddress` +
un périphérique D3D11 — **mesuré disponible en session 1** (épreuve H).

🔴 **Et une supposition tombe, mesurée** : `MFT_MESSAGE_SET_D3D_MANAGER` ne
peut **pas** être en cause. La documentation Microsoft l'impose *après*
l'activation et *avant* `SetInputType`/`SetOutputType`, et `encode/mft.rs:136-143`
le fait bien après `encode/mft.rs:119`. On échoue **avant** d'avoir la moindre
occasion de désigner un périphérique.

🔴 **TROUVAILLE COLLATÉRALE, NON CHERCHÉE, ET QUI DÉBORDE LARGEMENT CE
LOT — elle ne parle pas d'un chemin hypothétique, elle parle de la
PRODUCTION D'AUJOURD'HUI.** L'épreuve F montre qu'**aucun convertisseur de
couleur MATÉRIEL n'est énuméré** sur cette machine, **dans les deux
sessions**. Donc `find_hardware_video_processor()` (`encode/fabrique.rs:206`) échoue
**toujours**, et `create_color_converter` (`encode/fabrique.rs:112`) retombe
**toujours** sur son `CoCreateInstance(&CLSID_VideoProcessorMFT)`
(`encode/fabrique.rs:129`) — c'est-à-dire sur le **`Microsoft Video Processor MFT`,
LOGICIEL** (épreuve G, activé OK).

**Ce que cela veut dire, en clair : la conversion BGRA → NV12 de chaque
image de chaque fenêtre passe par le CPU, sur cette VM, depuis toujours — et
personne dans ce dépôt ne le savait.** Le commentaire d'en-tête d'`encode.rs`
décrit ce convertisseur comme celui qui « convertit BGRA→NV12 **sans quitter
le GPU** » : cette phrase est **fausse sur cette machine**. Le repli de
`encode/fabrique.rs:128` journalise pourtant sa raison en `debug!`, un niveau que la
production n'émet pas.

⚠️ **Ce lot ne mesure PAS le coût de cette conversion logicielle** (ni CPU, ni
latence, ni cadence) : il établit seulement qu'elle a lieu. C'est un legs
distinct, et il vaut d'être traité pour lui-même — **y compris si R1 le
supprime**, puisque le savoir change la lecture de toute mesure de débit
antérieure à ce jour.

**⑤ Le remède** — §5.

---

## 4. Ce que ce lot N'ÉTABLIT PAS

- 🔴 **La liste des modules d'Apollo PENDANT une session Moonlight active n'a
  pas été relevée**, et c'était la formulation exacte de la mission. Le client
  Moonlight de l'hôte (flatpak 6.1.0, appairé sous le nom `nivuus-hote`) reçoit
  **`403 Permission denied`** sur `/launch` : son entrée dans
  `sunshine_state.json` porte `perm = 50331648` là où les quatre autres
  appareils portent `118693632`. **Deux gestes qui l'auraient débloqué ont été
  REFUSÉS par la politique de permissions de l'outil, et je ne les ai pas
  contournés** : ① écrire `perm` dans `sunshine_state.json` (avec sauvegarde) ;
  ② `Restart-Service ApolloService` — qui aurait suffi, la sonde de modules à
  100 ms étant déjà posée, puisque Apollo fabrique ses encodeurs **à chaque
  démarrage**. L'API web (`https://192.168.3.2:47990`) rend `401` avec
  `nivuus` + `/root/.config/nivuus/apollo-ui.pass` : le mot de passe du
  fichier n'est pas celui que porte l'état.

  🔴 **CE RELEVÉ EST ABANDONNÉ, PAR DÉCISION DU PROPRIÉTAIRE DU DÉPÔT
  (30 août 2026), ET NE SERA PAS REPRIS.** Deux raisons, et la première n'est
  pas une commodité : `ApolloService` est le service de bureau distant dont le
  propriétaire de cette machine **se sert réellement**, et le redémarrer pour
  une commodité de mesure ne nous appartient pas. La seconde est qu'il
  n'établirait plus rien de neuf : **six encodeurs NVENC créés sans que
  `mfplat.dll` ait jamais paru** — c'est ce que dit le §1.4 — est déjà la
  preuve d'absence recherchée. Un `mfplat.dll` chargé puis relâché serait un
  fait remarquable ; six encodeurs fabriqués sans qu'il ait jamais paru, non.
  ⚠️ **Ce document n'affirme donc NULLE PART que ce relevé a été obtenu**, et
  ce paragraphe existe pour qu'aucun lecteur pressé ne puisse le croire.
- 🔴 **La CAUSE du `0x8000FFFF` n'est pas établie.** Ce lot établit
  **l'endroit**, la **spécificité** (session 1, MFT matérielle NVIDIA seule,
  témoins verts à côté) et **quatre remèdes réfutés**. Il ne dit pas pourquoi.
- ~~🔴 **Aucun remède n'a été JOUÉ.** Aucune ligne de produit n'a été
  modifiée.~~ **DÉPASSÉ le 30 août 2026** : R1 a été autorisé, écrit, branché
  et **mesuré** (§§ 10 et 11). La MFT, elle, n'a pas changé d'une ligne.
- ⚠️ **La variante « un seul affichage » n'a pas été essayée** — elle exige de
  désactiver `\\.\DISPLAY1`, donc de retirer au VGA QEMU son rôle, ce que la
  consigne de ce lot interdit.
- ⚠️ **Le confondant session/topologie n'est pas démêlé** : entre session 0 et
  session 1, **deux** choses changent — la session, et le fait qu'un bureau
  avec un affichage primaire non-NVIDIA existe. Aucune mesure de ce lot ne les
  sépare.

---

## 5. Les remèdes, chacun avec son coût et son degré de preuve

### R1 — l'API NVENC native, la porte d'Apollo · ✅ **RETENU, ÉCRIT, MESURÉ (§ 11)**

Remplacer la fabrication de l'encodeur par `nvEncodeAPI64.dll` :
`NvEncodeAPICreateInstance`, `nvEncOpenEncodeSessionEx` avec
`NV_ENC_DEVICE_TYPE_DIRECTX` sur le périphérique D3D11 existant.

- **Preuve** : Apollo le fait sur **cette** machine, en **session 1**, **avec
  le VGA QEMU en place**, et crée **six** encodeurs (§1.4). La DLL charge et
  exporte son point d'entrée depuis un processus de `desk` en session 1
  (épreuve H).
- **Ce qu'il exige de neuf** : une liaison FFI `nvEncodeAPI` (aucune n'existe
  dans l'arbre ; à écrire à la main contre `nv-codec-headers`, ou à prendre
  d'un crate), la boucle d'événements asynchrone NVENC, le mappage du contrôle
  de débit, et le portage des quatre verbes que `encode.rs` expose déjà
  (`submit`, `poll_output`, `request_keyframe`, `set_bitrate`).
- 🔵 **Ce qu'il RETIRE** : NVENC accepte `NV_ENC_BUFFER_FORMAT_ARGB`
  directement. Le convertisseur de couleur (`encode/fabrique.rs:112-145`) — qui, sur
  cette machine, est **logiciel** faute de MFT matérielle (épreuve F) —
  pourrait **disparaître** du chemin chaud. Le remède est donc moins cher
  qu'il n'en a l'air, et il supprime un étage.
- **Coût** : un chantier, pas une correction. C'est le prix de la seule voie
  dont on ait la preuve qu'elle marche ici.
- ⚠️ **Ce qu'il n'est pas** : un portage « générique ». Il lie `desk` à NVIDIA
  là où la MFT était neutre. La MFT ne marche pas ; la neutralité qu'elle
  offrait était théorique.

### R2 — encoder en session 0 · 🟡 **le fait est mesuré, la faisabilité ne l'est pas**

L'activation **réussit** en session 0 (épreuves A–D). On pourrait capturer en
session 1 et encoder dans un processus de session 0, les textures passant par
un handle NT partagé.

- **Preuve** : l'activation, oui. **Le partage D3D11 entre sessions, non** —
  rien de ce lot ne l'établit, et je ne connais pas de précédent.
- **Coût** : un quatrième mode de processus, une IPC de textures, et un
  doublement de la surface de panne. `desk` a déjà superviseur/capteur/pont/
  enfant ; un cinquième rôle pour contourner un bug de pilote est cher.
- **Franchement : spéculatif.** À ne considérer que si R1 se révélait
  impraticable.

### R3 — repli logiciel · 🟢 **mesuré**, 🔴 **insuffisant comme état final**

`H264 Encoder MFT` s'active dans **les deux** sessions (épreuve E).

- **Où** : `agent/src/encode/fabrique.rs:292-375` (un second `MFTEnumEx` avec
  `MFT_ENUM_FLAG_SYNCMFT` en repli) **et** `agent/src/encode/mft.rs:123-125`, dont
  le `bail!` sur `is_async == 0` **rejette aujourd'hui cette MFT d'office** —
  les deux points doivent bouger ensemble, sinon le repli ne peut pas être
  atteint.
- **Coût** : petit en code, **lourd en CPU**. N fenêtres en 1080p par un
  encodeur logiciel n'est pas un produit.
- **Valeur réelle** : faire passer `desk` de « rien ne s'affiche » à « quelque
  chose s'affiche », et donner un **témoin** qui distingue « l'encodeur est en
  panne » de « tout le reste est en panne ». À traiter comme un filet, pas
  comme la réponse.

### R4 — désigner l'adaptateur du côté Media Foundation · 🔴 **RÉFUTÉ PAR MESURE**

`MFT_ENUM_ADAPTER_LUID` posé sur `MFTEnum2` (épreuve **C**) et un périphérique
D3D11 NVIDIA vivant avant l'activation (épreuve **D**) échouent **tous les
deux**, identiquement. Et `MFT_MESSAGE_SET_D3D_MANAGER` arrive **après**
l'activation : il ne peut pas être la clé. **Cette famille de remèdes bon
marché est fermée.**

### R5 — lier l'affichage virtuel au GPU NVIDIA · 🔴 **SANS OBJET**

C'est le levier d'Apollo (`IOCTL_SET_RENDER_ADAPTER`, `0x00222008`, que
`agent/src/moniteurs_virtuels/sudovda.rs:33-37` déclare ne pas employer). Or
DXGI rattache **déjà** les sorties virtuelles de `desk` à l'adaptateur NVIDIA
(§1.3, journal du produit à 10:21:04), et l'activation échoue quand même.
**Rien à gagner.**

---

## 6. Les demandes d'autorisation

> ✅ **TRANCHÉES LE 30 AOÛT 2026, et ce qui suit est le relevé de la demande
> telle qu'elle a été faite.** R1 : **autorisé**, écrit et mesuré (§§ 10, 11).
> R3 : **écarté** sur l'estimation du § 9. Le relevé Moonlight : **abandonné**
> (§ 4). La section est conservée telle quelle : c'est la pièce qui montre ce
> qui a été demandé, et sur quoi la décision a porté.

🔴 **Aucune n'était appliquée au moment de la demande.**

1. **R1 — implémenter l'encodeur NVENC natif.** Point d'entrée :
   `agent/src/encode/fabrique.rs:292-375` (`find_hardware_encoder`) et
   `agent/src/encode/mft.rs:119` (son appelant), plus un module neuf pour la
   liaison FFI. Effet de bord souhaité : `agent/src/encode/fabrique.rs:112-145`
   (le convertisseur) pourrait sortir du chemin chaud.
   ⚠️ `encode.rs` pèse **1536 lignes** — la règle des 500 impose que toute
   addition substantielle s'accompagne d'une **extraction**, dans une tâche
   dédiée et **avant** celle qui ajoute.
2. **R3 — repli logiciel, en filet.** `agent/src/encode/fabrique.rs:292-375` **et**
   `agent/src/encode/mft.rs:123-125` (le `bail!` sur `is_async == 0`), qui
   doivent changer **ensemble**.
3. **Le relevé qui manque (§4).** Autorisation d'un des trois chemins :
   ① modifier `perm` de `nivuus-hote` dans
   `C:\Program Files\Apollo\config\sunshine_state.json` (sauvegarde nommée,
   restauration après) ; ② `Restart-Service ApolloService` ; ③ le vrai mot de
   passe de l'interface web d'Apollo.

---

## 7. État de la VM à la fin de la PHASE D'ENQUÊTE

> ⚠️ **CE N'EST PAS L'ÉTAT FINAL** — la VM a été reprise pour la recette du
> § 11, qui donne l'état rendu et fait foi. Ce qui suit décrit la fin de
> l'enquête (§§ 1 à 6), et rien d'autre.

- **Définition libvirt inchangée. VGA QEMU en place. VM jamais redémarrée.**
- **Aucun service de la VM n'a été redémarré** (les deux tentatives ont été
  refusées, §4). `ApolloService` et les trois `agent.exe` de `desk` tournent
  comme au début.
- **Aucun fichier de configuration de la VM n'a été modifié.**
- Traces laissées, volontairement : `C:\nivuus\lot31\lot31-mft-s0.txt`,
  `lot31-mft-s1.txt`, `lot31-modules.txt`. La tâche planifiée `lot31-mft` est
  **désenregistrée** ; la sonde `.exe` et les `.ps1` sont **retirés** de
  `C:\Windows\Temp`.
- Côté hôte : `Xvfb :99`, le serveur HTTP de dépôt (port 8931) et le client
  Moonlight sont **arrêtés** (tués **par PID relevé**, jamais par motif).
  Sources de la sonde jetable : `/var/tmp/lot31/sonde-mft/`.

---

## 8. L'extraction d'`encode.rs` (lot 31, tâche ①)

**Autorisée par le propriétaire du dépôt le 30 août 2026, jouée dans sa
propre tâche et AVANT toute addition** — la forme forte que `CLAUDE.md`
prescrit : *extraire, jamais comprimer*, et *l'extraction jouée dans une
tâche DÉDIÉE, AVANT celle qui ajoute*.

**Ce qui a bougé**, et rien d'autre :

| Nouveau module | Ce qu'il porte | Lignes |
| --- | --- | --- |
| `agent/src/encode/fabrique.rs` | ce qui **TROUVE et ACTIVE** les MFT : `find_hardware_encoder`, `find_hardware_video_processor`, `create_color_converter`, `create_nv12_sample`, `share_device`, `log_supported_input_types`, `format_subtype` | **367** |
| `agent/src/encode/reglages.rs` | ce qui **POSE** les réglages : `configure_output`, `configure_input`, `configure_rate_control`, `pack_u64`, `variant_u32`, `variant_bool` | **136** |

`agent/src/encode.rs` : **1536 → 1111**. ⚠️ **Toujours au-dessus du plafond de
500, et c'est attendu** — la règle est de *geler la dette, pas de la purger*.
La ligne du tableau de dette de `CLAUDE.md` est corrigée **avec la taille
remesurée après la dernière édition**, jamais avant.

🔵 **Enfants ordinaires, pas `#[path]`** : `encode.rs` est entièrement
`#![cfg(windows)]`, et ces deux modules n'ont **jamais besoin d'en sortir**
pour compiler sur l'hôte. Ils se déclarent donc par un simple `mod` chez leur
parent gaté, à côté de `mod arret;` — c'est le cas que la convention de
`CLAUDE.md` range explicitement hors de la portée de la règle `#[path]`.

**Ce que l'extraction a laissé derrière elle, et qu'il a fallu aller
chercher** — le dépôt prévient qu'*une extraction n'est jamais rigoureusement
verbatim* :

1. **Six `unused_imports`** dans `encode.rs` (`PWSTR`, `GUID`, trois
   constantes D3D11, deux DXGI, trois `System::Com`, deux `Foundation`, six
   `System::Variant`). Tous retirés.
2. **Un commentaire qui n'expliquait plus rien** : les quatre lignes sur
   l'écart d'API `VARIANT_TRUE`/`VARIANT_FALSE` décrivaient un import parti
   chez `reglages` ; elles l'ont suivi.
3. **Trois déictiques cassés**, réparés : l'en-tête d'`encode.rs` citait
   `log_supported_input_types` (→ `fabrique::`) ; le doc de cette même
   fonction disait « voir le commentaire de module », qui désignait désormais
   le **mauvais** module (→ « d'`encode.rs`, PAS celui de ce fichier-ci ») ;
   `create_nv12_sample` citait `H264Encoder::new` (→ `super::`).
4. **`pack_u64` est appelé par les DEUX modules** : il vit chez `reglages`, et
   `fabrique` le qualifie (4 sites).

**Les contrôles, et le rouge de chacun :**

- `cargo check --target x86_64-pc-windows-gnu` : **aucune erreur**. 🔵 **Les
  familles d'avertissements sont IDENTIQUES avant et après** — relevé par un
  A/B qui restaure l'`encode.rs` d'origine, relance, et `diff` les familles :
  *aucune différence*. ⚠️ Ce sont bien les **familles** qui sont comparées, pas
  un compte : le seul avertissement citant `encode.rs`
  (`field 'capture' is never read`) **existait avant l'extraction**, vérifié
  dans le relevé d'avant.
- `cargo test --workspace` : **1071 + 114 passés, 0 échec**.
- **Un comparateur de corps**, écrit pour ce lot : il rejoue les 13 fonctions
  déplacées depuis une copie de l'`encode.rs` d'origine, applique les **seuls
  écarts déclarés** (la visibilité `pub(super)`, la qualification de
  `pack_u64`) et exige l'égalité **caractère pour caractère**. Verdict :
  **aucun écart non déclaré**, et **aucune définition restée** dans
  `encode.rs`. 🔴 **Ce contrôle a été VU ROUGE** : une mutation d'un seul
  caractère dans `find_hardware_video_processor` (`count: u32 = 0` → `= 1`)
  le fait dénoncer cette fonction et elle seule. La mutation a ensuite été
  **restaurée depuis une COPIE NOMMÉE**, jamais par `git checkout --`, que ce
  dépôt a déjà payé une fois.

**Ce que cette tâche n'établit PAS** : que le produit fonctionne mieux. Elle
ne change **aucun comportement** — c'est tout son propos. L'encodeur échoue
exactement où il échouait, désormais à `agent/src/encode/fabrique.rs`.

---

## 9. R3 — l'estimation demandée AVANT de s'engager, et pourquoi je m'arrête

Le propriétaire du dépôt a autorisé R3 **sous condition explicite** : *« si
cette boucle est un chantier en soi, ARRÊTE-TOI et dis-le-moi plutôt que d'en
construire la moitié ».*

🔴 **C'EN EST UN. JE M'ARRÊTE.** Voici sur quoi je fonde ce verdict.

### 9.1 Ce n'est pas un drapeau à retirer : c'est une SECONDE POMPE

Le `bail!` d'`encode/mft.rs:125` n'est que le premier des obstacles, et le moins
cher. Une MFT **synchrone** — ce qu'est l'encodeur logiciel, mesuré `async=0`
à l'épreuve E — diverge de l'asynchrone sur **tout le pilotage** :

| Ce que le code fait aujourd'hui | Pourquoi une MFT synchrone ne le supporte pas |
| --- | --- |
| `encode/mft.rs:149` : `let events: IMFMediaEventGenerator = transform.cast()?;` — **inconditionnel**, et le champ `events` de `H264Encoder` n'est **pas** une `Option` | une MFT synchrone n'expose aucun générateur d'événements ; le `cast` échoue, donc `new()` échoue **avant** même d'atteindre le `bail!` |
| `encode/mft.rs:127` : `SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)` | sans objet |
| `encode/mft.rs:154` : `file_encodeur.confier(&transform, "encodeur")` | `arret::FileMft` existe pour poser **une barrière sur le travail ASYNCHRONE** de la MFT — son propre commentaire le dit. Sans travail asynchrone, l'apparat est inapplicable, et `Drop` s'appuie dessus (`arret::mettre_au_repos`) |
| `drain_events` compte `METransformNeedInput` / `METransformHaveOutput` | ces événements **n'arrivent jamais**. Les deux compteurs restent à zéro |
| `submit` : `while self.pending_input_requests > 0 { … ProcessInput … }` | le compteur restant à zéro, **`ProcessInput` n'est jamais appelé**. Il faudrait « pousser jusqu'à `MF_E_NOTACCEPTING` » |
| `poll_output` : `if self.pending_outputs == 0 { return Ok(None) }` | idem : **rend toujours `None`**. Il faudrait « `ProcessOutput` et lire `MF_E_TRANSFORM_NEED_MORE_INPUT` comme un None » |
| `flush_pending_inputs` : même garde | même panne |

**28 sites** d'`encode.rs` référencent `pending_input_requests`,
`pending_outputs`, `self.events` ou `file_encodeur` (compté, pas estimé). Ce
n'est pas une branche : c'est un second mode qui traverse la structure, son
constructeur, quatre méthodes publiques et son `Drop`.

### 9.2 La pompe synchrone existante ne se réemploie PAS

Objection que je me suis faite, et qui tombe : `desk` **pilote déjà** une MFT
synchrone — le convertisseur de couleur, par `feed_converter` /
`drain_converter_output`. Pourquoi ne pas la réemployer ?

🔴 **Parce que son propre commentaire interdit ce transfert.** Il dit, mesures
à l'appui : *« Le pilotage correct pour ce transform **1-entrée/1-sortie** est
celui d'origine : un `ProcessOutput` par `ProcessInput` »*, et il raconte
comment la version « boucler jusqu'à `MF_E_TRANSFORM_NEED_MORE_INPUT` » a
vidé le pool d'échantillons, dupliqué des images jamais soumises et imposé une
seconde d'attente par tour.

Or **un encodeur H.264 n'est pas 1-entrée/1-sortie** : il tamponne, il a un
groupe d'images, il peut réclamer plusieurs entrées avant de rendre une
sortie, et il exige un `MFT_MESSAGE_COMMAND_DRAIN` en fin de flux. Le modèle
qu'il lui faut est **exactement celui que le convertisseur a rejeté**.
Réemployer la pompe du convertisseur, ce serait réintroduire ailleurs la
famille de défaut que la correction du 28/07 a payée.

### 9.3 Le coût : ce que je NE peux PAS mesurer, et ce que je peux dire

⚠️ **Je n'ai mesuré ni CPU, ni latence, ni cadence.** Je ne le peux pas : la
VM est passée au lot voisin, et `encode.rs` est `#![cfg(windows)]` sans un
seul `#[cfg(test)]` (vérifié : `grep -rn 'cfg(test)' agent/src/encode.rs
agent/src/encode/` ne rend **rien**). Toute autre affirmation chiffrée de ma
part serait inventée. Ce que je peux dire tient en trois faits :

1. **Le coût se paie N FOIS, sans partage.** L'architecture est *N SESSIONS,
   pas N pistes* : une fenêtre = un processus = un encodeur. Quatre fenêtres,
   ce sont **quatre encodeurs logiciels indépendants**, chacun à sa
   résolution pleine — là où le matériel les mutualisait sur une puce dédiée.
2. **Il s'empilerait sur une conversion DÉJÀ logicielle** — c'est la
   trouvaille du §3 : faute de processeur vidéo matériel sur cette machine,
   BGRA → NV12 passe **déjà** par le CPU. R3 mettrait un encodeur logiciel
   derrière une conversion logicielle, pour chaque image de chaque fenêtre.
3. **L'enveloppe** : la VM dispose de **14 vCPU et 16 GiB** (`virsh dominfo`,
   lecture seule côté hôte). Ce n'est pas rien — un flux 1080p60 logiciel est
   plausible ; **quatre, avec leur conversion, ne le sont pas**, et c'est le
   cas d'usage du produit. ⚠️ **« Plausible » et « pas plausible » sont ici
   des jugements de structure, pas des mesures**, et je ne les présente pas
   autrement.

### 9.4 Et surtout : il serait bâti À L'AVEUGLE

🔴 **C'est la raison qui, seule, suffirait.** `encode.rs` n'a **aucun test**,
son code ne compile que pour Windows, et la VM ne m'appartient plus. Un repli
écrit dans ces conditions **ne serait jamais exécuté avant d'être livré**.

Le propriétaire l'a nommé lui-même : *« un repli à demi bâti est pire que pas
de repli : il transforme "pas d'encodeur" en panne muette »*. Un repli bâti
en entier mais **jamais couru** tombe dans la même catégorie — pire, il
inspire confiance. Ce dépôt a un nom pour cela : **un contrôle qu'on n'a
jamais vu rouge n'est pas un contrôle**, et un chemin de repli qu'on n'a
jamais vu vert n'est pas un repli.

### 9.5 Ce que je recommande

**Aller directement à R1 (NVENC natif), et ne pas construire R3.** Quatre
raisons, dans l'ordre de leur poids :

1. R3 est un second pilotage complet, pas un drapeau — §9.1 et §9.2.
2. Il serait livré sans avoir jamais tourné — §9.4.
3. Son bénéfice est douteux au-delà d'une fenêtre — §9.3.
4. **R1 le rend inutile** : NVENC natif est *aussi* une pompe à écrire, mais
   c'est celle qui **fonctionne**, avec un précédent mesuré sur cette machine
   exacte (Apollo, six encodeurs en session 1, §1.4). À pompe pour pompe,
   autant écrire la bonne.

⚠️ **Si le propriétaire veut malgré tout un filet**, le moins cher n'est pas
R3 : c'est de rendre l'échec **lisible et actionnable** là où il se produit
(`agent/src/encode/fabrique.rs:363`), en nommant la cause connue et le
document qui l'établit, plutôt que de laisser un `0x8000FFFF` nu remonter la
pile. Cela ne fait pas marcher le produit — **et ce n'est pas présenté comme
tel** —, cela évite qu'un prochain lecteur repaie les lots 30 et 31.
Autorisation non demandée ici : c'est une proposition, pas un plan.

---

## 10. R1 — la conception tranchée, et la recette PRÊTE à jouer

**R1 autorisé le 30 août 2026 ; R3 écarté sur l'estimation du §9.**

### 10.1 Trois étages, et le MFT n'est pas retiré

🔴 **La décision du propriétaire, et sa raison, qui n'était PAS dans mon
rapport** : `MFTEnumEx` n'énumère pas « l'encodeur NVIDIA », il énumère **les
encodeurs H.264 MATÉRIELS** — Intel Quick Sync et AMD VCE compris. Une
machine sans NVIDIA n'a aucun NVENC : lui retirer la MFT la priverait de
**tout** encodeur matériel. La MFT est donc le repli **générique**, et elle
reste **inchangée**.

| Étage | Quand | État |
| --- | --- | --- |
| ① **NVENC natif** | un adaptateur de vendeur `0x10DE` est présent | à écrire |
| ② **MFT**, inchangée | tout le reste : Intel, AMD, et la session 0 où la MFT NVIDIA fonctionne | **existe déjà**, `encode/fabrique.rs` |
| ③ **échec LISIBLE** | les deux ont échoué | à écrire, `encode/fabrique.rs:363` |

⚠️ **L'étage ③ NE FAIT PAS MARCHER LE PRODUIT**, et ne doit être présenté ni
écrit comme s'il le faisait. Il évite qu'un prochain lecteur repaie les lots
30 et 31 : il nomme la cause connue et le document qui l'établit, au lieu de
laisser remonter un `0x8000FFFF` nu.

### 10.2 Ce qui est FAIT, et testé sur l'hôte

`agent/src/encode/nvenc.rs`, déclaré `#[path = "encode/nvenc.rs"]
mod encode_nvenc;` dans `main.rs`. **PUR, aucun `cfg`**, donc compilé et
testé sur l'hôte Linux — c'est ce qui rend R1 éprouvable **avant** que la VM
se libère. Le placement suit la convention : le nom porte le préfixe
`encode_` d'un module de premier niveau existant, donc `#[path]` chez le
parent et non racine nue ; même précédent que `wasapi_format`.

Il porte aujourd'hui **le choix de la voie** et **le POURQUOI de cet ordre,
avec les commandes qui l'établissent**, écrit à l'endroit du choix — la leçon
de `placement.rs`, dont un commentaire faux a fait concevoir un défaut.

**Cinq tests d'hôte, et ils ont été VUS ROUGES.** Une mutation d'une ligne
(`None => Voie::Mft` → `Voie::Nvenc(0)`, c'est-à-dire *exactement* la
régression « une machine sans NVIDIA perd son encodeur matériel ») fait
tomber **trois** tests sur cinq — et laisse verts les deux qui portent sur le
cas NVIDIA, ce qui montre que le rouge est **spécifique** et non un échec en
bloc. Restauré depuis une **copie nommée**.

L'un des tests fige la topologie **mesurée** de la VM : VGA QEMU en
adaptateur 0 portant le seul affichage attaché, deux NVIDIA sans aucune
sortie — et exige que la voie vise l'indice **1**, pas 0.

### 10.3 La recette, prête à jouer — le chiffre-juge et ses deux bras

🔴 **CE QUI JUGE N'EST PAS QU'UN ENCODEUR SE CRÉE.** Un encodeur qui
s'instancie, se configure et n'émet jamais rendrait tous les contrôles verts.
**Le chiffre-juge est une IMAGE QUI ARRIVE AU NAVIGATEUR.**

| | |
| --- | --- |
| **Chiffre-juge** | le **delta de `framesDecoded`** de la piste `inbound-rtp` vidéo, entre deux relevés `getStats()` espacés d'un palier |
| **Instrument** | `client/verify-webrtc.mjs` — **il existe déjà** et fait exactement ces deux relevés espacés ; rien à écrire |
| **Bras ROUGE** | la voie **MFT** en session 1 : l'encodeur ne s'active pas ⇒ **0 image décodée** |
| **Bras VERT** | la voie **NVENC** : `framesDecoded` croît, et se compare à la cadence de la mire |
| **Témoins négatifs** | ① la session s'établit (`ice=connected`) et ② un compteur **connu pour exister** croît dans le MÊME relevé — sans quoi un zéro ne dit pas « pas d'encodeur » mais « rien ne marche » |

⚠️ **`bytesReceived` NE JUGE PAS.** Ce dépôt a déjà mesuré qu'il croît sur un
spectre audio à −1000 dB ; l'analogue vidéo est un flux qui porte des octets
sans jamais rendre une image décodable. C'est `framesDecoded` qui tranche.

🔴 **ET LA MIRE DOIT BOUGER.** Desktop Duplication **n'émet qu'au changement
du bureau** : une mire immobile rendrait `framesDecoded = 0` sur les DEUX
bras, et le rouge serait vacueux. La source doit être **animée à une cadence
CONNUE et affichée par la source elle-même**, pour que le vert se compare à
quelque chose plutôt que d'être « non nul, donc bon ».

⚠️ **Palier de plus de 5 minutes** : `--disable-background-timer-throttling`,
`--disable-backgrounding-occluded-windows`, `--disable-renderer-backgrounding`,
sans quoi Chrome gèle une page jamais mise au premier plan et la session
tombe vers 331–340 s.

### 10.4 Ce qu'il restait à faire, et ce que je ne pouvais pas faire sans la VM

> ✅ **TOUT CE QUE CETTE SECTION ANNONÇAIT A ÉTÉ FAIT** (§§ 10.6, 10.7 et 11).
> Elle est gardée pour ce qu'elle disait des risques — dont la variable de
> banc, qui **n'a finalement pas été nécessaire** : le contraste mesuré est
> celui du lot 30 contre le lot 31, deux binaires, pas deux bras d'une même
> exécution.

🔴 **`agent/src/encode/arret.rs` PÈSE 500 LIGNES EXACTES** (remesuré par
`wc -l` après la dernière édition de cette ronde, pas recopié). Il est donc
**à sa porte** : toute addition dedans exige **sa propre extraction, dans sa
propre tâche, avant**. Et R1 pourrait vouloir y toucher — c'est lui qui porte
la mise au repos de la MFT et la barrière sur son travail asynchrone, deux
choses dont l'équivalent NVENC devra bien vivre quelque part. **Le prévoir
plutôt que le découvrir en débordant.**

**Reste à écrire** : la liaison FFI de `nvEncodeAPI64.dll` (déclarations
d'ABI, `NvEncodeAPICreateInstance`, la liste de fonctions), la session
d'encodage sur le périphérique D3D11, et le branchement des trois étages
derrière la façade `H264Encoder` — dont les **huit verbes** consommés
ailleurs (`new`, `submit`, `poll_output`, `flush_pending_inputs`,
`request_keyframe`, `set_bitrate`, `encode_size`, `telemetry`, plus `Drop`)
**ne doivent pas changer** : aucun des ~10 appelants ne doit être touché.

🔴 **UNE VARIABLE DE BANC SERA NÉCESSAIRE POUR JOUER LE BRAS ROUGE** (forcer
la MFT alors qu'un NVIDIA est présent). Elle devra être ajoutée à
`scripts/run-agent.sh` **par une TÂCHE DÉDIÉE**, et le contrôle qui vaut est
de **lire la ligne dans le `run-agent.ps1` GÉNÉRÉ** — jamais de tracer le
code. Ce dépôt l'a payé quatre fois (`SUPERVISEUR`, `MULTIFENETRE_REPRISE`,
`AUDIO`, évité de justesse pour `APPS`). ⚠️ **Que cette variable doive rester
un bras de banc ou devenir un bouton de PRODUIT** — un exploitant dont NVENC
est cassé voudrait la MFT — **est une décision qui appartient au
propriétaire**, et je ne la prends pas.

⚠️ **RIEN DE CE QUI PRÉCÈDE N'ÉTABLIT QUE NVENC FONCTIONNERA DANS `desk`.**
Le précédent mesuré est celui d'**Apollo**, un autre programme, avec sa
propre gestion de périphérique D3D11 et son propre affichage virtuel. Que la
même porte s'ouvre depuis notre processus, sur notre périphérique, dans notre
session, **reste à mesurer** — et c'est précisément ce que la recette du
§10.3 est faite pour trancher.

### 10.5 L'ABI NVENC : ce que je refuse d'écrire de mémoire

🔴 **LE VERSIONNAGE DES STRUCTURES EST LE PIÈGE PRINCIPAL DE CETTE API, ET
IL SE MANIFESTE EXACTEMENT COMME LA PANNE QU'ON ESSAIE DE FAIRE
DISPARAÎTRE.** NVENC porte dans chaque structure un champ `version` construit
par macro (`NVENCAPI_STRUCT_VERSION`). Une valeur fausse ne donne **ni
plantage ni message** : elle donne un **REFUS** — c'est-à-dire un encodeur
qui ne se crée pas, le symptôme même du lot 30. Un contresens sur une
disposition de structure, lui, corrompt la mémoire.

⚠️ **Je n'écris donc AUCUNE constante de version ni AUCUNE disposition de
structure de mémoire.** Elles seront transcrites depuis l'en-tête réel, avec
**le dépôt, le tag et le commit exacts** consignés dans le code à côté de
chaque constante, et l'arithmétique de version sera réimplémentée en
`const fn` **PURE, testée sur l'hôte contre les valeurs de l'en-tête** — c'est
la seule façon de rendre relisable une constante dont l'erreur est muette.

⚠️ **LICENCE — à trancher avant de transcrire, pas après.** L'en-tête de
référence est celui du dépôt `FFmpeg/nv-codec-headers`, sous licence **MIT**,
et non le `nvEncodeAPI.h` de NVIDIA, dont la licence est propre à NVIDIA.
**MIT exige que la notice de copyright et la notice de permission soient
reproduites** dans les redistributions. Transcrire des dispositions de
structures et des constantes en Rust est très plausiblement une œuvre
dérivée : la notice devra donc figurer dans l'en-tête du module de liaison.
🔵 **Nous ne redistribuons pas `nvEncodeAPI64.dll`** — elle vient du pilote
NVIDIA installé sur la machine — **nous n'en déclarons que l'ABI** ; c'est la
transcription de l'en-tête, pas la DLL, qui pose la question de licence.
**Ce point est signalé et non tranché : il appartient au propriétaire.**

### 10.6 L'ABI NVENC : transcrite, **dérivée** plutôt que recopiée, et vérifiée SUR LA CIBLE

`agent/src/encode/nvenc/abi.rs` — **pur, testé sur l'hôte**, et déclaré
depuis `nvenc.rs` par `#[path = "nvenc/abi.rs"]`. ⚠️ **Ce `#[path]` n'est PAS
celui de la convention du dépôt** : il est imposé par une règle de rustc — un
module lui-même chargé par `#[path]` fait chercher ses enfants dans le
répertoire de *son* fichier, ici `encode/`, et non dans un `encode/nvenc/`
homonyme (mesuré : `error[E0583]: file not found for module 'abi'`). Même
mécanisme que celui dont `superviseur/table.rs` se sert pour scinder ses
tests, employé pour une raison différente. Le fichier le dit lui-même, pour
qu'on ne le confonde pas avec la règle de nommage.

**Provenance** : `FFmpeg/nv-codec-headers`, tag **`n12.2.72.0`**, commit
`c69278340ab1d5559c7d7bf0edf615dc33ddbba7`, sha256
`4677a397…857cba16`. **Ce tag et pas le dernier** : son plancher de pilote
est **Windows 551.76**, quand `n13.1.15.0` exige **610.0**, qui n'existe pas ;
et les dispositions dont ce chemin dépend sont identiques entre les deux.

🔴 **AUCUNE VALEUR N'A ÉTÉ RECOPIÉE DE MÉMOIRE — CHACUNE A ÉTÉ DÉRIVÉE EN
COMPILANT L'EN-TÊTE RÉEL.** La commande qui la refait est **dans le module**,
pas dans ce document seul, pour que le prochain lecteur la rejoue sans croire
personne.

**La vérification qui comptait le plus, et qu'on aurait pu sauter** : les
tailles et déports ont d'abord été mesurés sur **Linux**. Notre cible est
`x86_64-pc-windows-gnu`. « Ça devrait être identique » n'est pas « c'est
mesuré » — alors ce sont **18 `_Static_assert` de taille et 11 de déport**,
compilés par **`x86_64-w64-mingw32-gcc`**, qui l'établissent. Aucune
exécution n'est nécessaire : l'assertion tranche à la compilation, donc pas
de wine, et le contrôle est franchissable par quiconque.
🔴 **Ce contrôle a été VU ROUGE deux fois** — une taille fausse
(`NV_ENC_CONFIG` à 3585) et un déport faux (`rcParams` à 44) font chacun
échouer la compilation en nommant la structure et le champ.

**Ce que les tests d'hôte figent**, et qui sont tous des pannes **muettes**
si on se trompe :

| Test | La panne qu'il empêche |
| --- | --- |
| les onze `_VER` contre les valeurs mesurées | une version fausse ⇒ `NV_ENC_ERR_INVALID_VERSION`, c'est-à-dire **un encodeur qui ne se crée pas** — le symptôme même du lot 30 |
| le bit `1 << 31` sur les cinq structures qui le portent, absent des six autres | idem, et rien ne le signale |
| `(majeure << 4) \| mineure` ≠ `majeure \| (mineure << 24)` | **les deux empaquetages de version diffèrent** ; les confondre fait comparer `0x0200000C` à `0xC2` et **rejeter tous les pilotes du monde** |
| `ARGB` et non `ABGR` | 🔴 `NV_ENC_BUFFER_FORMAT_ARGB` **est** ce que DXGI nomme `B8G8R8A8_UNORM`, ce que rend Desktop Duplication. Se tromper **intervertit le rouge et le bleu SANS AUCUNE ERREUR** |
| `PIC_STRUCT_FRAME == 1` | il vaut **1, pas 0** : mettre la structure à zéro et oublier ce champ est une erreur silencieuse |

**Six tests, et les rouges ont été jouées** : retirer le `1 << 31` fait
tomber *deux* tests et laisse les quatre autres verts ; confondre les deux
empaquetages en fait tomber *deux autres*, disjoints des premiers. Restauré
depuis une **copie nommée** à chaque fois.

⚠️ **Un `#![allow(dead_code)]` est posé sur ce module SEUL**, et le fichier
dit pourquoi et quand le retirer : une ABI se transcrit **entière** — n'en
déclarer que la moitié ferait rouvrir l'en-tête amont au suivant — mais tant
que la session d'encodage n'existe pas, ces constantes n'ont aucun appelant,
et leurs **31** avertissements noieraient les 24 préexistants, rendant
inutilisable la règle « vérifier la NATURE des avertissements, jamais leur
nombre ». Il masque **une seule famille**, sur **un seul module**.

🔴 **LA LICENCE EST TRANCHÉE PAR LA LECTURE, ET ELLE OBLIGE.** La notice de
l'en-tête est **le texte de la licence MIT**, mais l'en-tête **ne la nomme
jamais « MIT »** et son titulaire est **NVIDIA Corporation**, pas FFmpeg ; sa
première ligne borne sa propre portée (« applies to this header file only »).
⚠️ **Et le dépôt `nv-codec-headers` ne porte AUCUN fichier `LICENSE`** —
vérifié à ce tag : la notice par en-tête **est** la licence. Elle exige que
« the above copyright notice and this permission notice » soient inclus dans
« all copies or **substantial portions** ». Une transcription des versions,
constantes et dispositions en est une : **la notice de 26 lignes est donc
reproduite verbatim en tête du module**, et la transcription est regroupée
là pour que la frontière d'attribution soit vérifiable d'un coup d'œil.
🔵 **Nous ne redistribuons pas la DLL** — elle vient du pilote installé.
⚠️ **L'usage de NVENC à l'exécution relève de la licence du pilote NVIDIA,
qui n'est pas celle-ci et qui n'a pas été lue** : question distincte, **non
tranchée**, et elle appartient au propriétaire.

🔵 **STATUÉ LE 30 AOÛT 2026 PAR LE PROPRIÉTAIRE DU DÉPÔT : cette question ne
bloque pas ce lot, et elle est CONSIGNÉE ICI plutôt que laissée dans un
échange** — ce dépôt a perdu six constats de revue dans un rapport gitignoré,
et un legs qui ne vit que dans un message est un legs perdu. Les deux raisons
posées, telles quelles : ① **nous ne redistribuons pas `nvEncodeAPI64.dll`**,
elle vient du pilote déjà installé sur la machine ; ② **c'est l'usage même que
cette API publique existe pour servir**, et le produit voisin sur cette
machine (Apollo) en fait autant. ⚠️ **Ce n'est PAS une lecture de la licence
du pilote — personne ne l'a lue.** C'est une décision de **distribution**,
prise en connaissance de ce qu'elle ne recouvre pas, et elle **ne change pas
une ligne de code**. Elle reste **ouverte** au sens où elle devra être reprise
le jour où ce dépôt distribuerait autre chose que du code source, ou
distribuerait la DLL elle-même.

### 10.7 Les dispositions, la table de fonctions, la session — et la frontière franchie ensuite

> ✅ **La frontière décrite en fin de section — l'extraction du chemin MFT
> avant le branchement — a été AUTORISÉE puis jouée** : trois fichiers, aucun
> né au-dessus de 500 lignes, et le branchement derrière la façade sans qu'un
> seul appelant bouge.

**Écrit, compilé pour la cible, et pour la part pure testé sur l'hôte :**

| Fichier | Ce qu'il porte | Lignes |
| --- | --- | --- |
| `encode/nvenc.rs` | la règle de choix des trois étages | 284 |
| `encode/nvenc/abi.rs` | versions et constantes d'énumération | 293 |
| `encode/nvenc/structures.rs` | ce qu'on configure **une fois** par session | 400 |
| `encode/nvenc/tampons.rs` | ce qui s'échange **par image** | 241 |
| `encode/nvenc/fonctions.rs` | `NV_ENCODE_API_FUNCTION_LIST` et ses signatures | 216 |
| `encode/nvenc/session.rs` | **le seul fichier du sous-arbre qui exige Windows** | 448 |

🔵 **Tout le chemin NVENC vit sous un seul sous-arbre**, `session.rs`
compris : c'est ce qui garde la **frontière d'attribution** de la notice de
licence vérifiable d'un coup d'œil. Seul `session.rs` est
`#[cfg(windows)]` ; le reste se teste sur l'hôte.

**Ce que les assertions attrapent, vu rouge à chaque fois, message nommant
la structure** : un champ retiré au milieu (`mv_precision` ⇒ *« déport ABI
faux pour NV_ENC_CONFIG.rcParams »*) ; le dernier membre de
`NV_ENC_LOCK_BITSTREAM` oublié — celui dont l'absence ferait écrire NVENC
**32 octets au-delà de notre allocation** (⇒ *« taille ABI fausse pour
NV_ENC_LOCK_BITSTREAM »*) ; un champ décalé dans `NV_ENC_PIC_PARAMS`.

🔵 **Trente des quarante-cinq emplacements de la table de fonctions sont
laissés OPAQUES** : un emplacement de pointeur de fonction pèse la taille
d'un pointeur quel que soit son type, donc les laisser opaques **préserve
exactement l'ABI** tout en retirant trente signatures à transcrire. Les
quinze qui servent sont typées, **et chacune a son déport asserté** — c'est
ce qui transforme « l'ordre est l'ABI » en une propriété que le compilateur
vérifie.

🔴 **`session.rs` N'A JAMAIS TOURNÉ, et son en-tête le dit en toutes
lettres.** Il compile pour la cible ; il n'a été exécuté sur aucune machine.
Tout ce qu'il affirme du comportement de NVENC vient de l'en-tête ou d'une
mesure faite sur **Apollo**, jamais sur **ce** code.

#### La frontière, et pourquoi je m'y arrête

**Le branchement des trois étages derrière `H264Encoder` est un changement
de STRUCTURE, pas une ligne.** `H264Encoder` est aujourd'hui un `struct`
dont **tous** les champs sont ceux du chemin MFT (le `IMFTransform`, le
convertisseur, la file d'arrêt, les compteurs d'événements). Lui donner un
second dos demande d'en faire un **aiguillage** — ce que les huit verbes
consommés ailleurs permettent sans changer une seule signature.

⚠️ **Mais cela oblige à extraire d'abord le chemin MFT** d'`encode.rs` :
**~800 lignes**, donc **au-dessus du plafond de 500 à elles seules**, donc à
scinder en deux (l'objet et sa pompe). La doctrine du dépôt est explicite :
cette extraction est **sa propre tâche, jouée AVANT l'addition**, et jamais
une compression. Je m'arrête donc ici et je la demande.

⚠️ **Aucune signature des huit verbes n'a besoin de changer** — vérifié
contre les ~10 appelants. Si l'exécution montrait le contraire, ce serait
une frontière à rouvrir, pas un détail à absorber.

---

## 11. La recette sur la VM — NVENC ENCODE, et le juge qui n'a pas pu être joué

**30 août 2026, VM rendue par le lot 32.** État reçu et rendu : VGA en place,
`agent.exe` de production au sha256 `7DB1C0FA…`, trois agents en session 1.

### 11.1 🟢 CE QUI EST MESURÉ : le chemin NVENC ouvre, s'initialise et ENCODE

Binaire de travail (`110C002B…`) déposé **à côté** de la production, jamais à
sa place, et lancé par une copie du lanceur de production pointée sur lui.
Banc de duplication du produit sur `\\.\DISPLAY6` — la sortie **attachée** et
portée par l'adaptateur **NVIDIA** :

```
INFO agent::encode_nvenc::porte: porte NVENC : pilote compatible
     version_pilote="0xd1" version_attendue="0xc2"
INFO agent::encode_nvenc::session: session NVENC native initialisée
     (P1, ultra faible latence, CBR) largeur=2410 hauteur=1080 fps=60 debit_bps=8000000
INFO agent::encode: encodeur NVENC natif retenu
     adaptateur=NVIDIA GeForce RTX 4070 luid="00000000:000076D9"
…
INFO …::compteurs: passe terminée passe="capture+encodage" voie="duplication"
     nombre=1 cadences=[119.5] unites=[1195] verdicts_faux=0
```

🔴 **C'EST EXACTEMENT LÀ QUE LE LOT 30 MESURAIT `0x8000FFFF`.** Le même
constructeur `H264Encoder::new`, sur le même périphérique de capture, sur la
même machine : là où la MFT rendait « Catastrophic failure » et **aucune**
unité, le chemin natif rend **1195 unités d'accès en 10 s**, `verdicts_faux=0`,
et se détruit proprement.

🔵 **Le garde de version a servi, et il n'était pas décoratif.** Le pilote
annonce **`0xd1`** — soit **13.1**, plus récent que la **12.2** transcrite — et
`pilote_compatible` l'accepte. Les deux empaquetages étant différents,
confondre `0xd1` avec `NVENCAPI_VERSION` aurait fait rejeter ce pilote.
⚠️ **Corollaire, à dire clairement** : ce relevé montre qu'un pilote **plus
récent** que l'en-tête fonctionne ; il ne dit **rien** d'un pilote plus ancien
que 12.2, qu'aucune machine ici ne porte.

### 11.2 Le juge que CE lot n'a pas pu relever — et qui l'a relevé ensuite

> 🟢 **DÉPASSÉ, ET PAR LA MAIN DU LOT VOISIN.** Le blocage décrit ci-dessous
> — la sortie virtuelle que Windows ne nomme jamais — était bien la cause, et
> **le lot 32 l'a corrigé puis a relevé le chiffre-juge** :
>
> | Bras | source | `framesDecoded` Δ / 25 s | vidéo | audio | ICE |
> | --- | --- | --- | --- | --- | --- |
> | verte ×2 | mire **animée** | **+494** et **+484** | ~1,2 Mo | ~0,4 Mo | `connected` |
> | rouge ×2 | fenêtre **statique** | **0** | **0** | coule | `connected` |
>
> 🔵 **Le zéro des rouges est interprétable**, et c'est ce qui en fait une
> mesure : dans le **même** relevé l'audio coule et l'ICE est connecté — le
> transport marche, seule la vidéo manque, parce que la source ne change pas.
> C'est le témoin négatif que la mire animée existait pour fournir.
>
> 🔴 **MESURE DU LOT 32, jouée avec DEUX remèdes en place** — cette porte
> NVENC et sa désignation de sortie. Elle établit que le chemin natif produit
> des images qui traversent ; **elle n'a pas été jouée par le lot 31**, et ce
> document ne se l'attribue pas.

**Le chiffre-juge n'a pas pu être relevé PAR CE LOT**, et la cause est
**mesurée, pas supposée** — elle est étrangère à ce lot.

La chaîne a été montée entièrement : plateforme de recette en mode
`motdepasse` (la production est en `pomerium`, dont l'OAuth exige un humain),
compte de recette, VM enrôlée et attribuée, agent enrôlé, page-shell
authentifiée **par le formulaire** (`prefixe posé : S8XrX3jz…`), et une mire
animée à **10 Hz affichant sa propre cadence** — Desktop Duplication n'émettant
qu'au changement du bureau.

**Le superviseur a bien détecté et annoncé les fenêtres, et la shell a bien
demandé ses viewports** (`session=…:w-1`, `w-3`, `w-4`, `w-5`,
`demande="780x492"`). **C'est la création de SORTIE VIRTUELLE qui échoue** :

```
INFO  …::moniteurs_virtuels::pilote: sortie virtuelle créée id=264 … largeur=780 hauteur=492
ERROR …::creation_sortie: aucune sortie neuve n'est apparue dans la limite limite_ms=5000
ERROR …::creation_sortie: la cible n'a jamais été nommée par la configuration
      d'affichage dans la limite — voir moniteurs_virtuels::config_affichage id_pilote=264
ERROR …::creation_sortie: aucune sortie candidate ne peut servir ce viewport
      — elle est rendue au pilote session=…:w-4 demande="780x492" designee="" candidates=[]
```

Le pilote SudoVDA **crée** la sortie ; Windows ne la **nomme** jamais dans sa
configuration d'affichage, donc `candidates=[]` et la session est refusée.
⚠️ **C'est le chemin de désignation des sorties, celui que le lot voisin
travaille — pas l'encodeur, que le §11.1 vient de voir encoder 1195 unités.**

### 11.3 Deux erreurs de mesure que j'ai commises, et corrigées

Elles valent d'être écrites : chacune m'a fait conclure faux un moment.

1. 🔴 **`Get-Process | MainWindowTitle` LU DEPUIS LA SESSION 0 REND UNE CHAÎNE
   VIDE, même pour une fenêtre bien présente.** J'en ai conclu « la mire ne
   s'affiche pas » et j'ai cherché du côté de WinForms et de l'apartment STA.
   **Le témoin qui a tranché est un `notepad.exe`** — lancé exprès, certain
   d'avoir une fenêtre, et rendant lui aussi un titre vide. Une énumération
   `EnumWindows` **exécutée EN session 1** a montré les six fenêtres, mire
   comprise. *Un zéro n'est interprétable qu'avec un témoin négatif.*
2. 🔴 **« LE SUPERVISEUR N'ANNONCE AUCUNE FENÊTRE » ÉTAIT FAUX**, et je l'ai
   cru parce que le chemin d'annonce **ne journalise rien** — pas même en
   `debug`. L'absence de trace n'était pas l'absence d'événement. Ce sont les
   `ERROR` de `creation_sortie`, **plus loin dans le même journal**, qui ont
   montré que les sessions `w-1`…`w-5` existaient bel et bien.

🔵 **Et un contrôle a servi exactement comme prévu** : les cinq champs que
`merite_une_fenetre` juge, relevés un par un en session 1, donnent
`MERITE=True` pour les **six** fenêtres. Ce relevé a écarté « le prédicat les
rejette » avant que j'aille y chercher une cause qui n'y était pas.

### 11.4 Ce que la recette laisse dû

- ✅ ~~🔴 **`framesDecoded` reste à relever.**~~ **RELEVÉ LE 30 AOÛT 2026 —
  PAR LE LOT 32, PAS PAR CELUI-CI** (tableau au § 11.2) : +494 et +484 contre
  0, avec le témoin négatif qui rend ce zéro lisible. La condition que cette
  ligne posait — « le jour où une sortie virtuelle se laisse nommer » — est
  exactement ce que le lot voisin a corrigé. ⚠️ **Le pilote de ce lot
  (`journaux-lot31/instrument/pilote-lot31.mjs`) n'est PAS celui qui a servi**
  : il reste au dépôt pour ce qu'il montre du montage, pas comme la pièce de
  cette mesure.
- ⚠️ **Le mode synchrone de `SessionNvenc` n'est pas éprouvé en régime
  multi-fenêtres** : le banc n'ouvre qu'un encodeur (`nombre=1`). Le plafond
  d'encodeurs NVENC natifs et leur comportement à N fenêtres restent **non
  mesurés**.
- ⚠️ **Aucun jugement d'image n'a été porté** : `verdicts_faux=0` dit que le
  banc n'a relevé aucun verdict FAUX, pas qu'une image est juste — ses
  verdicts sont tous `inconnues`. **Personne n'a regardé une image.**

### 11.5 État de la VM à la fin

Rendue telle que reçue, et **vérifié** plutôt qu'affirmé : `agent.exe` de
production au **même sha256** (`7DB1C0FA…`), `run-agent.ps1` identique à sa
sauvegarde nommée, **trois** agents de production vivants en session 1 (comme
au début), sorties virtuelles purgées, six tâches planifiées `lot31-*`
désenregistrées, **binaire de travail RETIRÉ** — « c'est exactement le fichier
qu'on copie par inadvertance » — et les scripts porteurs d'un `AGENT_SECRET`
effacés. ⚠️ Il reste sous `C:\nivuus\lot31\` **les journaux seuls** ; les trois
occurrences d'`AGENT_SECRET` qui y subsistent sont des messages qui **nomment
la variable**, jamais sa valeur (vérifié). `virsh list` : la VM n'a pas été
redémarrée, **aucune extinction**.

### 11.6 Ce que la production porte, et à quels niveaux de preuve

⚠️ **TROIS REMÈDES COEXISTENT DÉSORMAIS SUR CE PRODUIT, ET ILS NE SE VALENT
PAS.** Les ranger au même niveau serait la faute que ce document passe son
temps à éviter.

| Remède | Niveau de preuve |
| --- | --- |
| **La porte NVENC native** (ce lot) | 🟢 **MESURÉ** — 1195 unités d'accès (§ 11.1), puis des images au navigateur (§ 11.2, mesure du lot 32) |
| **La règle d'appartenance des sorties** (lot 32) | 🟢 **MESURÉ** — voir ses propres relevés |
| **La reprise d'attachement** (lot 32) | 🔴 **NON DÉMONTRÉE** — elle n'a **jamais pu être vue tirer** |

🔴 **« Livré » n'est pas « démontré ».** Le troisième est dans la production
sans qu'aucune mesure ne l'ait vu s'exercer ; c'est le lot 32 qui le dit de
son propre travail, et ce tableau ne fait que refuser de l'aplatir avec les
deux autres.

⚠️ **Et la chaîne complète, par `app.allanic.me`, reste à éprouver par le
propriétaire** : l'OAuth exige un humain, et c'est le seul bras qu'aucun lot
ne peut jouer.

---

## 12. Les pièges que ce lot a payés

**Rassemblés ici pour être trouvables** — le § 11.3 les raconte dans leur
contexte, celui-ci les énonce comme des règles. Aucun n'est théorique : chacun
m'a fait conclure faux, ou aurait pu.

### 12.1 🔴 L'ABSENCE DE TRACE N'EST PAS L'ABSENCE D'ÉVÉNEMENT

**Le plus coûteux du lot, parce qu'il RESSEMBLE à une mesure.** J'ai écrit
« le superviseur n'annonce aucune fenêtre » sur la foi d'un journal muet —
relancé jusqu'en `RUST_LOG=debug` pour être sûr. **C'était faux.** Le chemin
d'annonce (`Effet::AnnoncerOuverture` → `envoyer`) **ne journalise rien du
tout**, à aucun niveau. Les fenêtres étaient bien détectées, annoncées, et la
page-shell demandait ses viewports : ce sont les `ERROR` d'un **autre** module,
plus loin dans le même journal, qui l'ont montré.

**La règle** : un journal muet sur un chemin *qui n'écrit rien* ne dit pas que
le chemin n'a pas couru — il ne dit **rien**. Avant d'en conclure quoi que ce
soit, établir que ce chemin **écrirait** s'il courait. C'est le cousin de
« un zéro n'est interprétable qu'avec un témoin négatif », appliqué aux traces
plutôt qu'aux compteurs.

### 12.2 🔴 `MainWindowTitle` LU DEPUIS LA SESSION 0 REND UNE CHAÎNE VIDE

`Get-Process | Select MainWindowTitle`, exécuté par WinRM (donc en session 0),
rend **vide** pour une fenêtre pourtant bien présente en session 1. J'en ai
conclu que ma mire ne s'affichait pas, et je suis parti chercher du côté de
WinForms et de l'apartment STA — deux impasses.

**Ce qui a tranché est un témoin négatif** : un `notepad.exe` lancé exprès,
dont personne ne doute qu'il ait une fenêtre, et qui rendait lui aussi un
titre vide. **Le relevé qui vaut est une énumération `EnumWindows` exécutée
EN session 1.**

### 12.3 ⚠️ UN MÉNAGE EN `finally` PEUT REMPLACER LA VRAIE ERREUR

Le pilote de recette échouait en affichant `ENOTEMPTY: rmdir` — son propre
nettoyage de profil Chrome, levant depuis le `finally` et **écrasant** la
cause réelle (un simple nom de méthode). Le diagnostic est parti du mauvais
côté pendant deux essais. **Le ménage ne doit jamais pouvoir lever.**

### 12.4 🔴 UNE ROUGE RESTÉE VERTE SE DIAGNOSTIQUE, ELLE NE SE CLASSE PAS

Détail en § 10.6 : ramener un tableau `reserved` de 278 à 277 `u32` n'a rien
fait rougir, et j'ai d'abord écrit que le contrôle était faible. **Mesure
faite, la disposition obtenue est octet pour octet la MÊME** — le remplissage
d'alignement absorbe les quatre octets. Ce n'était pas un défaut qui passe,
c'était un **non-défaut**, et rester vert était la bonne réponse. **C'était mon
affirmation qui était fausse, pas le contrôle.**

### 12.5 ⚠️ MESURER UNE DISPOSITION SUR L'HÔTE POUR UNE CIBLE ÉTRANGÈRE

Les tailles de l'ABI NVENC ont d'abord été mesurées **sur Linux** alors que la
cible est `x86_64-pc-windows-gnu`. « Ça devrait être identique » n'est pas
« c'est mesuré » : c'est le patron *réutiliser la sortie d'une commande pour
répondre à la question d'une autre*. Remplacé par 29 `_Static_assert`
compilées par **mingw** — sans exécution, donc rejouables par quiconque.

### 12.6 ⚠️ UN IMPORT STATIQUE D'UN SYMBOLE ABSENT EMPÊCHE TOUT CHARGEMENT

Lier `MFTEnum2` statiquement alors qu'il n'existe pas sous ce nom a rendu la
sonde **incapable de démarrer** (`0xC0000139`,
`STATUS_ENTRYPOINT_NOT_FOUND`) — avant la première ligne de `main`, et le
symptôme se lit comme un plantage de la sonde. Résoudre par `GetProcAddress`.

### 12.7 ⚠️ UN NOM DXGI PERD SES ANTISLASHS EN TRAVERSANT LES COUCHES

`\\.\DISPLAY6` passé de bash à pywinrm à PowerShell à `schtasks` arrive en
`\.\DISPLAY6`, et l'agent répond « aucune sortie DXGI nommée » — un message
juste, sur une valeur qu'on n'a pas envoyée. **Construire le nom au plus près
de son usage**, pas à l'autre bout de la chaîne.

---

## 13. Le plafond d'encodeurs NVENC à N fenêtres — protocole PRÉPARÉ, NON JOUÉ

🔴 **RIEN DE CE QUI SUIT N'A ÉTÉ EXÉCUTÉ.** C'est un protocole, écrit pendant
que la VM appartient au lot voisin, pour être joué tel quel quand elle se
libère. Il ne rapporte aucun chiffre, et n'en suggère aucun.

**Pourquoi il compte.** C'est lui qui décidera **combien de fenêtres le produit
peut réellement servir** — la question que le chantier D a contournée sans
jamais l'expliquer : son plafond de **8 encodeurs** MFT est resté une des
« trois couches inconnues ». Le dos natif est une conception différente ; rien
n'autorise à lui prêter le même plafond, ni un autre.

### 13.1 Le chiffre-juge, et ce qu'il n'est pas

| | |
| --- | --- |
| **Chiffre-juge** | le nombre d'instances qui **produisent encore des unités d'accès**, pas le nombre qui se crée |
| **Relevé** | `unites=[…]` et `cadences=[…]`, **une entrée par instance**, dans la ligne `passe terminée` du banc |
| **Rouge attendu** | le rang **N** où la création échoue, **avec son `NVENCSTATUS` nommé** — pas un code de retour, la valeur |
| **Témoin négatif** | **N = 1**, connu pour rendre ~1195 unités en 10 s (§ 11.1) : si N=1 ne le rend plus, la machine a changé et la campagne ne mesure pas ce qu'on croit |

🔴 **UN ENCODEUR QUI SE CRÉE NE COMPTE PAS.** Huit instances construites dont
trois muettes valent trois, pas huit — et c'est exactement ce qu'un compte de
créations ferait lire à l'envers. **La condition est `unites > 0` pour
CHAQUE instance retenue.**

### 13.2 Le montage

```text
MULTIFENETRE_BANC=duplication  MULTIFENETRE_N=<1..8>  MULTIFENETRE_SORTIE=\\.\DISPLAY<n>
```

- ⚠️ **La sortie doit être ATTACHÉE et portée par l'adaptateur NVIDIA** :
  c'est l'adaptateur de la sortie capturée qui porte le périphérique D3D11, et
  donc la session NVENC. Le relever par la ligne `sortie retenue pour la
  duplication` **avant** de conclure quoi que ce soit du résultat.
- ⚠️ **Construire le nom `\\.\DISPLAY<n>` DANS le script distant** — § 12.7.
- ⚠️ **`Get-Process` avant CHAQUE tentative, y compris échouée**, et **purge**
  (`MULTIFENETRE_VDD_PURGE=1`) entre deux exécutions.
- 🔵 **LES CONDITIONS ONT CHANGÉ DEPUIS QUE CE PROTOCOLE A ÉTÉ ÉCRIT, et
  c'est le lot 32 qui l'a établi** : le vivier de sorties virtuelles est de
  **dix**, et il était **entièrement consommé** par des fenêtres que personne
  n'avait demandées. Une règle d'appartenance est désormais livrée — `desk`
  n'adopte que ce qu'il lance — et **cinq sorties suffisent là où dix étaient
  épuisées**. ⚠️ **Conséquence directe pour ce protocole** : un plafond
  mesuré AVANT cette règle aurait pu être celui du **vivier de sorties**, pas
  celui des **encodeurs** — deux choses différentes qu'un seul chiffre aurait
  confondues. Le relever à nouveau aujourd'hui n'a pas le même sens qu'hier.
- ⚠️ **Deux exécutions par valeur de N**, et **N croissant puis décroissant** :
  un plafond de *créations cumulées* ne se distingue d'un plafond de
  *concurrence* que si l'on redescend. C'est la leçon de
  `MULTIFENETRE_NVENC_CYCLES` au sous-bloc D5, et elle vaut ici sans changement.

### 13.3 Ce que ce protocole ne mesurera PAS, et qu'il ne faut pas lui faire dire

- **Il ne juge aucune image** : le banc rend `verdicts` tous `inconnues`.
  Il compte des unités d'accès, pas des images justes.
- **Il ne dit rien du produit réel à N fenêtres** : le banc ouvre N encodeurs
  sur **une seule** sortie capturée, là où le produit ouvre une capture **par
  fenêtre**, dans **N processus distincts**. C'est un plafond d'encodeurs, pas
  un plafond de sessions.
- **Il n'éprouve pas `regler_debit`**, jamais appelé sur la machine à ce jour.
