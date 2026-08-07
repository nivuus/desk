# Sous-bloc D10 — solder la branche : la sortie cesse d'être la fenêtre

**Date** : 7 août 2026
**Prédécesseur** : sous-bloc D9 (`2026-08-06-multifenetres-solder-la-dette-design.md`),
fusionné en `c9b7a31`, qui laisse **dix legs**.
**Objet** : les solder tous.

---

## 1. Objet, et pourquoi un seul bloc

D9 devait fermer les douze legs de D8. Il en a fermé une partie sur pièces, en a
fait disparaître deux (le changement de mode de sortie a été **retiré** plutôt
qu'armé), en a laissé plusieurs non exercés faute de pouvoir les provoquer, et il
en a découvert de neufs. Il en reste **dix**, dont **deux marqués 🔴 parce
qu'ils mordent aujourd'hui** :

| # | Leg | État à l'entrée de D10 |
| --- | --- | --- |
| 1 | Reconstruire la capture audio après sa mort | remède **inerte** dans le cas majoritaire |
| 2 | La course F5 sur le **second** registre (`capteur/serveur.rs::oublier`) | ouverte |
| 3 | L'A/B sur `set_desired_bitrate` | joué, **n'établit rien** |
| 4 | 🔴 La pollution de registre | **plafonne le produit à trois fenêtres** |
| 5 | 🔴 Borner la taille de sortie demandée | la fonction existe, **sans appelant** |
| 6 | `REARMEMENTS_MAX` ne mord pas dans le cas majoritaire | ouverte |
| 7 | Le maillon fautif du leg 10 de D8 | non identifié |
| 8 | Le cinquième déclencheur de mort de capture audio | jamais essayé |
| 9 | `survie_verdict.rs` posé à la racine du crate | deux conventions coexistent |
| 10 | Neuf constats de revue parqués sur la tâche 14 de D9 | dans son rapport versé |

**Un seul bloc, et pas dix**, parce que trois d'entre eux ne sont qu'un sujet et
que l'ordre est imposé : le leg 4 **rend au produit sa capacité de mesure**.
Tant qu'il tient, toute mesure de capacité porte sur trois fenêtres et ne se
compare à aucune campagne de D4 à D6 — c'est déjà ce qui a privé l'A/B (leg 3)
de sa base de comparaison en D9.

**Décision de périmètre du propriétaire du dépôt** : les dix, sans exception,
audio et A/B compris.

---

## 2. Forme du bloc, et son ordre

**Pas de phase éliminatoire en tête, contrairement à D9.** D9 s'ouvrait sur une
sonde parce qu'une inconnue commandait tout le reste (« le pilote accepte-t-il un
changement de mode sur une sortie dont la duplication est ouverte ? »). Ici la
voie retenue pour le leg 4 décide **par construction** : elle ne dépend d'aucune
mesure préalable, et elle rend sans objet la seule question de pilote encore
ouverte.

L'ordre est imposé par une seule dépendance :

1. **Les trois extractions de plafond** (§8), placées **avant** le code
   qu'elles accueillent ;
2. **Famille ① — le bloc 🔴** (legs 4 et 5), puis sa recette : huit fenêtres,
   **registre laissé sale** ;
3. **Famille ② — l'audio** (legs 1, 6, 8), puis sa recette ;
4. **Famille ④ — l'A/B apparié** (leg 3), qui a besoin du point 2 ;
5. **Famille ③ — les legs froids** (legs 2, 7, 9, 10) ;
6. **La revue transverse de fin de branche**, obligatoire (§7.4).

---

## 3. Famille ① — la sortie cesse d'être la fenêtre (legs 4 et 5)

### 3.1 Le fait à réparer

`superviseur/boucle.rs:312` exige que la sortie apparue corresponde à la taille
demandée, à `placement::TOLERANCE_PX` (4 px) près, et **la rend au pilote**
sinon. Or une sortie virtuelle **ne naît pas à la taille demandée** : elle naît à
la dernière taille laissée au registre par un `CDS_UPDATEREGISTRY` antérieur
(établi par D8, tâche 3bis, **une exécution, chaîne `avant(N) = après(N-1)` sur
trois transitions — confirmée et reproduite, jamais expliquée**).

Sur cette VM, le registre est resté à 3840×2160 : **aux six exécutions de la
recette ③ de D9, sans exception, trois sessions s'établissent** et toutes les
tentatives au-delà échouent sur `sortie créée mais introuvable dans la topologie
DXGI`.

Le produit n'écrit plus au registre depuis D9 — mais **rien ne nettoie ce qui y
a déjà été écrit**, et la portée du blocage (par GUID ou globale) n'est pas
tranchée.

### 3.2 La voie retenue : tolérer et recadrer

Trois voies ont été pesées : **normaliser le registre** au démarrage du
superviseur (réintroduit dans le produit l'écriture registre que D9 vient d'en
retirer, sur un mécanisme non expliqué, et exige d'abord de trancher la portée) ;
**tolérer et étirer** (on encoderait à la taille du registre, alors que le
plafond de 8 encodeurs concurrents **n'a jamais été mesuré au-delà de
1280×720** — NVENC borne en macroblocs par seconde, pas en sessions) ; et
**tolérer et recadrer**, retenue.

**Le changement tient en une phrase.** Aujourd'hui la sortie *est* la fenêtre :
`WindowsSource::sur_sortie` calcule sa région par `region_de_sortie(dw, dh)`, où
`(dw, dh)` est la taille de la **sortie**. Demain, la fenêtre est **posée à
l'origine de sa sortie, à la taille retenue**, et la région capturée est ce
rectangle-là.

**La taille retenue** vaut, sur chaque axe :

```
retenue = min( borner_a_la_taille_max(viewport) , taille réelle de la sortie )
```

- née **trop grande**, la sortie est recadrée ;
- née **trop petite**, elle est honorée à ce qu'elle offre, et le client met à
  l'échelle ;
- **aucun cas ne rend plus jamais une sortie au pilote pour une question de
  taille.**

### 3.3 ⚠️ Le risque de fuite entre sessions est écarté PAR CONSTRUCTION

L'en-tête de `windows_source/sortie.rs` (l. 20-29) documente un danger réel : un
repli de duplication vers le **bureau physique** fait afficher un coin du bureau
réel de la VM dans la fenêtre d'un utilisateur — « en multi-fenêtres il fait fuir
le contenu d'un moniteur vers la session d'autrui ».

**Ce danger appartient à `ModeCapture::FenetreRecadree`**, qui duplique le bureau
(`DesktopCapture::new()`). **La voie retenue ne l'emprunte pas** : elle recadre à
l'intérieur de la duplication **de la sortie elle-même**
(`DesktopCapture::sur_sortie`), qui ne touche jamais le bureau physique. Le mode
reste `SortieEntiere`, et `redimensionne_la_fenetre()` continue de rendre
**faux**.

*(Une rédaction antérieure de cette conception présentait ce risque comme une
condition à lever ; c'était une erreur d'analyse, corrigée ici avant écriture du
code.)*

### 3.4 Les quatre points de couture

| Où | Aujourd'hui | Demain |
| --- | --- | --- |
| `superviseur/boucle.rs:298` | crée à la taille du viewport brute | crée à `borner_a_la_taille_max(viewport)` — **c'est le leg 5** |
| `superviseur/boucle.rs:312` | refuse hors de ±4 px | accepte toute sortie **au moins aussi grande**, parmi les seules sorties **apparues** |
| `windows_source/sortie.rs::region_de_sortie` | rectangle plein de la sortie | rectangle de la **taille retenue**, à l'origine, tronqué à la sortie |
| `superviseur/placement.rs` | réimpose rect fenêtre == rect moniteur | réimpose rect fenêtre == origine de la sortie + taille retenue |

⚠️ **Le filtre sur les sorties APPARUES doit être préservé tel quel.** Le
commentaire de `boucle.rs:271-280` explique pourquoi : sans lui, un critère
« au moins aussi grande » apparierait volontiers un **moniteur physique**
préexistant, et la fenêtre serait posée sur l'écran réel de la VM. Le passage
d'une égalité à une inégalité rend ce filtre **plus** nécessaire, pas moins.

⚠️ **`TOLERANCE_PX` ne disparaît pas** : le replacement périodique
(`placement.rs:86`) continue de s'en servir pour juger si la fenêtre a dérivé.
Seul l'**appariement** cesse de l'employer.

### 3.5 Ce que cette famille ferme, et ce qu'elle n'ouvre pas

- Le plafond de trois fenêtres disparaît **sans aucune écriture registre**.
- La question « le mode registre est-il par GUID ou global ? » devient **sans
  objet, pas résolue** — elle reste inconnue, et la spec ne prétendra pas
  l'inverse.
- Le **redimensionnement par fenêtre reste inerte** (`SortieEntiere`). La voie
  retenue le rendrait techniquement possible ; l'activer est un **comportement
  neuf**, pas un leg, et il est **hors périmètre** (§12).

---

## 4. Famille ② — l'audio mort : reconstruire, et savoir le provoquer

### 4.1 Le fait à réparer

Quand `capture.read()` échoue plus de `LECTURES_ECHOUEES_MAX` fois d'affilée
(tolérance ajoutée par F3 de D7), le fil de capture de `windows_audio.rs` pose
`capture_morte` et exécute un `return` **définitif** (l. 366 et 414). Côté
capteur, réélire la même session ne fait que pousser `Audio { actif: true }` →
`set_actif(true)`, **qui n'écrit qu'un booléen atomique que ce fil mort ne relira
jamais**.

**Rien, nulle part, ne reconstruit la source.** Une fenêtre seule de son groupe de
PID — le cas majoritaire, une application une fenêtre — perd son son pour le
restant de la session. La seule moitié qui fonctionne est la **promotion d'une
voisine** du même groupe, qui a sa propre capture sur un fil qui n'a jamais
échoué.

### 4.2 Le remède : un reconstructeur confié à la session

Ce qui manque n'est pas un mécanisme — `Session::set_audio_source`
(`transport/piste_audio.rs:26`) est publique et se rappelle — mais **de quoi
refabriquer la source** : `demarrage/audio.rs::brancher` sait la construire, n'est
appelé qu'une fois, et détient seul le `config` et le `clock_origin` nécessaires.

- **`demarrage/audio.rs`** construit, en plus de la source, un **reconstructeur**
  — une fermeture qui refait exactement le même choix de mode (par PID via
  `pid_de_fenetre`, ou mix de session) — et le confie à la `Session` à côté de la
  source.
- **La branche a1sexies de `transport/tick.rs`** (l. 285-289), qui détecte déjà
  la mort et pose le verrou `audio_mort_signale`, **tente d'abord la
  reconstruction** : **bornée en nombre**, et espacée par une temporisation
  propre. ⚠️ **Ne pas réemployer `audio::temporisation_de_reprise`** : elle
  cadence les relectures *dans* le fil de capture, pas les reconstructions de
  source — deux durées de sens différent qui divergeraient en silence. La
  nouvelle constante est **non calibrée**, et sa doc le dira.
  Si la reconstruction aboutit, la session retrouve son son sans que le capteur
  ait rien à faire.

⚠️ **La branche a1sexies court sur le fil de `Session::run`.** Ouvrir une source
WASAPI y est un appel bloquant de durée non bornée : la reconstruction est donc
tentée **au plus une fois par temporisation**, et si la mesure montre qu'elle
retarde le drainage, elle passe sur un fil. C'est le même risque que
`Drop for H264Encoder` porte déjà sur ce fil (`CLAUDE.md`, chantier des
duplications parallèles) — nommé ici plutôt que découvert.
- **`AudioMort` cesse d'être le premier geste et devient le repli** : il n'est
  émis qu'une fois la reconstruction épuisée — c'est-à-dire quand il n'y a plus
  rien à reconstruire, l'arbre de processus ayant disparu. La promotion d'une
  voisine garde alors son rôle exact.

⚠️ **`transport/` ne connaît qu'un objet de trait et une fermeture** : rien de
Windows n'y entre. La chaîne se teste donc sur l'hôte Linux avec un
reconstructeur factice qui échoue *k* fois puis réussit.

### 4.3 Le leg 6 se referme par là, et pas autrement

`REARMEMENTS_MAX` (5) ne mord pas aujourd'hui parce que
`sommeil/porteurs.rs:117` remet le compteur à zéro dès qu'une session est
**décidée** porteuse — et pour une fenêtre seule de son groupe, la sortie de répit
la rend automatiquement porteuse. Le garde-fou est décoratif dans le cas
majoritaire.

Il doit être remis à zéro sur une **preuve**, pas sur une décision : un message
`VersCapteur::AudioVivant`, symétrique d'`AudioMort`, émis quand la reconstruction
a réussi **et** que des paquets repartent.

⚠️ **`capteur/pont_media.rs` porte un bras catch-all `Ok(autre) => return` qui
tue le fil `lire_le_media` EN SILENCE.** Ce fichier a déjà été payé quatre fois
(D5 `Sommeil`, D6 `Part`, D7 `Audio`, D8 `PleinEcran`). `AudioVivant` circule dans
l'autre sens (enfant → capteur, comme `AudioMort`), donc ne passe pas par ce
`match` ; **le vérifier explicitement fait partie de la tâche**, et
`transport/controle.rs` porte un `match` exhaustif qui, lui, se signale seul.

### 4.4 Le leg 8, et ce que la mesure ne promettra pas

Les quatre déclencheurs de D9 (`Restart-Service Audiosrv`, `Stop`/`Start`,
`Stop-Process audiodg`, `Disable`/`Enable-PnpDevice`) n'ont produit **aucune**
ligne `lecture audio échouée` sur **9 exécutions versées** : la capture *process
loopback* suit l'**arbre de processus**, pas le service ni le périphérique. Toute
disruption à ces niveaux est structurellement le mauvais levier.

Le cinquième déclencheur nommé par D9 — tuer le `chrome.exe` **cible** — **tue
aussi la fenêtre**, donc la session : il ne provoque pas ce qu'on veut observer.
Deux choses distinctes, tenues distinctes :

1. une tentative réelle sur un **processus enfant** de l'arbre (un renderer), qui
   laisse la fenêtre vivante — **issue inconnue, elle peut ne rien produire** ;
2. et, quoi qu'il advienne de la première, une **injection de faute** de banc,
   `AUDIO_FAUTE_LECTURE=<n>`, qui fait échouer les *n* prochaines lectures.

⚠️ **L'injection de faute établit que le remède fonctionne, jamais qu'une cause
naturelle existe.** C'est exactement la distinction que D9 a payée en croyant
mesurer un remède qu'aucun déclencheur n'atteignait.

**Convention de la variable** : celle de `PART_SONDAGE` — **variable de BANC,
jamais une configuration livrée**, lue dans l'enfant, transmise par
`scripts/run-agent.sh` (piège payé en D1, D2 et D6 : **toute variable neuve doit y
être ajoutée explicitement**, sinon l'agent démarre sans elle et sans rien
signaler). Trace émise seulement si armée.

---

## 5. Famille ③ — les legs froids (2, 7, 9, 10)

### 5.1 Leg 2 — la course F5 sur le second registre

`capteur/serveur.rs:373` fait un `remove` inconditionnel. C'est la course F5
(préexistante, nommée par D7) que D9 a fermée sur le registre de **sommeil**
seulement : le brief de sa tâche 10 ne nommait que `sommeil`.

Le patron est écrit, éprouvé et transposable : `sommeil/registre.rs`
(`generations`, `prochaine_generation`, `retirer_est_perime`). ⚠️ **Le frapper
au bon endroit** : la première version du leg 2 en D9 attachait la génération au
**lancement du processus** quand la course est à l'**attache** —
`retirer_est_perime` ne pouvait alors structurellement pas rendre `true` en
production.

### 5.2 Legs 7 et 10 — le maillon du `Resize`, et les constats parqués

**Premier geste, avant toute mesure** : corriger le rapport de la tâche 14 de D9,
qui déclare le leg 10 de D8 clos **en sens inverse de ses propres journaux**.
Sans quoi on repart de sa conclusion.

L'inventaire réel (`agent-critere-1-1.log`) : `w-2` = 3 `Visibility` / **0
`Resize`** ; `w-3` = 3/3 ; `w-5` = 1/**0** ; `w-7` = 13/1. **Deux sessions sur
quatre, canal de contrôle démontré vivant, zéro `Resize`.**

⚠️ **Le canal vivant ÉCARTE l'hypothèse « canal mort » pour ces deux sessions ;
il ne DÉSIGNE PAS le client** — c'est la nuance que la correction C3 de D8 a
payée. Depuis D9 la trace `contrôle reçu` porte son `session`, donc la question
est instrumentable : `video.clientWidth`/`clientHeight` côté client, le
`ResizeObserver` de `client/src/main.ts` étant l'hypothèse la plus proche.

Les **neuf constats parqués** sur cette même tâche sont repris un à un : chacun
est soit traité, soit **requalifié explicitement** (dont un « défaut
d'instrument » qui est en réalité un **comportement du produit** — la page-shell
réémet `fenetre-ouverte` pour une fenêtre déjà ouverte, mécanisme non élucidé).

### 5.3 Leg 9 — trancher une convention, pas déplacer un fichier

Le dépôt a **deux conventions** pour un module enfant :

- à la racine nue : `geometry.rs`, `sortie_dxgi.rs`, `survie_verdict.rs` ;
- chez le parent, hissé par `#[path]` : `capture_reprise` (D2), `windows_source_sortie`
  (D1), `windows_source_telemetrie` (D9).

Le leg n'est pas « déplacer `survie_verdict.rs` » : c'est **choisir la règle,
l'écrire dans `CLAUDE.md`**, puis y conformer ce fichier-ci. Sans quoi
l'arbitrage se rejouera à l'identique au prochain module.

---

## 6. Famille ④ — l'A/B sur `set_desired_bitrate` (leg 3)

D9 a joué l'A/B et **il n'établit rien** : +23,2 % entre bras (moyennes 8,843 et
7,180 Mb/s cumulés) contre **+83,1 % de variance intra-bras** (11,439 contre
6,247 sur le même bras armé). **Le bruit dépasse le signal.**

**Ajouter des exécutions à ce montage ne convergera pas**, parce que le facteur
dominant est la charge de l'hôte — D6 l'avait déjà nommé, et sa recette a vu la
performance varier d'un facteur 19 à binaire et protocole identiques.

**Changer le plan d'expérience, pas le montage** :

- bras **alternés A/B/A/B dos à dos**, dans la même fenêtre de charge ;
- comparaison **par paires** consécutives, jamais de moyennes de bras ;
- **au moins quatre paires**, et le verdict énonce le nombre de paires et le
  signe de chacune, pas seulement leur moyenne.

La dérive de l'hôte s'annule dans chaque paire. ⚠️ **À jouer après la famille ①**,
sinon il se rejoue à trois fenêtres comme D9.

⚠️ **Le résultat peut rester « n'établit rien », et c'est une issue acceptable** :
la spec ne promet pas un verdict, elle promet un plan d'expérience qui, lui,
peut trancher.

---

## 7. Recettes et critères de réception

### 7.1 Critères

| # | Critère | Comment il est jugé |
| --- | --- | --- |
| ① | **Huit fenêtres s'établissent, registre laissé sale** | `enfant lancé` et `fenêtre attachée au capteur` = 8 dans `agent.log`, et **zéro** ligne `sortie créée mais introuvable dans la topologie DXGI`. Trois aujourd'hui |
| ② | **L'image livrée est celle de la fenêtre, pas un coin de bureau** | sur une sortie **née trop grande**, chaque page décode des images (`framesDecoded` en croissance) et la fenêtre montre **son** application seule. Source **animée** à cadence connue, sans quoi une capture lente et une source lente se lisent pareil |
| ③ | **Une capture audio morte est reconstruite** | sous `AUDIO_FAUTE_LECTURE`, le son repart ; `AudioVivant` au journal ; **aucun** `abandon définitif` |
| ④ | **Une capture irrécupérable retombe sur la promotion** | reconstructeur épuisé → `AudioMort` → voisine promue, comme D9 le livrait déjà |
| ⑤ | **L'A/B apparié** | nombre de paires, signe de chacune, et verdict qui peut être « n'établit rien » |

**Deux exécutions par critère, pas une** (règle héritée de D9). **Aucun taux ne
sera revendiqué.**

### 7.2 « Vu rouge » est une exigence, pas une formule

Doctrine du dépôt, payée trois fois (F1 de D7, la sonde P1 de D8, le confondeur
de D9) : **un contrôle qu'on n'a jamais vu rouge n'est pas un contrôle**. Pour
chaque critère, la spec exige que l'on **provoque délibérément l'état qu'il doit
dénoncer** et qu'on vérifie qu'il le dénonce :

- ① se voit rouge sur le binaire d'aujourd'hui — registre à 3840×2160, trois
  sessions ;
- ③ se voit rouge en désarmant la reconstruction ;
- et le contrôle doit **pouvoir** échouer : vérifier que la trace observée a plus
  d'une valeur atteignable, pas seulement qu'elle apparaît.

### 7.3 Montages

Tous acquis des sous-blocs précédents : fenêtres **Chrome `--app` animées** à
cadence connue (`instrument/anim-d4.html`), **un `--user-data-dir` par fenêtre**
(sans quoi on compte des lancements et non des fenêtres), navigateur pilote sur
l'**hôte** et jamais sur la VM, et `Get-Process agent` revérifié **après chaque
tentative, y compris échouée**.

⚠️ **Le registre est laissé SALE à dessein pour le critère ①** — c'est l'état qui
plafonne le produit aujourd'hui, et le rétablir à 1280×720 par la sonde
`MULTIFENETRE_MODE_SORTIE` **annulerait la mesure**.

### 7.4 Revue transverse de fin de branche — obligatoire

Elle a trouvé **cinq** défauts en D7, **trois** Critiques en D8, **six** en D9, et
**tous franchissaient une frontière de tâche** : chacun était correct des deux
côtés pris séparément. Une revue par tâche ne peut structurellement pas les voir.

Sa cible propre en D10 : **les affirmations de code devenues fausses dans leur
propre branche** — trois des six défauts de D9 étaient de cette nature. Les
commentaires qui décrivent aujourd'hui « la sortie *est* la fenêtre » sont
nombreux, et la famille ① les réfute tous.

---

## 8. Le plafond de 500 lignes, budgété d'avance

**Relevé par la commande le 7 août 2026**, et non recopié :

| Fichier | Lignes | Marge | Rôle dans D10 |
| --- | --- | --- | --- |
| `agent/src/superviseur/table.rs` | **494** | **6** | **pas sur le chemin prévu** — mais c'est la marge la plus serrée du dépôt après `encode/arret.rs` : s'il faut y toucher, l'extraction précède l'addition, sans exception (ses tests de rétention vivent déjà à part) |
| `agent/src/superviseur/boucle.rs` | **492** | **8** | **le cœur de la famille ①** |
| `agent/src/transport/tick/tests.rs` | **489** | **11** | **les tests de la branche a1sexies** |
| `agent/src/windows_audio.rs` | **479** | **21** | **le fil de capture** |

Les trois derniers reçoivent une **tâche d'extraction dédiée, placée AVANT celle
qui y ajoute du code**. C'est le seul geste qui a fonctionné en D9
(`serveur/instances.rs` : marge rendue de 10 à 65, et la tâche suivante a pu y
ajouter son code sans rien franchir) ; les deux fichiers traités après coup y ont
été **compressés** — geste que `CLAUDE.md` interdit nommément — **puis extraits
quand même**.

Points de chute nommés :

- `windows_audio.rs` → **`agent/src/windows_audio/`**, en commençant par le corps
  du fil de capture (point de chute déjà inscrit dans `CLAUDE.md` depuis D7) ;
- `transport/tick/tests.rs` → un module de tests par famille de branche ;
- `superviseur/boucle.rs` → le calcul d'appariement et de taille retenue, qui est
  **pur** et a donc vocation à sortir de toute façon (§10).

---

## 9. Gestion des erreurs

- **Une sortie née trop petite pour être exploitable** (`region_de_sortie` rend
  `None`) reste un échec, et la sortie est rendue au pilote comme aujourd'hui :
  la famille ① supprime le refus **pour cause de taille**, pas le refus pour
  cause de sortie inexploitable.
- **La reconstruction audio épuisée** n'est pas une erreur fatale : la session
  continue **muette**, et `AudioMort` part au capteur. Le repli n'est **jamais**
  le mix global — c'est l'arbitrage écrit au cadrage de D7, et il ne se rouvre
  pas ici.
- **`AudioVivant` perdu** laisse `REARMEMENTS_MAX` à sa valeur courante : le
  garde-fou se referme trop tôt plutôt que jamais. Dégradation choisie.
- **La branche a1sexies ne doit pas mettre de paquet en file** : c'est
  l'invariant de drainage de `transport/tick.rs`, et D6 a payé deux rondes pour
  l'énoncer correctement.

---

## 10. Stratégie de test

**Tout ce qui peut être pur l'est**, et se teste sur l'hôte Linux :

- le calcul de la **taille retenue** et le nouveau critère d'appariement (aucun
  `cfg`, aucun objet COM) ;
- `borner_a_la_taille_max` et `region_de_sortie` sont **déjà** purs et testés :
  ils gagnent les cas de la taille retenue ;
- la **course** du leg 2, jouée dans le test comme `sommeil/registre.rs` la joue ;
- la **chaîne de reconstruction audio**, avec un reconstructeur factice qui
  échoue *k* fois puis réussit — c'est ce qui permet de la voir rouge sans VM.

Vérifications de fin de branche, comme en D9 : `cargo test -p agent`,
`cargo check --target x86_64-pc-windows-gnu` (types, emprunts, visibilités et
durées de vie ; **pas l'édition de liens**), et `npx vitest run` côté client.

---

## 11. Ce que D10 n'établira PAS

- **La portée du blocage registre** (par GUID ou globale) : rendue **sans objet,
  pas résolue**.
- **L'existence d'une cause naturelle** de mort de capture audio, si le
  déclencheur réel du §4.4 ne produit rien. L'injection de faute n'en dit rien.
- **Le coût de la duplication d'une sortie surdimensionnée**, que la voie retenue
  introduit et que rien ne mesure ici.
- **Le plafond de 8 encodeurs au-delà de 720p** : le bornage à `TAILLE_MAX_SORTIE`
  (1920×1080, **non calibrée**) limite le risque, il ne le mesure pas.
- **Les trois couches inconnues du chantier D** : le plafond de 8 encodeurs,
  celui de 4 processus, et le mécanisme de l'abandon du mutex DXGI.
- **La latence de bout en bout**, qu'aucun sous-bloc du chantier D n'a jamais
  mesurée.
- **Aucune constante n'est calibrée** par un jugement visuel ou d'écoute —
  `BPP_MIN`, `FACTEUR_FOCUS`, `PART_DORMANTE_BPS`, `HYSTERESIS`,
  `REPIT_APRES_ECHEC`, `TAILLE_MAX_SORTIE`, `REPIT_REARMEMENT_AUDIO`,
  `REARMEMENTS_MAX`.
- **La visibilité et le focus restent imposés par le pilote de recette**, page
  par page — limite héritée de D5, qu'aucun sous-bloc n'a levée.
- **Aucun client réel, aucun HiDPI réel.**

---

## 12. Hors périmètre

- **Réactiver le redimensionnement par fenêtre.** La voie retenue le rend
  techniquement possible (recadrage mobile dans une sortie plus grande) ; c'est un
  **comportement neuf**, pas un leg.
- **Nettoyer le registre**, sous quelque forme que ce soit : la voie retenue rend
  le produit indifférent à son état.
- **Expliquer** la non-persistance du changement de mode, ou la naissance d'une
  sortie à la taille du registre. Les deux restent des faits mesurés et non
  expliqués.
- **Identifier** la couche du plafond de 8 encodeurs ou de 4 processus.
- La latence de bout en bout, le recouvrement, le déplacement de fenêtre, le
  clavier concurrent, les applications UWP.
