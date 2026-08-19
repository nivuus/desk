# Sondes 2 et 3 — VB-Cable sur la VM, son format, le peripherique par defaut

Releve du 19 aout 2026, par WinRM, VM `Windows` demarree et joignable.
Pieces brutes : `vbcable-etat.log` (etat general) et `vbcable-format.log`
(le format, par `IAudioClient::GetMixFormat` — la MEME API que notre code
WASAPI emploiera).

Methode : les scripts sont ecrits sur le partage (`C:\dev\micro-*.ps1`) et
invoques par `-File`. **Aucun guillemet en ligne de commande** — `nodejs-winrm`
enveloppe tout dans `powershell -Command "& { ... }"` et un script inline
entre en collision avec cette enveloppe, sans erreur claire.

---

## Sonde 2 — VB-Cable EST installe (l'installation a ete faite par le
## proprietaire du depot AVANT cette tache ; aucune installation n'a ete
## tentee ici)

```
Class : MEDIA         VB-Audio Virtual Cable        OK   ROOT\MEDIA\0000
Class : AudioEndpoint Haut-parleurs (VB-Audio Virtual Cable)  OK
                      SWD\MMDEVAPI\{0.0.0.00000000}.{DEEC1914-...}   <- CABLE Input, RENDU
Class : AudioEndpoint CABLE Output (VB-Audio Virtual Cable)   OK
                      SWD\MMDEVAPI\{0.0.1.00000000}.{5FAE72B2-...}   <- CAPTURE

pnputil /enum-drivers : oem20.inf, d'origine vbmmecable64_win7.inf,
                        VB-Audio Software, 1.0.3.5, signe Vincent Burel.
```

**Ce que cela etablit** : les deux endpoints attendus existent et sont `OK`.
**Ce que cela n'etablit PAS** : la silenciosite de l'installation. Elle n'a pas
ete tentee par cette tache et reste donc INCONNUE — l'installation a ete faite
hors de cette mesure, avec ajout du certificat de l'editeur aux magasins
`TrustedPublisher` et `Root`. **E2 ne peut pas supposer une installation
non interactive sur une machine neuve.**

---

## 🔴 Sonde 2 step 4 — LE FORMAT : le risque R3 est REEL, et il est ASYMETRIQUE

`IAudioClient::GetMixFormat`, en mode PARTAGE, sur les endpoints ACTIFS :

| Endpoint | Sens | Format rendu par GetMixFormat |
| --- | --- | --- |
| Haut-parleurs (Steam Streaming Speakers) | rendu | 48000 Hz, 2 ch, float32 |
| HDP-V104 (NVIDIA HD Audio) | rendu | 48000 Hz, 2 ch, float32 |
| **Haut-parleurs (VB-Audio Virtual Cable)** = **CABLE Input** | **rendu** | **48000 Hz, 2 ch, float32** ✅ |
| **CABLE Output (VB-Audio Virtual Cable)** | **capture** | **44100 Hz, 2 ch, float32** 🔴 |

Corrobore par le format PAR DEFAUT declare au registre (`MMDevices`), decode
octet par octet depuis le blob brut verse dans `vbcable-format.log` — le
WAVEFORMATEX commence a l'octet 8, apres l'en-tete PROPVARIANT :

```
CABLE Input  (rendu)   : FE FF 02 00 | 80 BB 00 00 | ... -> 0xBB80 = 48000 Hz, 2 ch, 24 bits PCM
CABLE Output (capture) : FE FF 02 00 | 44 AC 00 00 | ... -> 0xAC44 = 44100 Hz, 2 ch, 24 bits PCM
```

⚠️ **La difference 24 bits (registre) / float32 (GetMixFormat) n'est pas une
contradiction** : en mode PARTAGE l'engin audio de Windows mixe toujours en
float32. C'est `GetMixFormat` qui dit ce que notre code verra, et c'est donc
lui qui fait foi ici — le registre ne donne que la frequence et le nombre de
canaux, qui concordent.

### Ce que ce releve decide

- ✅ **Le chemin que E1 et E2 empruntent REELLEMENT est a 48 kHz.**
  `wasapi/rendu.rs` (E2) ecrira sur **CABLE Input**, en mode partage, et y
  trouvera **48000 Hz, 2 canaux, float32** — c'est exactement ce que
  `LecteurMicro::remplir(&mut [f32])` produit. **Notre regle (spec §4, aucun
  reechantillonnage) est satisfaite sans aucune concession, et R3 n'est PAS
  eliminatoire pour notre code.**
- 🔴 **Mais l'autre bout du cable est a 44100 Hz.** L'application Windows qui
  ouvrira « CABLE Output » comme microphone lira du 44100 Hz, et **VB-Cable
  reechantillonnera 48000 -> 44100 en interne**. Ce n'est pas notre regle qui
  est violee — c'est une conversion que nous avions decide de ne pas faire et
  qui se fait quand meme, hors de notre code et hors de notre mesure.
  **Consequence pour E2 : la qualite et la latence du cable ne sont pas
  celles d'un chemin sans conversion, et la recette d'ecoute de E2 doit en
  tenir compte.**

### Le remede, ECRIT et NON APPLIQUE

Le remede est une **configuration de l'image de base**, pas du code (spec §3) :
aligner le format par defaut de CABLE Output sur 48000 Hz, soit par l'onglet
« Statistiques avancees » du peripherique dans le panneau de configuration son,
soit par le panneau VB-Cable, soit par la transposition de registre ci-dessous.

**Il n'a PAS ete applique** : l'ecriture au registre et le redemarrage
d'`Audiosrv` ont ete refuses par le bac a sable de l'agent. Ce n'est pas un
echec de mesure — la mesure est faite — mais **le remede attend le
proprietaire de la machine**. Le script est pret sur le partage
(`C:\dev\micro-fixe-format-e1.ps1`) ; sa transposition est minimale et
verifiable : recopier les octets 8.. du blob du RENDU (deja a 48 kHz, meme
structure) dans le blob de la CAPTURE, en laissant intacts les 8 octets
d'en-tete PROPVARIANT propres a chaque valeur — seuls `nSamplesPerSec` et
`nAvgBytesPerSec` different entre les deux.

**Verification apres application** : rejouer `micro-format-e1.ps1` et lire
`CABLE Output ... HZ=48000` dans la section 2.

---

## Sonde 3 — CABLE Output EST DEJA le microphone par defaut

`IMMDeviceEnumerator::GetDefaultAudioEndpoint`, les trois roles :

```
RENDU    eConsole / eMultimedia / eCommunications : Haut-parleurs (VB-Audio Virtual Cable)
CAPTURE  eConsole / eMultimedia / eCommunications : CABLE Output (VB-Audio Virtual Cable)
```

**Sonde 3 repondue favorablement, et sans qu'aucune API non documentee
(`IPolicyConfig`) n'ait ete necessaire** : l'installateur VB-Cable a place ses
deux endpoints par defaut sur les trois roles. La degradation d'usage que la
spec §12 redoutait — « chaque application se regle a la main » — **n'a pas
lieu** sur cette VM.

⚠️ **Deux reserves, nommees :**

1. **WinRM tourne en SESSION 0.** Le peripherique par defaut est une notion de
   session : ce releve est celui de la session 0, **pas de la session
   interactive** ou tournent les applications que E2 fera ecouter. La
   concordance est plausible (le reglage est stocke par machine puis par
   utilisateur) mais **elle n'est pas mesuree ici**. Le rejouer depuis une
   tache planifiee `/it` est le geste qui la trancherait.
2. 🔴 **Le RENDU par defaut est passe sur le cable, ce qui n'etait pas
   demande et n'est PAS anodin** : tout son que Windows joue sans choisir
   explicitement son endpoint part desormais dans le cable au lieu des
   haut-parleurs. **Le chantier A capte le son par WASAPI loopback sur le
   peripherique de rendu par defaut de la session interactive** (releve du
   28 juillet 2026, § « 🔊 Audio de la VM » de `CLAUDE.md`) : si ce defaut a
   aussi bascule dans la session interactive, **le loopback de A capterait
   desormais le cable et non les haut-parleurs**. Ce n'est pas mesure, et
   c'est un risque de regression sur un chantier LIVRE. **A verifier avant
   toute recette audio ulterieure**, y compris celle de E1.

---

## Ce que les sondes 2 et 3 n'etablissent PAS

- **La silenciosite de l'installation** (l'installation n'a pas ete tentee ici).
- **La licence** : VB-Audio est gratuit en usage PERSONNEL seulement (R4).
- **Aucun taux** : une execution de chaque releve.
- **Le peripherique par defaut de la SESSION INTERACTIVE** (reserve 1 ci-dessus).
- **L'effet du basculement du rendu par defaut sur le chantier A** (reserve 2).
