# Lot 3, item 12 (3.10) — le maillon fautif du `Resize`

**Mesuré le 5 septembre 2026.** ⚠️ **Aucun chiffre de ce document ne se
recopie sans relancer sa commande.**

---

## 1. Le verdict

🔵 **AUCUN MAILLON N'EST FAUTIF AUJOURD'HUI, SUR CE CHEMIN.** Trois viewports
demandés, trois arrivés intacts jusqu'à la taille décodée par le navigateur.

| viewport demandé (CDP) | `demande=` reçue par le capteur | `retenue=` | `change=` | `videoWidth×videoHeight` au navigateur |
| --- | --- | --- | --- | --- |
| 1024×700 | `1024x700` | `1024x700` | `true` | **1024×700** |
| 1400×500 | `1400x500` | `1400x500` | `true` | **1400×500** |
| 640×900 | `640x900` | `640x900` | `true` | **640×900** |

Trois rapports d'aspect franchement différents (1,46 / 2,80 / 0,71), tous
servis exactement. `borne="1860x1080"` et `texture="1860x1080"` aux trois :
les demandes tiennent sous la borne, aucune n'est écrêtée.

## 2. Le tableau maillon par maillon

🔴 **Disculper un maillon ne désigne pas le coupable suivant** : chaque ligne
porte SA mesure.

| Maillon | Ce qui le disculpe, mesuré |
| --- | --- |
| **① l'annonce du navigateur** | le stimulus est `Emulation.setDeviceMetricsOverride`, donc le `ResizeObserver` du produit (`client/src/resize-dom.ts`, câblé par `main.ts`) — pas un appel interne. La preuve qu'il a émis : la valeur exacte reparaît côté agent |
| **② la réception par l'enfant** | `demande="1024x700"` figure dans le journal **de l'agent**, sous le span `fenetre{session=…:w-1}` : le message a traversé le canal de contrôle et a été désérialisé |
| **③ le relais au capteur, et son ACCEPTATION** | la trace `Resize recu par le capteur` est émise **dans le capteur** (`agent::windows_source::redimensionnement`). Elle n'existe que si le message a été **accepté** par la file bornée — corroboré par l'item 6 : `message REFUSE` = 0 |
| **④ l'application par la source** | `retenue="1024x700" courante="780x492" change=true` — la source retient la taille demandée et déclare le changement |
| **⑤ la taille effectivement encodée** | `videoWidth=1024, videoHeight=700` relevés **dans la page**, sur l'élément `<video>` : c'est la taille de la trame DÉCODÉE, donc celle qui est sortie de l'encodeur |

## 3. Le contrôle peut rendre l'autre valeur — dans le MÊME relevé

🔴 Un `change=true` qui ne pourrait jamais valoir `false` ne prouverait rien.
Le même journal, même session, porte les deux :

```
23:10:23  demande="780x493" retenue="780x492" courante="780x492" change=false
23:10:26  demande="1024x700" retenue="1024x700" courante="780x492" change=true
23:10:32  demande="1400x500" retenue="1400x500" courante="1024x700" change=true
23:10:38  demande="640x900"  retenue="640x900"  courante="1400x500" change=true
23:10:44  demande="780x493" retenue="780x492"  courante="640x900"  change=true
```

La première ligne est la taille par défaut, arrondie (`493 → 492`) et trouvée
**identique à la courante** : `change=false`. La dernière est la même demande,
mais après les trois autres : `change=true`, parce que la courante a bougé.
**Le champ discrimine bien un état du produit, pas une constante de trace.**

## 4. L'historique du journal, et ce qu'il dit du legs

Sur les **169 993 lignes** du journal (cinq jours) :

```
redimensionnement ignor…  : 34
Resize                     : 208
```

⚠️ **Les 34 refus datent TOUS du 31 août 2026**, et portent tous le même
motif : `la source capture une sortie DXGI entière … mode=SortieEntiere`, à
des tailles de 1580×1237 à 1723×1303. **Aucun depuis.** Le mode
`SortieEntiere` n'est plus pris sur ce chemin, et c'est pourquoi les trois
viewports d'aujourd'hui aboutissent.

## 5. Ce que cet item N'ÉTABLIT PAS

- 🔴 **LA ROUGE PRESCRITE PAR LE PLAN N'A PAS ÉTÉ JOUÉE.** Le plan (étape 3)
  demandait un `Resize` **en mode `SortieEntiere`**, pour voir sortir le refus
  connu et voulu. Ce mode n'est pas pris aujourd'hui ; l'armer suppose
  `MULTIFENETRE_MODE_SORTIE` ou un plein écran, et **cela n'a pas été fait**.
  Ce qui tient lieu de discrimination est le couple `change=false` /
  `change=true` du § 3 — **c'est une autre rouge, plus faible**, et elle est
  déclarée comme telle.
- ⚠️ **Personne n'a REGARDÉ l'image.** `videoWidth × videoHeight` dit la taille
  de la trame décodée, **jamais qu'elle montre la bonne chose** — ni cadrage,
  ni netteté, ni bandes.
- ⚠️ **Une seule fenêtre, une seule session.** Le comportement du `Resize` en
  multi-fenêtres n'est pas mesuré.
- ⚠️ **`devicePixelRatio` valait 1** dans tous les bras (`deviceScaleFactor: 1`)
  : le chemin HiDPI, où le `Resize` part en pixels périphériques, n'est pas
  éprouvé.
- ⚠️ **Le legs « un redimensionnement demandé pendant le SOMMEIL d'une fenêtre
  est perdu »** n'est pas touché : aucune fenêtre n'a été endormie.

Journal brut : `maillons-20260905T0110.log` — relevé de la page : `viewports.json`.
