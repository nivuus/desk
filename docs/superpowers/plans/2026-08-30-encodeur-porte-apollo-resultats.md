# Lot 31 — par quelle porte Apollo obtient ses encodeurs, et laquelle reste ouverte à `desk`

**30 août 2026.** Dépôt `desk`, branche `package-nivuus`. VM `Windows`
(libvirt), **définition inchangée, VGA QEMU en place** — c'est la condition
dans laquelle Apollo réussit, donc la condition de toute mesure de ce lot.

> 🔴 **CE DOCUMENT NE MODIFIE AUCUN CODE DE PRODUIT.** Les remèdes sont
> **nommés avec leur `fichier:ligne`** et laissés à la décision du
> propriétaire du dépôt. Les seuls artefacts écrits sont des **sondes
> jetables** (`/var/tmp/lot31/`, et `C:\nivuus\lot31\` sur la VM).

---

## 0. Le résultat en une phrase

Apollo n'emprunte **pas** la porte que `desk` emprunte : il appelle l'**API
NVENC native** (`nvEncodeAPI64.dll`), là où `desk` passe par la **MFT Media
Foundation de NVIDIA** — et cette MFT, **mesurée aujourd'hui**, refuse de
s'activer en **session 1** sur cette machine (`0x8000FFFF`) alors qu'elle
s'active en **session 0**, dans le même binaire, à la même minute, sur les
quatre arrangements que l'API Media Foundation permet d'essayer.

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
| `agent/src/encode/fabrique.rs:280` | `find_hardware_encoder()` |
| `agent/src/encode/fabrique.rs:294` | `MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_HARDWARE \| MFT_ENUM_FLAG_SORTANDFILTER, NV12 → H264)` — **aucun CLSID n'est nommé** : c'est une énumération, et elle rend **une** entrée, `NVIDIA H.264 Encoder MFT` |
| `agent/src/encode/fabrique.rs:351-352` | 🔴 **LA PORTE** : `first.ActivateObject()` → `0x8000FFFF` en session 1 |
| `agent/src/encode.rs:374` | l'appel, depuis `H264Encoder::new` |
| `agent/src/encode.rs:378-380` | `bail!` si la MFT n'est **pas asynchrone** — ce qui **exclut d'office** la MFT logicielle (épreuve E, `async=0`) |
| `agent/src/encode.rs:391-398` | `share_device` + `MFT_MESSAGE_SET_D3D_MANAGER` — **APRÈS** l'activation |
| `agent/src/capture/ouverture.rs:125-131` | le périphérique D3D11, créé **sur l'adaptateur qui porte la sortie capturée** |

**Ce que cette porte suppose et que l'autre ne suppose pas** : que le pilote
NVIDIA veuille bien instancier **son objet COM d'encodeur enregistré dans le
magasin Media Foundation**, dans le contexte de session/station où on le lui
demande. La porte native ne suppose que `LoadLibrary` + `GetProcAddress` +
un périphérique D3D11 — **mesuré disponible en session 1** (épreuve H).

🔴 **Et une supposition tombe, mesurée** : `MFT_MESSAGE_SET_D3D_MANAGER` ne
peut **pas** être en cause. La documentation Microsoft l'impose *après*
l'activation et *avant* `SetInputType`/`SetOutputType`, et `encode.rs:391-398`
le fait bien après `encode.rs:374`. On échoue **avant** d'avoir la moindre
occasion de désigner un périphérique.

🔴 **TROUVAILLE COLLATÉRALE, NON CHERCHÉE, ET QUI DÉBORDE LARGEMENT CE
LOT — elle ne parle pas d'un chemin hypothétique, elle parle de la
PRODUCTION D'AUJOURD'HUI.** L'épreuve F montre qu'**aucun convertisseur de
couleur MATÉRIEL n'est énuméré** sur cette machine, **dans les deux
sessions**. Donc `find_hardware_video_processor()` (`encode/fabrique.rs:194`) échoue
**toujours**, et `create_color_converter` (`encode/fabrique.rs:101`) retombe
**toujours** sur son `CoCreateInstance(&CLSID_VideoProcessorMFT)`
(`encode/fabrique.rs:118`) — c'est-à-dire sur le **`Microsoft Video Processor MFT`,
LOGICIEL** (épreuve G, activé OK).

**Ce que cela veut dire, en clair : la conversion BGRA → NV12 de chaque
image de chaque fenêtre passe par le CPU, sur cette VM, depuis toujours — et
personne dans ce dépôt ne le savait.** Le commentaire d'en-tête d'`encode.rs`
décrit ce convertisseur comme celui qui « convertit BGRA→NV12 **sans quitter
le GPU** » : cette phrase est **fausse sur cette machine**. Le repli de
`encode/fabrique.rs:117` journalise pourtant sa raison en `debug!`, un niveau que la
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
- 🔴 **Aucun remède n'a été JOUÉ.** Aucune ligne de produit n'a été modifiée.
- ⚠️ **La variante « un seul affichage » n'a pas été essayée** — elle exige de
  désactiver `\\.\DISPLAY1`, donc de retirer au VGA QEMU son rôle, ce que la
  consigne de ce lot interdit.
- ⚠️ **Le confondant session/topologie n'est pas démêlé** : entre session 0 et
  session 1, **deux** choses changent — la session, et le fait qu'un bureau
  avec un affichage primaire non-NVIDIA existe. Aucune mesure de ce lot ne les
  sépare.

---

## 5. Les remèdes, chacun avec son coût et son degré de preuve

### R1 — l'API NVENC native, la porte d'Apollo · 🟢 **précédent MESURÉ ici**

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
  directement. Le convertisseur de couleur (`encode/fabrique.rs:101-133`) — qui, sur
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

- **Où** : `agent/src/encode/fabrique.rs:280-355` (un second `MFTEnumEx` avec
  `MFT_ENUM_FLAG_SYNCMFT` en repli) **et** `agent/src/encode.rs:378-380`, dont
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

🔴 **Aucune n'a été appliquée.**

1. **R1 — implémenter l'encodeur NVENC natif.** Point d'entrée :
   `agent/src/encode/fabrique.rs:280-355` (`find_hardware_encoder`) et
   `agent/src/encode.rs:374` (son appelant), plus un module neuf pour la
   liaison FFI. Effet de bord souhaité : `agent/src/encode/fabrique.rs:101-133`
   (le convertisseur) pourrait sortir du chemin chaud.
   ⚠️ `encode.rs` pèse **1536 lignes** — la règle des 500 impose que toute
   addition substantielle s'accompagne d'une **extraction**, dans une tâche
   dédiée et **avant** celle qui ajoute.
2. **R3 — repli logiciel, en filet.** `agent/src/encode/fabrique.rs:280-355` **et**
   `agent/src/encode.rs:378-380` (le `bail!` sur `is_async == 0`), qui
   doivent changer **ensemble**.
3. **Le relevé qui manque (§4).** Autorisation d'un des trois chemins :
   ① modifier `perm` de `nivuus-hote` dans
   `C:\Program Files\Apollo\config\sunshine_state.json` (sauvegarde nommée,
   restauration après) ; ② `Restart-Service ApolloService` ; ③ le vrai mot de
   passe de l'interface web d'Apollo.

---

## 7. État de la VM à la fin de ce lot

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

Le `bail!` d'`encode.rs:380` n'est que le premier des obstacles, et le moins
cher. Une MFT **synchrone** — ce qu'est l'encodeur logiciel, mesuré `async=0`
à l'épreuve E — diverge de l'asynchrone sur **tout le pilotage** :

| Ce que le code fait aujourd'hui | Pourquoi une MFT synchrone ne le supporte pas |
| --- | --- |
| `encode.rs:404` : `let events: IMFMediaEventGenerator = transform.cast()?;` — **inconditionnel**, et le champ `events` de `H264Encoder` n'est **pas** une `Option` | une MFT synchrone n'expose aucun générateur d'événements ; le `cast` échoue, donc `new()` échoue **avant** même d'atteindre le `bail!` |
| `encode.rs:382` : `SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1)` | sans objet |
| `encode.rs:409` : `file_encodeur.confier(&transform, "encodeur")` | `arret::FileMft` existe pour poser **une barrière sur le travail ASYNCHRONE** de la MFT — son propre commentaire le dit. Sans travail asynchrone, l'apparat est inapplicable, et `Drop` s'appuie dessus (`arret::mettre_au_repos`) |
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
(`agent/src/encode/fabrique.rs:351-352`), en nommant la cause connue et le
document qui l'établit, plutôt que de laisser un `0x8000FFFF` nu remonter la
pile. Cela ne fait pas marcher le produit — **et ce n'est pas présenté comme
tel** —, cela évite qu'un prochain lecteur repaie les lots 30 et 31.
Autorisation non demandée ici : c'est une proposition, pas un plan.
