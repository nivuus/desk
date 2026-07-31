# Canal de contrôle du pilote d'affichage virtuel — reconnaissance

Chantier D, tâche 3 (2026-07-31). Ne produit pas de code : établit **comment**
on commande le pilote « SudoMaker Virtual Display Adapter » (SudoVDA) déjà
installé et sain sur la VM (`ROOT\DISPLAY\0003`), sans dépendre du démarrage
d'une session Apollo.

**Verdict : forme B — canal par IOCTL.** Détail complet en fin de document.
Confiance élevée : le GUID d'interface et deux des six codes IOCTL sont
vérifiés **par présence d'octets** dans le binaire installé sur cette VM, pas
seulement lus en amont.

---

## 1. Fichiers du pilote localisés (fait local)

```
$ find /media/vm/Windows/System32/DriverStore/FileRepository -iname 'sudovda*'
/media/vm/Windows/System32/DriverStore/FileRepository/sudovda.inf_amd64_b30b37ad037ba94a/sudovda.cat
/media/vm/Windows/System32/DriverStore/FileRepository/sudovda.inf_amd64_b30b37ad037ba94a/SudoVDA.dll
/media/vm/Windows/System32/DriverStore/FileRepository/sudovda.inf_amd64_b30b37ad037ba94a/SudoVDA.inf

$ find '/media/vm/Program Files' '/media/vm/Program Files (x86)' -maxdepth 4 -iname '*.dll' | grep -i -E 'apollo|sunshine|vda'
/media/vm/Program Files/Apollo/drivers/sudovda/SudoVDA.dll
```

Le répertoire `/media/vm/Program Files/Apollo/drivers/sudovda/` contient aussi
`install.bat`, `uninstall.bat`, `nefconc.exe` (outil Nefarius de création de
device node / installation INF, **pas** un outil de pilotage runtime),
`sudovda.cer`, `sudovda.cat`, `SudoVDA.inf`.

Les deux copies de `SudoVDA.dll` (DriverStore et `Program Files\Apollo\...`)
sont **bit-à-bit identiques** :

```
$ md5sum '/media/vm/Windows/.../SudoVDA.dll' '/media/vm/Program Files/Apollo/drivers/sudovda/SudoVDA.dll'
200ec71b297ca42469652256c4fa896b  .../DriverStore/.../SudoVDA.dll
200ec71b297ca42469652256c4fa896b  .../Program Files/Apollo/drivers/sudovda/SudoVDA.dll
```

`install.bat` (fait local, lu intégralement) confirme le hardware ID et la
classe déclarés à l'installation :

```
nefconc.exe --remove-device-node --hardware-id root\sudomaker\sudovda --class-guid "4D36E968-E325-11CE-BFC1-08002BE10318"
nefconc.exe --create-device-node --class-name Display --class-guid "4D36E968-E325-11CE-BFC1-08002BE10318" --hardware-id root\sudomaker\sudovda
nefconc.exe --install-driver --inf-path "SudoVDA.inf"
```

## 2. L'INF (fait local) — aucune interface de périphérique déclarée

`SudoVDA.inf` est encodé en **UTF-16LE** (confirmé par `file` : « Windows
setup INFormation »). `grep` fonctionne directement dessus dans cet
environnement (testé, pas besoin d'`iconv` ici — contrairement aux journaux
`.log` d'un chantier précédent).

Contenu intégral pertinent :

```ini
[Version]
ClassGUID = {4D36E968-E325-11CE-BFC1-08002BE10318}
Class = Display
...
[SudoVDA_Install.NT.hw]
AddReg = MyDevice_HardwareDeviceSettings

[MyDevice_HardwareDeviceSettings]
HKR,, "UpperFilters", %REG_MULTI_SZ%, "IndirectKmd"
HKR, "WUDF", "DeviceGroupId", %REG_SZ%, "SudoVDAGroup"
HKR,, "Security",, "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;WD)"

[SudoVDA_Install.NT.Services]
AddService = WUDFRd,0x000001fa,WUDFRD_ServiceInstall

[SudoVDA_Install.NT.Wdf]
UmdfService = SudoVDA, SudoVDA_Install
UmdfServiceOrder = SudoVDA
UmdfKernelModeClientPolicy = AllowKernelModeClients

[SudoVDA_Install]
UmdfLibraryVersion=2.25.0
ServiceBinary=%12%\UMDF\SudoVDA.dll
UmdfExtensions = IddCx0102
```

**Aucune section `AddInterface` ni GUID d'interface de périphérique n'est
déclarée dans l'INF.** C'est attendu pour un pilote UMDF/IddCx (extension
`IddCx0102`, chargé via `WUDFRd.sys`, le réflecteur WDF) : ce type de pilote
enregistre ses interfaces de périphérique **au runtime**, dans son code, pas
via une directive INF statique. L'absence oriente donc vers l'hypothèse B,
sans la prouver — c'est cohérent avec elle, pas une preuve suffisante à elle
seule.

## 3. Exports de la DLL — hypothèse A écartée pour ce fichier

`objdump -x` (sortie en français dans cet environnement — chercher « Table
d'adresses d'exportation », pas l'anglais « Export Address Table ») :

```
$ objdump -x SudoVDA.dll | sed -n '/Table d.adresses d.exportation/,/^$/p'
Table d'adresses d'exportation -- base de nombre ordinal 1
          Ordinal  Address  Type
[   0] +base[   1] 00007190 Exportation RVA

Table [Ordinal/Nom de pointeur] -- Base Ordinal 1
          Ordinal   Hint Name
[   0] +base[   1]  0000 FxDriverEntryUm
```

**Un seul export : `FxDriverEntryUm`.** C'est le point d'entrée générique
requis par tout pilote UMDF (`FxDriverEntryUm` = symbole standard du framework
WDF, injecté par le linker WDF, pas un point d'entrée métier). Aucun symbole
de la famille `AddVirtualDisplay` / `CreateMonitor` / `RemoveVirtualDisplay`
n'est exporté par cette DLL — **hypothèse A (contrôle par appel de fonction
exportée de cette DLL) est écartée pour ce fichier précis.**

Import table (`objdump -x`) : `ntdll.dll`, `KERNEL32.dll`, `ADVAPI32.dll`,
`ole32.dll`, `dxgi.dll`, `d3d11.dll`, `AVRT.dll`, `api-ms-win-crt-*.dll`. Pas
d'import de `SETUPAPI.dll` ni de `DeviceIoControl` — cohérent : c'est le
pilote lui-même (le serveur de l'IOCTL), pas un client.

Recherche de chaînes dans la DLL (`strings -a`) : **aucun GUID** au format
texte, **aucune** chaîne `ioctl`/`DeviceIoControl`/`CreateFile`. Une seule
chaîne liée au domaine : `IndirectMonitorContextWrapper` (nom de classe
interne, cohérent avec l'exemple IddCx de Microsoft). Absence de chaînes
utile en soi : les GUID/IOCTL numériques n'ont pas besoin d'être des chaînes
de texte pour exister dans le binaire (voir §5, où ils sont retrouvés en tant
que **séquences d'octets**, pas de texte).

## 4. Le client réel du pilote : `Apollo\sunshine.exe` (fait local)

`nefconc.exe` (Nefarius) ne sert qu'à l'**installation** du device node — pas
au pilotage runtime (imports : `SETUPAPI.dll`, aucune trace de
`DeviceIoControl`). Le véritable client runtime est `sunshine.exe`
(l'exécutable d'Apollo, fork de Sunshine) :

```
$ file '/media/vm/Program Files/Apollo/sunshine.exe'
... PE32+ executable ... x86-64 (stripped to external PDB) ...
```

Imports pertinents (`objdump -x`, confirmés présents) :

```
SETUPAPI.dll: SetupDiGetClassDevsA, SetupDiGetClassDevsW,
              SetupDiEnumDeviceInterfaces,
              SetupDiGetDeviceInterfaceDetailA, SetupDiGetDeviceInterfaceDetailW,
              SetupDiGetDeviceInstanceIdW, SetupDiOpenDevRegKey,
              SetupDiDestroyDeviceInfoList
KERNEL32.dll: CreateFileA, CreateFileW, DeviceIoControl
```

C'est exactement le schéma canal-IOCTL standard Windows : énumération d'une
interface de périphérique par GUID (`SetupDiGetClassDevs` +
`SetupDiEnumDeviceInterfaces` + `SetupDiGetDeviceInterfaceDetail`), ouverture
du chemin obtenu (`CreateFile`), puis `DeviceIoControl`.

Chaînes de log extraites du binaire (`strings -a`), toutes préfixées
`[SUDOVDA]` :

```
[SUDOVDA] Open device failed!
[SUDOVDA] SUDOVDA protocol not compatible with driver!
[SUDOVDA] Watchdog fetch failed!
[SUDOVDA] Watchdog: Timeout %d, Countdown %d
[SUDOVDA] Configuration: W: %d, H: %d, FPS: %d
[SUDOVDA] Failed to add virtual display.
[SUDOVDA] Cannot get name for newly added virtual display!
[SUDOVDA] Virtual display removed successfully.
```

`sunshine.exe` étant **stripped** (PDB externe absent du poste), **aucun GUID
ni code IOCTL numérique n'est récupérable en clair depuis ce binaire** — ce
sont des constantes compilées en immédiats, invisibles à `strings`. C'est ce
qui a motivé l'étape suivante (source amont), conformément à l'étape 4 du
brief.

## 5. Source amont — GUID, IOCTL, structures (lu en amont, puis confirmé localement)

### 5.1 Provenance

Le pilote est publié par SudoMaker : `https://github.com/SudoMaker/SudoVDA`.
Il n'expose pas de release taggée corrélable à la version installée
(`DriverVer = 07/14/2025, 1.10.9.289` dans l'INF local) — la page Releases de
ce dépôt est vide au moment du relevé. La correspondance de version n'est donc
**pas établie par numéro de version** ; elle l'est par confirmation binaire
directe (§5.3), ce qui est plus fort.

Le fork Apollo (`https://github.com/ClassicOldSong/Apollo`, dépôt dont
l'exécutable local `sunshine.exe` est manifestement issu — même chaînes de
log `[SUDOVDA] ...` retrouvées mot pour mot, voir §5.4) vendorise une copie
des en-têtes SudoVDA sous `third-party/sudovda/` :

- `https://raw.githubusercontent.com/ClassicOldSong/Apollo/master/third-party/sudovda/sudovda-ioctl.h`
- `https://raw.githubusercontent.com/ClassicOldSong/Apollo/master/third-party/sudovda/sudovda.h`
- Point d'appel réel dans Apollo, retrouvé par recherche de code (`OpenDevice(`) :
  `https://github.com/ClassicOldSong/Apollo/blob/master/src/platform/windows/virtual_display.cpp`

Dernière modification connue de `sudovda-ioctl.h` dans Apollo : commit
`fd037c48` du 2024-09-08 (« Update driver »), signé et vérifié par GitHub —
soit environ onze mois avant le pilote installé ici (`DriverVer =
07/14/2025, 1.10.9.289`). **Ce fait est lu en amont.**

La vérification par octets du §5.3 referme une partie de cet écart, mais une
partie **seulement**. Ce qu'elle établit, dans le binaire réellement installé
sur cette VM : le GUID d'interface `SUVDA_INTERFACE_GUID` et 2 des 6 codes
IOCTL (`IOCTL_ADD_VIRTUAL_DISPLAY`, `IOCTL_GET_WATCHDOG`) — trois constantes
numériques isolées, qui n'ont aucune raison de changer tant que le contrat
externe du pilote reste rétrocompatible. Ce qu'elle **n'établit pas** :
la disposition des structures de tampon du §5.2 (ordre des champs, tailles,
présence de champs ajoutés). Rien, dans cette vérification, n'exclut qu'un
champ ait été ajouté ou réordonné dans `VIRTUAL_DISPLAY_ADD_PARAMS` ou une
autre structure entre septembre 2024 et juillet 2025 sans que le GUID ni les
codes IOCTL n'aient eux-mêmes besoin de changer. Cette réserve est reprise
telle quelle au §5.2 et au verdict du §6 — c'est là que la tâche 5 la lira.

### 5.2 GUID, hardware ID, IOCTL, structures (lu en amont, texte intégral)

```c
static const char* SUVDA_HARDWARE_ID = "root\\sudomaker\\sudovda";

// {4d36e968-e325-11ce-bfc1-08002be10318}  — classe Display standard Windows,
// confirmée aussi dans l'INF local (§2)
static const GUID SUVDA_CLASS_GUID = { 0x4d36e968, 0xe325, 0x11ce, { 0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18 } };

// {e5bcc234-1e0c-418a-a0d4-ef8b7501414d}  — INTERFACE DE PÉRIPHÉRIQUE,
// c'est CELUI-CI qu'il faut passer à SetupDiGetClassDevs / CM_Get_Device_Interface_List
static const GUID SUVDA_INTERFACE_GUID = { 0xe5bcc234, 0x1e0c, 0x418a, { 0xa0, 0xd4, 0xef, 0x8b, 0x75, 0x01, 0x41, 0x4d } };

#define IOCTL_ADD_VIRTUAL_DISPLAY     CTL_CODE(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_REMOVE_VIRTUAL_DISPLAY  CTL_CODE(FILE_DEVICE_UNKNOWN, 0x801, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_SET_RENDER_ADAPTER      CTL_CODE(FILE_DEVICE_UNKNOWN, 0x802, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_GET_WATCHDOG            CTL_CODE(FILE_DEVICE_UNKNOWN, 0x803, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_DRIVER_PING             CTL_CODE(FILE_DEVICE_UNKNOWN, 0x888, METHOD_BUFFERED, FILE_ANY_ACCESS)
#define IOCTL_GET_PROTOCOL_VERSION    CTL_CODE(FILE_DEVICE_UNKNOWN, 0x8FF, METHOD_BUFFERED, FILE_ANY_ACCESS)

typedef struct _SUVDA_PROTOCAL_VERSION {
    uint8_t Major;
    uint8_t Minor;
    uint8_t Incremental;
    bool TestBuild;
} SUVDA_PROTOCAL_VERSION, *PSUVDA_PROTOCAL_VERSION;

static const SUVDA_PROTOCAL_VERSION VDAProtocolVersion = { 0, 2, 1, true };

typedef struct _VIRTUAL_DISPLAY_ADD_PARAMS {
    UINT Width;
    UINT Height;
    UINT RefreshRate;
    GUID MonitorGuid;
    CHAR DeviceName[14];
    CHAR SerialNumber[14];
} VIRTUAL_DISPLAY_ADD_PARAMS, *PVIRTUAL_DISPLAY_ADD_PARAMS;

typedef struct _VIRTUAL_DISPLAY_REMOVE_PARAMS {
    GUID MonitorGuid;
} VIRTUAL_DISPLAY_REMOVE_PARAMS, *PVIRTUAL_DISPLAY_REMOVE_PARAMS;

typedef struct _VIRTUAL_DISPLAY_ADD_OUT {
    LUID AdapterLuid;
    UINT TargetId;
} VIRTUAL_DISPLAY_ADD_OUT, *PVIRTUAL_DISPLAY_ADD_OUT;

typedef struct _VIRTUAL_DISPLAY_SET_RENDER_ADAPTER_PARAMS {
    LUID AdapterLuid;
} VIRTUAL_DISPLAY_SET_RENDER_ADAPTER_PARAMS, *PVIRTUAL_DISPLAY_SET_RENDER_ADAPTER_PARAMS;

typedef struct _VIRTUAL_DISPLAY_GET_WATCHDOG_OUT {
    UINT Timeout;
    UINT Countdown;
} VIRTUAL_DISPLAY_GET_WATCHDOG_OUT, *PVIRTUAL_DISPLAY_GET_WATCHDOG_OUT;

typedef struct _VIRTUAL_DISPLAY_GET_PROTOCOL_VERSION_OUT {
    SUVDA_PROTOCAL_VERSION Version;
} VIRTUAL_DISPLAY_GET_PROTOCOL_VERSION_OUT, *PVIRTUAL_DISPLAY_GET_PROTOCOL_VERSION_OUT;
```

Toutes les structures sont **sans `#pragma pack`** dans le source amont : donc
alignement naturel MSVC/x64 par défaut. Tailles/dispositions dérivées (voir
§7 pour la traduction Rust `#[repr(C)]`) :

| Structure | Taille | Alignement | Détail des offsets |
| --- | --- | --- | --- |
| `VIRTUAL_DISPLAY_ADD_PARAMS` | 56 octets | 4 | Width@0, Height@4, RefreshRate@8, MonitorGuid@12 (16 o.), DeviceName@28 (14 o.), SerialNumber@42 (14 o.) → 56, déjà multiple de 4, pas de padding final |
| `VIRTUAL_DISPLAY_REMOVE_PARAMS` | 16 octets | 4 | MonitorGuid@0 |
| `VIRTUAL_DISPLAY_ADD_OUT` | 12 octets | 4 | AdapterLuid@0 (LUID = 2×`u32`/`i32`, 8 o.), TargetId@8 |
| `VIRTUAL_DISPLAY_SET_RENDER_ADAPTER_PARAMS` | 8 octets | 4 | AdapterLuid@0 |
| `VIRTUAL_DISPLAY_GET_WATCHDOG_OUT` | 8 octets | 4 | Timeout@0, Countdown@4 |
| `VIRTUAL_DISPLAY_GET_PROTOCOL_VERSION_OUT` | 4 octets | 1 | Version@0 (4×`u8`, `TestBuild` = `bool` MSVC = 1 octet) |

`GUID` Win32 = `{ DWORD Data1; WORD Data2; WORD Data3; BYTE Data4[8]; }` (16
octets, alignement 4). `LUID` = `{ DWORD LowPart; LONG HighPart; }` (8 octets,
alignement 4).

> **Réserve sur cette section entière — lue en amont, non confirmée par
> octets.** Le §5.3 confirme, dans le binaire `SudoVDA.dll` réellement
> installé sur cette VM, le GUID d'interface et 2 des 6 codes IOCTL — trois
> constantes numériques isolées. Il ne confirme **rien** sur la disposition
> ci-dessus : ni l'ordre des champs, ni leurs tailles, ni l'absence de champ
> ajouté depuis. Ces structures proviennent d'un en-tête dont la dernière
> modification connue (2024-09-08) précède d'environ onze mois le pilote
> installé (`DriverVer 07/14/2025`, `1.10.9.289`). **La tâche 5 doit traiter
> ces structures comme une lecture amont non confirmée localement** — pas
> comme un fait vérifié au même titre que le GUID ou les deux codes IOCTL du
> §5.3. Voir la fin du §5.3 pour une piste bon marché de confirmation
> empirique avant d'engager le tampon le plus riche
> (`VIRTUAL_DISPLAY_ADD_PARAMS`, 56 octets).

### 5.3 Vérification croisée sur le binaire local (fait local — pas seulement lu)

Le GUID d'interface et deux codes IOCTL calculés à partir des macros
ci-dessus ont été recherchés **en tant que séquences d'octets brutes**
(little-endian, disposition mémoire réelle d'un `GUID` Win32) dans le
`SudoVDA.dll` **effectivement installé sur cette VM**
(`/media/vm/Windows/System32/DriverStore/FileRepository/sudovda.inf_amd64_b30b37ad037ba94a/SudoVDA.dll`,
md5 `200ec71b297ca42469652256c4fa896b`) :

```python
guid_bytes = bytes([0x34,0xc2,0xbc,0xe5, 0x0c,0x1e, 0x8a,0x41, 0xa0,0xd4,0xef,0x8b,0x75,0x01,0x41,0x4d])
data.find(guid_bytes)   # -> 53656   (TROUVÉ, occurrence unique attendue vu la longueur — 16 octets)
```

**`SUVDA_INTERFACE_GUID` est présent tel quel dans le binaire installé.** Une
correspondance fortuite sur 16 octets alignés est négligeable statistiquement
— c'est une confirmation directe, pas une coïncidence.

Codes IOCTL calculés (`CTL_CODE(FILE_DEVICE_UNKNOWN=0x22, Function, METHOD_BUFFERED=0, FILE_ANY_ACCESS=0)`
= `(0x22 << 16) | (Function << 2)`) puis recherchés comme DWORD little-endian :

| Constante | Valeur calculée | Trouvée dans le `.dll` local ? |
| --- | --- | --- |
| `IOCTL_ADD_VIRTUAL_DISPLAY` | `0x00222000` | **Oui**, offset 16316 |
| `IOCTL_REMOVE_VIRTUAL_DISPLAY` | `0x00222004` | Non trouvée telle quelle |
| `IOCTL_SET_RENDER_ADAPTER` | `0x00222008` | Non trouvée telle quelle |
| `IOCTL_GET_WATCHDOG` | `0x0022200C` | **Oui**, offset 16284 |
| `IOCTL_DRIVER_PING` | `0x00222220` | Non trouvée telle quelle |
| `IOCTL_GET_PROTOCOL_VERSION` | `0x002223FC` | Non trouvée telle quelle |

Deux codes sur six confirmés par octets, à des offsets voisins (16284 et
16316 — cohérent avec une suite de comparaisons dans une routine de
dispatch). Les quatre autres ne sont **pas** absents à cause d'une erreur
d'hypothèse : le compilateur peut encoder une comparaison contre une petite
plage de constantes proches (`0x800..0x8FF`) via une soustraction suivie
d'une table de saut, ce qui ne laisse pas la valeur pleine en immédiat
32 bits littéral dans le flux d'octets — une recherche de motif brut ne les
retrouve alors pas, sans que cela remette en cause leur exactitude (elles
suivent la même macro, avec la même base numérique confirmée). **Ne pas
sur-interpréter cette absence comme un doute sur la valeur des quatre codes
non retrouvés : la vérification par octets est une confirmation positive
(quand elle réussit), pas un test d'exhaustivité.**

Le GUID de classe standard `{4D36E968-E325-11CE-BFC1-08002BE10318}` n'a en
revanche **pas** été retrouvé en octets bruts dans la DLL — attendu : ce GUID
sert à l'installation (INF, `nefconc.exe`), pas au dialogue runtime avec le
pilote, il n'a pas de raison d'être compilé dans le corps de `SudoVDA.dll`.

**Suggestion pour la tâche 5 — vérifier le contrat de bout en bout à bon
marché, avant d'engager les structures riches.** Ceci est une piste, pas une
certitude : elle n'a pas été exécutée dans cette tâche (qui ne produit pas de
code et n'appelle pas le pilote). Parmi les six IOCTL, `IOCTL_GET_PROTOCOL_VERSION`
a le tampon le plus simple possible — entrée nulle (`nullptr`, 0 octet),
sortie de 4 octets seulement (`SUVDA_PROTOCAL_VERSION` : `Major`, `Minor`,
`Incremental`, `TestBuild`, chacun 1 octet). Un premier appel réel à ce code,
sur le device ouvert via `SUVDA_INTERFACE_GUID` (confirmé §5.3), donnerait une
confirmation de bout en bout distincte de la lecture amont : que l'appel
réussisse et rende exactement 4 octets serait déjà un signal fort ; que
`Major` vaille `0` (valeur documentée en amont, `VDAProtocolVersion = {0, 2,
1, true}`) le renforcerait encore, sans le prouver de manière absolue — une
coïncidence sur un seul octet reste possible, ce qu'une correspondance GUID
sur 16 octets (§5.3) exclut, mais pas un octet isolé. `IOCTL_GET_WATCHDOG`
(sortie 8 octets, deux `UINT`) est une deuxième piste du même ordre, un cran
plus riche. Réussir ces deux appels — sans effet de bord observable sur le
bureau, contrairement à `IOCTL_ADD_VIRTUAL_DISPLAY` — avant d'engager le
tampon d'entrée le plus riche (`VIRTUAL_DISPLAY_ADD_PARAMS`, 56 octets)
donnerait une base empirique à coût très faible pour la suite.

### 5.4 Corroboration indépendante : les chaînes de `sunshine.exe` local sont le texte exact du source amont

Le fichier `virtual_display.cpp` d'Apollo (amont, lu intégralement) contient
mot pour mot les chaînes de format retrouvées par `strings` dans le
`sunshine.exe` **installé sur cette VM** (§4) :

```cpp
// openVDisplayDevice()
printf("[SUDOVDA] Open device failed!\n");
...
printf("[SUDOVDA] SUDOVDA protocol not compatible with driver!\n");

// startPingThread()
printf("[SUDOVDA] Watchdog: Timeout %d, Countdown %d\n", watchdogOut.Timeout, watchdogOut.Countdown);
printf("[SUDOVDA] Watchdog fetch failed!\n");

// createVirtualDisplay()
printf("[SUDOVDA] Failed to add virtual display.\n");
printf("[SUDOVDA] Cannot get name for newly added virtual display!\n");
printf("[SUDOVDA] Configuration: W: %d, H: %d, FPS: %d\n", width, height, fps);

// removeVirtualDisplay()
printf("[SUDOVDA] Virtual display removed successfully.\n");
```

Correspondance texte-à-texte sur huit chaînes distinctes : c'est une preuve
indépendante (au-delà du GUID en octets, §5.3) que le binaire local exécute
bien — à une variation de version près — ce chemin de code amont. La séquence
d'appel qui en résulte (lue en amont, non exécutée ni observée sur cette VM) :

```cpp
DRIVER_STATUS openVDisplayDevice() {
    // retry avec backoff exponentiel 20ms -> ... -> abandon au-delà de 320ms
    SUDOVDA_DRIVER_HANDLE = OpenDevice(&SUVDA_INTERFACE_GUID);
    ...
    if (!CheckProtocolCompatible(SUDOVDA_DRIVER_HANDLE)) { ... }  // IOCTL_GET_PROTOCOL_VERSION
}

std::wstring createVirtualDisplay(client_uid, client_name, width, height, fps, guid) {
    VIRTUAL_DISPLAY_ADD_OUT output;
    AddVirtualDisplay(SUDOVDA_DRIVER_HANDLE, width, height, fps, guid, client_name, client_uid, output); // IOCTL_ADD_VIRTUAL_DISPLAY
    // puis poll GetAddedDisplayName (QueryDisplayConfig / DisplayConfigGetDeviceInfo, PAS un IOCTL SudoVDA)
    // avec backoff jusqu'à ce que Windows publie le nouveau device GDI
}

bool removeVirtualDisplay(const GUID& guid) {
    RemoveVirtualDisplay(SUDOVDA_DRIVER_HANDLE, guid);  // IOCTL_REMOVE_VIRTUAL_DISPLAY
}
```

`OpenDevice()` (amont, texte intégral déjà cité en préambule de cette
recherche) : `SetupDiGetClassDevs(&SUVDA_INTERFACE_GUID, ..., DIGCF_PRESENT | DIGCF_DEVICEINTERFACE)`
puis boucle `SetupDiEnumDeviceInterfaces` / `SetupDiGetDeviceInterfaceDetailA`
jusqu'à obtenir un `DevicePath`, puis :

```cpp
CreateFileA(detail->DevicePath,
    GENERIC_READ | GENERIC_WRITE,
    FILE_SHARE_READ | FILE_SHARE_WRITE,
    NULL, OPEN_EXISTING,
    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_NO_BUFFERING | FILE_FLAG_OVERLAPPED | FILE_FLAG_WRITE_THROUGH,
    NULL);
```

**Point d'attention pour la tâche 5** (observation, pas un verdict — cette
tâche ne teste pas le comportement) : le handle est ouvert avec
`FILE_FLAG_OVERLAPPED`, mais les appels `DeviceIoControl` du même fichier
amont (`AddVirtualDisplay`, `RemoveVirtualDisplay`, etc.) passent `nullptr`
comme dernier paramètre (`lpOverlapped`). La documentation Win32 de
`DeviceIoControl` est explicite : ce paramètre ne peut pas être `NULL` quand
le handle a été ouvert en mode chevauchant. Que ceci fonctionne quand même en
pratique (le pilote source amont semble l'utiliser tel quel en production)
n'a pas été vérifié ici — signalé pour que la tâche 5 choisisse en
connaissance de cause (p. ex. ouvrir sans `FILE_FLAG_OVERLAPPED` côté Rust,
ce qui est la voie la plus sûre pour des appels synchrones).

## 6. Verdict formel — forme B

> **Réserve à lire avant d'écrire du code sur ces tampons.** Seuls le GUID
> d'interface et les codes `IOCTL_ADD_VIRTUAL_DISPLAY` /
> `IOCTL_GET_WATCHDOG` sont confirmés par octets dans le binaire
> `SudoVDA.dll` installé sur cette VM (§5.3). **La disposition des
> structures ci-dessous — y compris `VIRTUAL_DISPLAY_ADD_PARAMS`, le tampon
> le plus riche — reste une lecture amont non confirmée localement**, avec un
> en-tête dont la dernière modification connue (2024-09) précède d'environ
> onze mois le pilote installé (`DriverVer 07/14/2025`). Rien ne garantit
> qu'aucun champ n'a été ajouté ou réordonné entre ces deux dates. Voir la
> fin du §5.3 pour une piste bon marché de confirmation empirique
> (`IOCTL_GET_PROTOCOL_VERSION` puis `IOCTL_GET_WATCHDOG`, tampons simples,
> sans effet de bord) avant d'engager `IOCTL_ADD_VIRTUAL_DISPLAY`.

- **GUID d'interface de périphérique** (à passer à `SetupDiGetClassDevs` /
  l'équivalent Rust `windows-rs` `CM_Get_Device_Interface_List` ou
  `SetupDiGetClassDevs`) :
  `SUVDA_INTERFACE_GUID = {0xe5bcc234, 0x1e0c, 0x418a, [0xa0, 0xd4, 0xef, 0x8b, 0x75, 0x01, 0x41, 0x4d]}`
  — **confirmé présent en octets dans le `SudoVDA.dll` installé sur cette VM**
  (§5.3).
- **Ouverture** : énumérer les interfaces de ce GUID (`DIGCF_PRESENT | DIGCF_DEVICEINTERFACE`),
  récupérer le `DevicePath`, `CreateFile` en `GENERIC_READ | GENERIC_WRITE`,
  partage lecture+écriture, `OPEN_EXISTING`. Voir réserve sur
  `FILE_FLAG_OVERLAPPED` ci-dessus (§5.4).
- **Codes IOCTL** (`METHOD_BUFFERED`, `FILE_ANY_ACCESS`, `FILE_DEVICE_UNKNOWN`
  = `0x22`) :

  | Code | Valeur | Tampon d'entrée | Tampon de sortie | Confirmé par octets locaux |
  | --- | --- | --- | --- | --- |
  | `IOCTL_ADD_VIRTUAL_DISPLAY` | `0x00222000` | `VIRTUAL_DISPLAY_ADD_PARAMS` (56 o.) | `VIRTUAL_DISPLAY_ADD_OUT` (12 o.) | **oui** |
  | `IOCTL_REMOVE_VIRTUAL_DISPLAY` | `0x00222004` | `VIRTUAL_DISPLAY_REMOVE_PARAMS` (16 o.) | — | non (voir §5.3) |
  | `IOCTL_SET_RENDER_ADAPTER` | `0x00222008` | `VIRTUAL_DISPLAY_SET_RENDER_ADAPTER_PARAMS` (8 o.) | — | non (voir §5.3) |
  | `IOCTL_GET_WATCHDOG` | `0x0022200C` | — | `VIRTUAL_DISPLAY_GET_WATCHDOG_OUT` (8 o.) | **oui** |
  | `IOCTL_DRIVER_PING` | `0x00222220` | — | — | non (voir §5.3) |
  | `IOCTL_GET_PROTOCOL_VERSION` | `0x002223FC` | — | `VIRTUAL_DISPLAY_GET_PROTOCOL_VERSION_OUT` (4 o.) | non (voir §5.3) |

  Toutes ces valeurs partagent la même formule (`CTL_CODE` avec la même base
  `FILE_DEVICE_UNKNOWN`/`METHOD_BUFFERED`/`FILE_ANY_ACCESS`), dont deux
  applications sont confirmées en octets — les quatre autres reposent sur la
  même formule appliquée au même en-tête amont, non re-confirmées par octets
  pour la raison de compilation exposée en §5.3, mais lues dans le même
  fichier source que les deux confirmées.

- **Séquence d'usage minimale** (`AddVirtualDisplay`) : ouvrir le device →
  vérifier `IOCTL_GET_PROTOCOL_VERSION` compatible (Major identique, Minor du
  client ≤ Minor du pilote) → `IOCTL_ADD_VIRTUAL_DISPLAY` avec
  `{Width, Height, RefreshRate, MonitorGuid (GUID choisi par le client, sert
  d'identifiant de retrait), DeviceName[14], SerialNumber[14]}` → récupérer
  `{AdapterLuid, TargetId}` en sortie → (optionnel) interroger Windows
  (`QueryDisplayConfig`/`DisplayConfigGetDeviceInfo`, hors SudoVDA) pour
  obtenir le nom GDI du nouvel écran → au retrait, `IOCTL_REMOVE_VIRTUAL_DISPLAY`
  avec le même `MonitorGuid`.

## 7. Ce que la tâche 5 doit encore décider (hors périmètre de cette reconnaissance)

- **Disposition des tampons non confirmée par octets**, au-delà du GUID et
  des 2 codes IOCTL du §5.3 : les structures du §5.2 sont une lecture amont
  non confirmée localement, avec l'écart de version non résolu décrit au
  §5.1. Avant d'implémenter `IOCTL_ADD_VIRTUAL_DISPLAY` (le tampon d'entrée
  le plus riche, 56 octets), il est recommandé — sans certitude que ce soit
  nécessaire ni suffisant — de valider empiriquement `IOCTL_GET_PROTOCOL_VERSION`
  puis `IOCTL_GET_WATCHDOG` (tampons de sortie simples, 4 et 8 octets, sans
  effet de bord observable) ; détail de cette piste en fin de §5.3.
- La traduction `#[repr(C)]` Rust des structures ci-dessus (tailles/offsets
  donnés en §5.2 pour vérification, pas de définition Rust fournie ici — ce
  n'est pas une reconnaissance de l'API Rust `windows`, mais de l'ABI C
  Win32).
- Le choix concret d'ouverture (chevauchante ou non) compte tenu de la
  réserve §5.4.
- Le protocole de version (`VDAProtocolVersion = {0, 2, 1, true}` côté
  en-tête amont daté de 2024-09 ; rien ne garantit que le pilote 1.10.9.289
  installé annonce exactement cette même version au runtime — à interroger
  via `IOCTL_GET_PROTOCOL_VERSION` au moment de l'implémentation, pas ici).
- Le pilote impose un **watchdog** (`IOCTL_GET_WATCHDOG` /
  `IOCTL_DRIVER_PING`) : un client qui n'envoie pas de ping périodique après
  avoir ouvert le device risque une coupure — à prendre en compte dans la
  conception du chantier D, pas dans cette tâche.
