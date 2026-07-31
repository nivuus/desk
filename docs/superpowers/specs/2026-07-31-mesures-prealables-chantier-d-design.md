# Mesures préalables au chantier D — conception

> Second chantier de mesure, dans la lignée directe de la sonde de capture
> multi-fenêtres (`plans/2026-07-30-sonde-capture-multifenetre-resultats.md`).
> Il ne construit rien du **chantier D (multi-fenêtres)**, décrit dans
> `2026-07-28-support-jeux-design.md` §4 et §5 : il remplit les quatre trous
> que la sonde a laissés, sans lesquels la spécification de D n'est pas
> écrivable.

## 1. Question posée

La sonde a rendu son verdict le 30 juillet 2026 : **aucune voie de capture
n'est franchement viable**. Elle recommande « un moniteur virtuel par
fenêtre » comme cible, `PrintWindow` comme repli, et pose que **deux mesures
sont bloquantes avant de spécifier le chantier D** — le plafond de sorties
virtuelles simultanées et le plafond d'encodage sur périphériques D3D11
séparés.

Le chiffre qui fonde la voie recommandée (une sortie DXGI de 3413×960 pendant
une session de streaming Apollo) est aussi **le seul chiffre du document sans
journal joint**, et il n'est pas reproductible sans le propriétaire du poste.
La voie 2 repose donc sur un relevé unique, non rejoué, dont le plafond n'a
jamais été approché.

Ce bloc lève les quatre inconnues qui dimensionnent le chantier D.

## 2. Décisions de cadrage

| Décision | Retenu |
| --- | --- |
| Nature du bloc | Chantier de **mesure**, aucun livrable produit |
| Périmètre | **Quatre** mesures : ① plafond de sorties virtuelles, ② plafond d'encodage sur périphériques séparés, ③ correction d'image sur sortie virtuelle, ④ `PrintWindow` à N=4 et N=8 |
| Pilotage du moniteur virtuel | **Notre propre code**, pas Apollo |
| Repli si le pilote résiste | **Changer de pilote d'affichage virtuel**, pas rétro-ingénierer |
| Ordre | Par risque décroissant, ④ excepté (gratuit, et il assure le repli) |
| Emplacement du code | `agent/src/diagnostics/`, hors chemin de production |

**Pourquoi piloter le pilote nous-mêmes plutôt que relancer Apollo.** La sonde
attribuait le blocage de ① à un obstacle matériel — un second appareil client
apparié, que le propriétaire du poste n'a pas. Mais le même document note
qu'appeler `AddVirtualDisplay` directement est « identifié mais non éprouvé »,
et que **le produit ne peut pas durablement dépendre du démarrage d'une session
Apollo**. Piloter le pilote nous-mêmes libère la mesure de tout matériel tiers
et éprouve du même geste le mécanisme dont le chantier D aura besoin. C'est un
travail que D devra faire de toute façon ; l'avancer ici le fait payer une
seule fois.

**Pourquoi l'ordre par risque décroissant.** Trois des quatre mesures sont
quantitatives sur des mécanismes déjà éprouvés : on sait faire, on cherche un
chiffre. La première est qualitative sur un mécanisme inconnu, et son repli
accepté (installer un autre pilote) a un coût non borné. Commencer par du
travail borné ne ferait que retarder la découverte du risque non borné. La
sonde est explicite sur l'enjeu : « si le plafond est 1, tout l'arbitrage
bascule vers la voie 4 et le chantier D change de nature ».

**L'exception ④.** Elle ne coûte aucun code (§4) et solidifie le repli au cas
où ① s'effondre. Il n'existe plus d'argument pour la repousser.

## 3. Ce que pèse réellement chaque mesure

L'inspection du banc existant déplace le centre de gravité du bloc : deux des
quatre mesures sont quasi gratuites.

> **L'ordre des sections ci-dessous n'est pas l'ordre d'exécution.** Les
> sections sont groupées par coût, pour que la lecture aille du connu vers
> l'inconnu. L'ordre d'exécution est celui du §2 : **④ → ① → ③ → ②**.

| Mesure | Code neuf | Nature |
| --- | --- | --- |
| ④ `PrintWindow` à N=4 et N=8 | **aucun** | Le banc a déjà la voie `printwindow` et le paramètre `MULTIFENETRE_N`. Deux exécutions. |
| ② Plafond d'encodage, périphériques séparés | **une dizaine de lignes** | `nvenc::plafond()` partage `capture.device()` entre tous les encodeurs ; il s'agit d'en créer un par encodeur. |
| ① Plafond de sorties virtuelles | **la pièce neuve** | Piloter le pilote d'affichage indirect depuis notre code. |
| ③ Correction d'image sur sortie virtuelle | **modéré** | Le banc sait dupliquer une sortie ; il faut lui dire laquelle, et trancher le piège d'échelle 1,5. |

## 4. Mesure ④ — `PrintWindow` à N=4 et N=8

Aucun code. Deux exécutions du banc existant :

```
MULTIFENETRE_BANC=printwindow MULTIFENETRE_N=4
MULTIFENETRE_BANC=printwindow MULTIFENETRE_N=8
```

**Réserve de méthode, à porter au document de résultats.**
`disposition::tuiles` découpe le bureau : l'aire par fenêtre **décroît** quand
N croît. Une cadence stable de N=2 à N=8 ne prouvera donc pas que le nombre de
fenêtres est neutre, seulement que le débit de pixels l'est. La sonde s'est
fait exactement cette remarque sur la voie `duplication` ; la redécouvrir
serait une régression de méthode. **Le résultat s'énonce en pixels par seconde
autant qu'en images par seconde.**

## 5. Mesure ② — plafond d'encodage sur périphériques D3D11 séparés

`nvenc::plafond()` passe aujourd'hui `capture.device()` à tous les encodeurs :
un périphérique unique et partagé. Le changement consiste à créer un
`ID3D11Device` par encodeur, à paramètres identiques (1280×720, 60 i/s,
8 Mb/s) et même plafond de recherche (16).

**Le composant fautif se nommera de lui-même.** La sonde a laissé le refus
`MF_E_UNSUPPORTED_D3D_TYPE` non attribué, mais elle a posé au passage deux
`.context()` distincts — l'un sur le `SetInputType` de l'encodeur H.264
(`agent/src/encode.rs:1438`), l'autre sur celui du convertisseur de couleur
(`agent/src/encode.rs:1224`). Le prochain refus désignera son coupable sans
travail supplémentaire.

Deux issues, deux conséquences produit opposées :

- **Le plafond ne bouge pas** → la contrainte est l'encodeur matériel. Huit
  fenêtres encodées simultanément, et la suspension de l'encodage des fenêtres
  masquées (Page Visibility API, déjà prévue par la spec de D) devient une
  **condition de viabilité**, non un confort.
- **Le plafond monte** → la contrainte était le partage du périphérique. Le
  produit gagne des fenêtres, mais hérite d'un **coût nouveau à consigner** :
  la capture produit ses textures sur un périphérique et les encodeurs vivent
  sur d'autres, donc il faudra partager les textures entre périphériques D3D11.
  Ce bloc enregistre ce coût ; il ne le résout pas (§10).

## 6. Mesure ① — piloter le moniteur virtuel depuis notre code

Le projet ne sait pas comment on commande ce pilote. La conception traite donc
① comme une **reconnaissance**, non comme l'implémentation d'un mécanisme
connu.

### 6.1 Établir le canal de contrôle

Deux hypothèses, éprouvées dans cet ordre :

1. **Une DLL de contrôle** livrée avec le pilote, dont on inspecte les exports
   (`SudoVDA.dll`, piste citée par la sonde).
2. **L'interface de périphérique du pilote** et ses IOCTL, si aucune DLL
   n'expose de point d'entrée utilisable.

Le jalon est atteint quand **un seul appel de notre code fait apparaître une
sortie DXGI**. C'est le moment de vérité du bloc.

### 6.2 Monter en N

Créer les sorties une à une. Après chaque création, relever combien de sorties
sont **réellement attachées**.

**Source de vérité : `IDXGIOutput::GetDesc`, jamais WMI.** La sonde a pris ce
champ WMI en flagrant délit de péremption : il annonçait encore une sortie
5120×1440 **68 secondes après** que DXGI eut confirmé son détachement.

Arrêt sur refus du pilote, ou à 16 sorties — par cohérence avec le plafond de
recherche de la sonde d'encodeurs.

### 6.3 Le garde-fou

**C'est la partie la plus importante de cette conception.** Un moniteur
virtuel est un **état global du système d'exploitation qui survit au
processus** : une sonde qui plante à N=5 laisse cinq moniteurs fantômes
derrière elle. Quatre dispositions :

- **Une garde à destruction automatique** qui détruit les sorties créées, y
  compris en cas de panique du thread.
- **Un relevé de la topologie initiale versé au journal avant toute
  création**, pour qu'une restauration manuelle reste toujours possible.
- **Une purge autonome**, activée par sa propre variable d'environnement, pour
  rattraper un plantage sans redémarrer la VM.
- **N'utiliser que l'ajout, jamais le remplacement.** Le
  `dd_configuration_option = ensure_only_display` qui a fait disparaître
  l'écran physique pendant la sonde est un réglage **d'Apollo**, pas une
  contrainte du pilote. En pilotant nous-mêmes, on ne demande jamais la
  désactivation de quoi que ce soit.

Le risque reste borné par un fait : l'accès à la VM passe par WinRM et le
partage CIFS, pas par son écran. Une topologie d'affichage cassée gêne, elle
n'enferme pas.

### 6.4 Critère d'abandon, borné

Le coût de ① est non borné par nature ; ce critère le borne. Les deux
hypothèses de canal de contrôle sont éprouvées, puis le pilote de
remplacement. **Si les trois résistent, la voie 2 est déclarée hors
d'atteinte** et le bloc conclut là-dessus : le chantier D se rabattra sur
`PrintWindow`, dont ④ aura entre-temps donné la tenue à N=4 et N=8.

## 7. Mesure ③ — capturer réellement sur la sortie virtuelle

Elle s'enchaîne sur ①, avec le banc existant, en trois temps.

- **Désigner la sortie.** Le banc duplique aujourd'hui celle qui porte le
  bureau ; il doit pouvoir se voir nommer une sortie précise.
- **Trancher le piège d'échelle 1,5.** Comparer le rectangle annoncé par la
  sortie (`DesktopCoordinates`) aux dimensions réelles de la texture rendue par
  l'acquisition. S'ils diffèrent, tout recadrage calculé sur le rectangle serait
  décalé d'autant. La sonde a signalé cette trouvaille sans jamais la rencontrer
  en pratique ; c'est ici qu'on la rencontre ou qu'on l'écarte.
- **Prononcer un verdict de correction** : mires sur le moniteur virtuel, avec
  recouvrement, selon le même protocole que les quatre voies de la sonde.

**Honnêteté à porter au document de résultats.** Que Windows compose réellement
des fenêtres sur un moniteur virtuel sans écran attaché est **l'hypothèse
fondatrice de la voie 2, et elle n'a jamais été vérifiée**. Si ③ rend une image
noire, la voie recommandée par la sonde s'effondre — et c'est un **résultat**,
pas un échec du bloc.

## 8. Discipline de mesure

**Un processus par sonde.** Une variable d'environnement par mesure, une seule
mesure par exécution du binaire. C'est la règle établie par la sonde, parce que
ces API échouent par **plantage du processus** (`0xc0000005` vu au jalon 1) et
non par code d'erreur : une sonde monolithique perdrait toutes les mesures déjà
faites. Trois entrées nouvelles s'ajoutent à l'aiguillage de
`agent/src/diagnostics/multifenetre.rs` : la montée en sorties virtuelles, la
purge autonome, et le choix de sortie du banc.

**Aucune trace par image.** Compteurs agrégés, journalisés à la seconde. La
leçon a déjà été payée deux fois : au chantier NAT, une trace par `Transmit`
écrite sur le partage CIFS a détruit la session qu'elle mesurait.

**Journaux versés, et en UTF-8.** Chaque mesure verse son fichier de journal
sous `docs/superpowers/plans/`, **normalisé en UTF-8 à la capture**. Les
journaux de la sonde mêlent UTF-16LE et UTF-8 avec BOM, au point que
`CLAUDE.md` doit expliquer comment les lire — un `grep` direct sur les premiers
ne trouve rien. Ce bloc ne reproduit pas ce défaut.

**Pas de chiffre sans journal.** C'est la seule faiblesse que la sonde a
laissée, et elle porte précisément sur la voie qu'elle recommande.

## 9. Tests et conventions

**Ce qui n'aura pas de filet.** L'essentiel de ce code est `#[cfg(windows)]` et
ne tourne que sur la VM. Comme le banc existant, les appels au pilote et
l'acquisition DXGI sont éprouvés par la mesure seule.

**Ce qui sort derrière une frontière testable**, et doit l'être :

- **la garde de destruction** — qu'elle détruise bien tout ce qui a été créé, y
  compris en cas de panique, vérifié avec un pilote factice ;
- **le calcul du facteur d'échelle** entre rectangle annoncé et texture réelle —
  du calcul pur, et précisément le piège signalé au §7 ;
- **la sélection de sortie** par nom ou index.

**Taille des fichiers.** `agent/src/diagnostics/multifenetre/voies.rs` est à
402 lignes : lui ajouter la sélection de sortie l'amènerait près du plafond de
500 posé par `CLAUDE.md`. Le choix de sortie sort donc dans son propre module.

## 10. Hors périmètre, explicitement

- **Aucun code de production.** La brique de pilotage du moniteur virtuel sera
  promue au chantier D, pas ici.
- **Le diagnostic de `0x800706BE`** (l'erreur RPC de `Windows.Graphics.Capture`)
  et **l'instrumentation de `DwmGetDxSharedSurface`**. La sonde les classe
  hors chemin critique, les deux réparations bon marché de WGC étant déjà
  écartées.
- **Le partage de textures entre périphériques D3D11**, consigné comme coût
  au §5, jamais résolu ici.
- **La spécification du chantier D elle-même.** Elle vient après, nourrie par
  les quatre chiffres.

## 11. Critère de réussite du bloc

Quatre chiffres au journal, chacun accompagné de son fichier de journal versé
au dépôt :

1. Combien de sorties virtuelles simultanées ce pilote accepte — ou, à défaut,
   lequel des trois canaux de contrôle a résisté.
2. Combien d'encodeurs H.264 matériels tiennent sur des périphériques D3D11
   séparés, et lequel des deux `SetInputType` refuse le suivant.
3. Si une fenêtre posée sur un moniteur virtuel est capturée correctement, et
   si le rectangle annoncé s'accorde aux dimensions de la texture.
4. La cadence de `PrintWindow` à N=4 et N=8, en images **et** en pixels par
   seconde.

À l'issue, `2026-07-28-support-jeux-design.md` §5 D perd sa mention « deux
mesures bloquantes » et le chantier D devient spécifiable.

## 12. Prérequis d'exécution

La VM Windows est éteinte au moment d'écrire cette conception
(`virsh list --all` → `fermé`). Toute mesure de ce bloc exige la séquence de
démarrage documentée dans `CLAUDE.md` § « Cycle de vie de la VM Windows » —
dont l'attente d'un **accès réel** à `/media/vm`, et non de la seule présence
de l'entrée de montage CIFS.
