# Les pixels du recadrage — la première image que ce chantier ait regardée

**31 août 2026, lot 33.** `CLAUDE.md` portait « AUCUN JUGEMENT VISUEL N'A ÉTÉ
PORTÉ » et « personne n'a regardé une image » comme legs ouverts depuis D1.
Ces fichiers sont la première pièce qui les entame.

## Ce qui est ici

| Fichier | Ce que c'est |
| --- | --- |
| `sonde-pixels.ps1` | la sonde, **jouée en SESSION 1** par tâche planifiée `/it` (un relevé WinRM est en session 0). Elle imprime sa propre session, trouve la fenêtre servie par son cadre DWM, capture ce rectangle, et échantillonne **11 points** par bord |
| `recadrage-1548x1032.png` | l'image capturée, telle que la duplication DXGI la voit |
| `coin-haut-gauche-x12.png` | le coin haut-gauche, agrandi ×12 au plus proche voisin |
| `coin-bas-droit-x12.png` | le coin bas-droit, idem |

## Le relevé

```
== session = 1
== recadrage lu (cadre DWM) = 1548x1032+1280+0
rangee 0    (bord HAUT)   : #494949 x10  #2F2F2F x1
rangee 1    (voisine)     : #F3F3F3 x11
rangee 2                  : #F3F3F3 x11
rangee 1029               : #F0F0F0 x11
rangee 1030               : #F0F0F0 x11
rangee 1031 (bord BAS)    : #2F2F2F x11
colonne 0   (bord GAUCHE) : #2F2F2F x11
colonne 1   (voisine)     : #FFFFFF x11
colonne 2                 : #FFFFFF x11
colonne 1545              : #F0F0F0 x11
colonne 1546              : #F0F0F0 x11
colonne 1547(bord DROIT)  : #2F2F2F x11
```

## 🔵 Le témoin, sans lequel le relevé ne dirait rien

**Les rangées et colonnes VOISINES ressortent `#F3F3F3` et `#FFFFFF`** — clair,
et jusqu'à du blanc pur. Un instrument qui rendrait « noir » par défaut (mauvais
rectangle, capture vide, écran verrouillé) les rendrait sombres elles aussi.
**C'est ce contraste, dans le MÊME relevé, qui autorise à lire le `#2F2F2F` des
bords comme une mesure** — jamais la valeur isolée.

## Ce que cela établit

Un pixel sombre sur **les quatre bords**, clair immédiatement en dedans :
c'est **la bordure que Windows peint autour de la fenêtre**, capturée parce que
le recadrage coïncide au pixel près avec `DWMWA_EXTENDED_FRAME_BOUNDS`.

🔴 **Et cela élimine l'artefact de mise à l'échelle du navigateur sans rien
avoir à supposer** : ces pixels sont lus **DANS LA VM**, sans navigateur
d'aucune sorte. Le pixel noir est dans l'image encodée, pas à l'écran.
