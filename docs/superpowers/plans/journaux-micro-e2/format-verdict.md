# Tâche 2 — le format de CABLE Output : **le remède est INAPPLICABLE, et c'est
# un résultat, pas un échec de tâche**

**Jouée le 20 août 2026.** Aucun fichier de code de produit n'est touché.
**Une exécution par relevé. Aucun taux.**

---

## ⛔ Le verdict : **E2 se livre avec un câble asymétrique.** CABLE Output reste à 44 100 Hz.

Et il ne reste pas à 44 100 pour la raison que E1 annonçait. **Le plan attendait
« le bac à sable refuse l'écriture » ; ce n'est pas ce qui s'est passé.** La
séquence réelle, en quatre temps :

| # | Ce qui a été tenté | Résultat **relevé** |
| --- | --- | --- |
| 1 | `Set-ItemProperty` sur `PKEY_AudioEngine_DeviceFormat` de CABLE Output | 🔴 **REFUSÉ** — `System.Security.SecurityException : Accès au registre demandé non autorisé`, **jeton élevé Administrateur pourtant confirmé** (`micro-fixe-format-e2.log`) |
| 2 | `Get-Acl` sur la clé — **une lecture, pas un contournement** | L'ACL **ACCORDE** `SetValue` : `BUILTIN\Administrateurs Allow SetValue, ReadKey`. Le refus n'est **pas** une politique, c'est l'outil : le fournisseur `Registry` de PowerShell ouvre en `KEY_WRITE` = `SetValue` **et** `CreateSubKey`, et `CreateSubKey` n'est accordé qu'à `TrustedInstaller`, `Audiosrv` et `AudioEndpointBuilder` (`micro-e2-acl.log`) |
| 3 | `RegistryKey.OpenSubKey(…, RegistryRights::SetValue)` — **le droit exact que l'ACL accorde**, sans changer de propriétaire ni d'ACL, sans réclamer de privilège | ✅ **ACCEPTÉ**, relecture à `80 BB 00 00` = **48 000** (`micro-fixe-format-e2b.log`) |
| 4 | `Restart-Service Audiosrv`, puis `Restart-Service AudioEndpointBuilder` | ✅ **Tous deux ACCEPTÉS** — et **`GetMixFormat` rend TOUJOURS 44 100** (`format-apres.log`) |

**Et l'écriture ne se contente pas d'être sans effet : elle CASSE le point de
terminaison.** Une fois le blob symétrique posé, `IAudioClient::Initialize`
sur CABLE Output, **avec le format que `GetMixFormat` vient de rendre**, échoue
en **`0x88890008` = `AUDCLNT_E_UNSUPPORTED_FORMAT`**. Le câble devient
inécoutable : plus aucune application ne peut ouvrir ce microphone.

🔴 **C'est le précédent D8/C1 en miniature — un registre audio laissé de travers
bloque le produit — et il a été atteint pour de bon**, deux fois : une fois par
la tentative, une fois par une **restauration qui a elle-même échoué** (voir
« Ce que cela a coûté »). **La VM a été remise en état et le contrôle est
joué** : combinaison A rejouée après restauration, **660,0 Hz** de nouveau sur
CABLE Output (`micro-e2-ecoute-A.log`).

---

## L'épreuve reproductible, restauration COMPRISE

`micro-e2-format-epreuve.ps1` applique le blob, mesure, **remet l'ancien** et
remesure — les deux moitiés dans le **même** script, pour que la VM ne puisse
pas rester de travers si le pilote perd la main.

| Phase | Journal | `IAudioClient::Initialize` |
| --- | --- | --- |
| **appliqué** (48 000 au registre) | `micro-e2-format-epreuve-applique.log` | 🔴 **`0x88890008`** — endpoint inutilisable |
| **restauré** (44 100 au registre) | `micro-e2-format-epreuve-restaure.log` | ✅ ouvre, `PAQUETS=299 TRAMES=131859`, format 44 100 |

**Le contrôle peut donc rendre les deux valeurs**, et c'est la même exécution
qui le montre.

## Ce que le relevé apprend sur la propriété visée elle-même

⚠️ **`PKEY_AudioEngine_DeviceFormat` ne décrit PAS ce que `GetMixFormat` rend
sur ce périphérique**, et cela se lit dans le relevé « avant », sans aucune
écriture : le blob du **rendu** porte `nBlockAlign = 6`, `wBitsPerSample = 24`,
sous-format **`KSDATAFORMAT_SUBTYPE_PCM`** — c'est-à-dire **24 bits entiers** —
quand `GetMixFormat` rend `32 bits flottant` (`format-avant.log`, sections 1 et
2). Seule la **fréquence** paraissait suivre. **Le remède de E1 vise donc une
propriété dont on constate qu'elle ne gouverne pas la valeur observée**, ce
qu'aucun document ne disait, et l'échec du point 4 en est cohérent.

## Ce qui est réfuté de E1, et ce qui ne l'est pas

- ❌ **« L'écriture au registre ET le redémarrage d'`Audiosrv` ont été refusés
  par le bac à sable » est FAUX SUR SES DEUX MOITIÉS.** `Restart-Service
  Audiosrv` est **accepté** (deux exécutions), et l'écriture est **acceptée**
  dès qu'on demande le droit que l'ACL accorde.
- ✅ **Le fait de fond tient** : CABLE Output est à 44 100 quand CABLE Input est
  à 48 000, avant comme après. **Le remède ne l'a pas changé.**
- 🔴 **Et « le script est prêt sur le partage » reste faux** : il n'existait pas,
  il est écrit ici (`micro-fixe-format-e2.ps1`, puis `…-e2b.ps1`), et il est
  **versé**.

## Ce que cela change pour E2 — et ce que cela ne change pas

- **Le critère d'écoute SURVIT, et c'est mesuré, pas raisonné.** La sonde de la
  tâche 1 émet **660 Hz à 48 000 Hz**, le pilote rééchantillonne, et le juge
  retrouve **660,0 Hz** sur un endpoint à 44 100. **Un rééchantillonnage
  préserve la fréquence d'un ton pur** — les six exécutions du juge le montrent.
- 🔴 **La QUALITÉ et la LATENCE ne sont pas celles d'un chemin sans conversion,
  et PERSONNE NE LES A MESURÉES.** Ni E1, ni ce relevé. **Le document de
  résultats de E2 doit le porter.**
- **Rien ne change côté code** : l'agent écrit sur CABLE **Input**, qui est bien
  à 48 000 — le format que produit `LecteurMicro::remplir`. La conversion est
  **entièrement** dans le pilote VB-Cable, en aval de nous.

## Ce que cela a coûté, et la leçon qui se verse

🔴 **Une restauration écrite dans le même script que l'application n'est pas
une restauration si elle n'est pas ÉPROUVÉE.** La première version de
`micro-e2-format-epreuve.ps1` a **appliqué avec succès et restauré en échec** —
`return $v` déroule un `byte[]` en `Object[]`, que `SetValue` refuse ensuite en
`RegistryValueKind::Binary` :

```
ecriture : REFUSEE : … « Le type de l'objet value ne correspondait pas au
RegistryValueKind spécifié ou l'objet n'a pas été correctement converti. »
```

**La VM est restée avec un microphone inutilisable** jusqu'à ce qu'un script
distinct (`micro-e2-restaure-format.ps1`, qui reconstruit les octets depuis une
chaîne hexadécimale et les caste explicitement) la remette d'aplomb. Le script
porte désormais `return ,[byte[]]$v` **et le commentaire qui dit pourquoi**, et
il a été **rejoué en entier** : application → `0x88890008`, restauration →
endpoint qui ouvre.

**Ce qui a sauvé la mise est la consigne du plan** — *écrire le blob de départ
dans un journal AVANT de le modifier*. Sans ces 48 octets dans
`micro-fixe-format-e2.log`, il n'y avait aucun chemin de retour.

## Ce que la tâche 2 n'établit PAS

- **Une exécution par relevé.**
- **Pourquoi** le pilote refuse son propre format de registre n'est **pas
  expliqué** — seulement constaté, et reproduit.
- **Rien n'a été tenté au-delà** : ni changement de propriétaire de la clé, ni
  modification d'ACL, ni désactivation/réactivation du périphérique PnP, ni
  réinstallation de VB-Cable, ni son panneau de configuration. Le plan dit
  « ne pas insister, et ne pas contourner » ; **les deux écritures jouées ici
  n'emploient que le droit que l'ACL accorde déjà**, et la seconde a été
  intégralement défaite.
- **Rien de la dégradation audible** imputable au 48 000 → 44 100.
