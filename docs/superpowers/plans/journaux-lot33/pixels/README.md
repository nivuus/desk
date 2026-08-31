# Lire les pixels d'une fenêtre servie — instrument réutilisable

**31 août 2026, lot 33.** `CLAUDE.md` portait « personne n'a regardé une
image » comme manque ouvert depuis D1 : ce répertoire est la première pièce qui
l'entame, et l'instrument est ici pour être **rejoué par quelqu'un d'autre**.

---

## 1. Mode d'emploi

🔴 **EN SESSION 1, JAMAIS PAR WinRM DIRECT.** Un relevé WinRM court en
session 0, où `EnumWindows` ne rend rien et où il n'y a **aucun bureau à
capturer** : la sonde rendrait une image vide, c'est-à-dire noire — donc
exactement le symptôme qu'on cherche, pour la mauvaise raison. **La sonde
imprime sa propre session en première ligne ; si elle ne dit pas
`== session = 1`, le relevé est à jeter.**

Le partage `D$` est monté sur l'hôte en `/media/win-d` (voir `mount`). Depuis
`packages/installer` :

```bash
# ① déposer la sonde là où la session 1 la verra
sudo mkdir -p /media/win-d/nivuus-pixels
sudo cp docs/superpowers/plans/journaux-lot33/pixels/sonde-pixels.ps1 \
        /media/win-d/nivuus-pixels/sonde.ps1

# ② la jouer EN SESSION 1, par tâche planifiée /it
sudo python3 console/guest/winrm_exec.py ps "
  schtasks /create /tn LirePixels /tr 'powershell -ExecutionPolicy Bypass -NoProfile -File D:\nivuus-pixels\sonde.ps1' /sc once /st 00:00 /it /ru Administrator /f
  schtasks /run /tn LirePixels
"

# ③ relire par le partage — PAS par WinRM : la sortie texte s'y corrompt
sudo cat /media/win-d/nivuus-pixels/pixels.txt

# ④ NE RIEN LAISSER
sudo python3 console/guest/winrm_exec.py ps "schtasks /delete /tn LirePixels /f"
sudo rm -rf /media/win-d/nivuus-pixels
```

⚠️ **Le chemin de sortie est en dur dans la sonde** (`D:\nivuus-...`) : le
changer, c'est éditer les deux lignes `$out` / `$bmp.Save`. ⚠️ **Écrire dans un
fichier DÉJÀ existant depuis une autre session l'écrase** — le lot 33 a lu un
relevé portant `== session = 0` parce qu'un essai manuel avait recouvert celui
de la tâche planifiée. **Un nom de fichier neuf par relevé.**

## 2. Ce que la sonde fait

Elle choisit la fenêtre `Notepad` **visible, non minimisée, la plus grande**,
prend son `DWMWA_EXTENDED_FRAME_BOUNDS` — c'est-à-dire **le rectangle que le
recadrage vise** —, capture cette zone de l'écran, puis échantillonne **11
points** le long de chaque bord et de ses deux voisines. Elle écrit aussi
l'image entière en PNG.

Pour une autre application, changer le test `-eq 'Notepad'`.

## 3. 🔵 Le témoin, sans lequel le relevé ne dit rien

**Les rangées et colonnes VOISINES doivent ressortir CLAIRES.** Un instrument
qui rendrait « noir » par défaut — mauvais rectangle, capture vide, session
verrouillée, session 0 — les rendrait sombres elles aussi, et l'on lirait une
bordure là où il n'y a qu'une panne. **C'est le contraste dans le MÊME relevé
qui fait la mesure, jamais la valeur isolée d'un bord.**

## 4. Le relevé du 31 août 2026 (avant correctif)

```
== session = 1
== recadrage lu (cadre DWM) = 1548x1032+1280+0
rangee 0    (bord HAUT)   : #494949 x10  #2F2F2F x1
rangee 1    (voisine)     : #F3F3F3 x11
rangee 1031 (bord BAS)    : #2F2F2F x11
rangee 1030 (voisine)     : #F0F0F0 x11
colonne 0   (bord GAUCHE) : #2F2F2F x11
colonne 1   (voisine)     : #FFFFFF x11
colonne 1547(bord DROIT)  : #2F2F2F x11
colonne 1546(voisine)     : #F0F0F0 x11
```

Un pixel sombre sur les quatre bords, clair immédiatement en dedans : **la
bordure que Windows peint autour de la fenêtre**, capturée parce que le
recadrage coïncide au pixel près avec le cadre DWM.

🔴 **Ces pixels sont lus DANS LA VM, sans navigateur** : ils éliminent d'eux-
mêmes toute explication par le rendu de la page (cache, mise à l'échelle
fractionnaire, `object-fit`).

---

## 5. 🔴 LE CRITÈRE DE VÉRIFICATION DU CORRECTIF, EN UNE PHRASE

**Après déploiement, `rangee 0` et `colonne 0` doivent rendre la MÊME famille
de couleurs CLAIRES que leurs voisines `rangee 1` et `colonne 1` (`#F3F3F3`,
`#FFFFFF`, `#F0F0F0`) ; si elles rendent encore `#2F2F2F` — ou toute valeur
sombre isolée pendant que la voisine reste claire — le correctif n'a pas
mordu.**

⚠️ Et si **les deux** ressortent sombres, ce n'est pas le correctif qui a
échoué : c'est l'instrument qui n'a rien capturé (voir le témoin, §3).
