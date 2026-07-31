# Symbolisation des piles du plantage — tâche 2bis

Toutes les correspondances ci-dessous sont **relevées**, pas devinées : elles
viennent des PDB publics Microsoft, appariés au binaire par le GUID du
répertoire de débogage du PE lui-même.

## Méthode, reproductible sans débogueur sur la VM

Aucun débogueur n'est installé sur la VM (vérifié : ni `cdb.exe` ni
`windbg.exe` sous `C:\Program Files (x86)\Windows Kits\10\Debuggers`, dossier
`Debuggers` absent). Tout a été fait depuis l'hôte Linux, sur des copies :

1. Copier le module depuis `/media/vm/Windows/System32/`. Contrôle
   d'identité : `md5sum` de la copie contre `Get-FileHash -Algorithm MD5` sur
   la VM (`ntdll.dll` = `e6a8db4200c96a8297f7ab3b316b97f5`, 2 113 024 octets,
   identiques).
2. Lire le répertoire de débogage du PE (entrée `CODEVIEW`/`RSDS`) pour en
   extraire nom de PDB + GUID + âge. Pour `ntdll.dll` :
   `ntdll.pdb / A4BE1E08ADA695257A97082C3AE900971`.
3. Télécharger le PDB : `https://msdl.microsoft.com/download/symbols/<pdb>/<GUID+âge>/<pdb>`.
4. Lire le flux des symboles publics du PDB (MSF 7.00 → flux 3 = DBI → index du
   flux des symboles à l'offset 20 de son en-tête → enregistrements `S_PUB32`
   `0x110E`), et le répertoire `.pdata` du PE pour les **limites de fonction**.
   Une adresse est nommée `f+0xd` seulement quand le début de fonction donné
   par `.pdata` **coïncide exactement** avec un symbole public ; sinon la sortie
   le dit (« symbole public le plus proche EN DESSOUS »).

Scripts : `pe_info.py`, `sym.py`, `dmp.py` (scratchpad de session, hors dépôt).

**Piège relevé au passage** : `(Get-Item ntdll.dll).VersionInfo.FileVersion`
rend `10.0.20348.5020` (champ numérique de `VS_FIXEDFILEINFO`) alors que
l'Observateur d'évènements rend `10.0.20348.5386` (chaîne du bloc
`StringFileInfo`). **Les deux désignent le même fichier** — la chaîne `5386`
est présente deux fois dans le binaire, la chaîne `5020` zéro fois. Ne pas
conclure à un désappariement de version sur cet écart.

## `ntdll.dll` 10.0.20348.5386 — l'adresse fautive

| RVA | Symbole |
| --- | --- |
| `0x19daa` | **`RtlpEnterCriticalSectionContended+0x1da`** (fonction `0x19bd0`, limite `.pdata`, coïncide avec le public) |
| `0x174c2` | `RtlEnterCriticalSection+0x42` |
| `0x7bfe2` | `RtlpCallVectoredHandlers+0x112` |
| `0x31722` | `RtlDispatchException+0x62` |
| `0xa42ee` | `KiUserExceptionDispatcher+0x2e` |
| `0x6958a` | `TppWorkpExecuteCallback+0x13a` |
| `0xbb26` | `TppWorkerThread+0x686` |
| `0x7edeb` | `RtlUserThreadStart+0x2b` |

Désassemblage **depuis le début de la fonction** (`objdump -b pei-x86-64`, donc
sans risque de désynchronisation), extrait :

```
180019d1e:  f0 41 0f b1 16   lock cmpxchg %edx,(%r14)   ; CAS sur LockCount
180019d84:  49 83 7d 18 00   cmpq $0x0,0x18(%r13)       ; LockSemaphore != 0 ?
180019d8f:  49 8b 4d 00      mov  0x0(%r13),%rcx        ; rcx = DebugInfo
180019d93:  48 83 f9 ff      cmp  $-1,%rcx              ; sentinelle « pas de debug info »
180019daa:  ff 41 24         incl 0x24(%rcx)            ; <== FAUTE : DebugInfo->ContentionCount++
180019dad:  4d 8b 6d 18      mov  0x18(%r13),%r13
```

La disposition est **ancrée par le code lui-même**, pas seulement par
`winnt.h` : `r14 = r13 + 8` (relevé dans les registres), et `r14` porte le
`lock cmpxchg` → `r13` est le début de la `RTL_CRITICAL_SECTION` et `+8` son
`LockCount`. Le champ à `+0` est donc `DebugInfo`, et `+0x24` dans le
`RTL_CRITICAL_SECTION_DEBUG` pointé est `ContentionCount`.

## `RTWorkQ.dll` — le fil fautif (dépilage réel, `RtlCaptureStackBackTrace`)

| RVA | Symbole |
| --- | --- |
| `0xbdca` | `CSerialWorkQueue::QueueItem::ExecuteWorkItem+0xca` |
| `0xb8b9` | `CSerialWorkQueue::QueueItem::OnWorkItemAsyncCallback::Invoke+0x29` |
| `0x8e91` | `WorkItem::Free+0x171` (symbole public le plus proche en dessous) |

## `nvEncMFTH264x.dll` — l'appelant immédiat

`0x7ff820d8735d` → `nvEncMFTH264x.dll+0x735d`, appelé depuis
`nvEncMFTH264x.dll+0x34ac`. **Aucun PDB public n'existe pour ce module**
(pilote NVIDIA) : ces deux cadres restent en `module+décalage`, et rien de plus
n'en est affirmé. L'identification du module vient de la liste des modules du
vidage WER (le filtre d'exception, lui, ne connaît que les modules chargés à
son installation, antérieure au chargement de la MFT — d'où les
`<hors module>` dans les journaux bruts).

## Fil principal — pile relevée par BALAYAGE, non par dépilage

Valeurs trouvées sur la pile du fil principal au-dessus de `rsp`, dans
**l'ordre croissant des adresses**, donc du cadre le plus interne au plus
externe. Ce n'est pas un dépilage : une valeur peut être résiduelle. Ce qui
donne son poids à ce relevé, c'est que la suite obtenue est **exactement
imbriquée** comme une pile d'appels, et **identique sur les deux vidages**.

| Adresse | Symbole |
| --- | --- |
| `user32+0x254fe` | `RealMsgWaitForMultipleObjectsEx+0x1e` |
| `combase+0x6a84b` | `CCliModalLoop::BlockFn+0x193` |
| `combase+0x68195` | `ClassicSTAThreadWaitForHandles+0xa5` |
| `combase+0x5de20` | `CoWaitForMultipleHandles+0x80` |
| `RTWorkQ+0xa893` | `CPlatform::FinalShutdown+0x1e3` |
| `RTWorkQ+0xb2df` | `CPlatform::Shutdown+0x4f` |
| `RTWorkQ+0xb280` | `RtwqShutdown+0x10` |
| `mfplat+0x38809` | **`MFShutdown+0x29`** |
| `agent.exe+0xed87` | `agent::diagnostics::multifenetre::banc::executer+0x1987` |
| `agent.exe+0xb0cae` | `agent::diagnostics::multifenetre::aiguiller+0x78e` |
| `agent.exe+0xdb61c` | `agent::diagnostics::aiguiller+0x21c` |
| `agent.exe+0xa1179` | `tokio::runtime::park::CachedParkThread::block_on::<agent::main>` |
