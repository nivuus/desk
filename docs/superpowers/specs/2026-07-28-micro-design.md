# Chantier E — Microphone

**Date** : 28 juillet 2026
**Statut** : Spécification validée, prête à planifier
**Portée** : Porter la voix de l'utilisateur, captée par son navigateur, jusqu'à
un périphérique de capture que les applications Windows de la VM voient comme un
microphone ordinaire

---

## 1. Objectif

Le chantier A (`2026-07-28-audio-design.md`) porte le son **de la VM vers le
navigateur**. Le sens inverse n'existe pas, et il est explicitement hors du
périmètre de A :

> **Le microphone** (navigateur → VM). Chantier distinct : capture navigateur,
> décodage côté agent, périphérique d'entrée virtuel sous Windows.

Ce chantier est ce chantier distinct. Sans lui, aucune application Windows
lancée dans la VM ne dispose d'un microphone : ni la visioconférence citée par
l'objectif de A, ni le vocal de jeu, ni la dictée.

Le besoin retenu est **générique** : il ne s'agit pas de servir un usage
particulier, mais qu'une application Windows qui demande un microphone en trouve
un. Les exigences sont donc l'union des trois cas :

| Usage | Ce qu'il impose |
| --- | --- |
| Visioconférence dans la VM | Écho maîtrisé, latence bout-en-bout tolérable jusqu'à ~150 ms |
| Vocal de jeu | Latence plus serrée, coexistence avec une VM chargée |
| Dictée, reconnaissance vocale | Fidélité du signal : pas de coupure, pas de trou non comblé |

## 2. Dépendance au chantier A

**E suppose A livré.** Il réutilise :

- `agent/src/opus.rs` — l'enveloppe libopus, à laquelle E ajoute le décodage ;
- la plomberie COM et la connaissance WASAPI établies par `agent/src/wasapi.rs`.

À la date de rédaction, A n'est que spécifié : aucun `audio.rs` n'existe dans
`agent/src`. E vient donc après A dans l'ordre des chantiers.

## 3. La contrainte qui commande tout : Windows n'offre rien

Rien dans Windows ne permet d'alimenter une entrée audio par programme. Un
périphérique de capture est présenté par un **pilote** ; il n'existe pas d'API,
même privée, pour en fabriquer un depuis l'espace utilisateur.

Trois voies étaient possibles : un pilote tiers déjà signé, un pilote
open-source compilé et signé par nous (sample MSFT Sysvad ou équivalent MIT), ou
un contournement — dont l'existence n'est pas établie.

**Décision : VB-Cable, installé dans l'image de base de la VM.** Pilote signé,
gratuit, zéro code pilote à écrire, opérationnel immédiatement. Le pilote expose
une paire : « CABLE Input » est un endpoint de **rendu**, « CABLE Output » un
endpoint de **capture**, et tout ce qui est joué sur le premier ressort sur le
second. L'agent joue donc la voix reçue sur CABLE Input ; les applications
Windows sélectionnent CABLE Output comme microphone.

Conséquence heureuse, à ne pas perdre de vue : **CABLE Input n'est pas le
périphérique de rendu par défaut**, donc le loopback du chantier A ne le capture
pas. La voix de l'utilisateur ne repart pas vers son propre navigateur. Aucune
boucle locale n'est créée par construction.

### Réserve de licence — à régler avant commercialisation

La licence VB-Audio est gratuite en **usage personnel**. La distribution dans un
produit commercial exige un accord de distribution auprès de VB-Audio. Cela
n'empêche rien aujourd'hui et ne conditionne aucune décision technique de ce
document : la seule pièce spécifique au pilote est le nom de l'endpoint visé.
Mais le point doit être réglé avant toute mise sur le marché, et un
remplacement ultérieur par un pilote open-source signé ne toucherait qu'à
`wasapi_render.rs`.

## 4. Décisions actées

| Décision | Choix | Justification |
| --- | --- | --- |
| Périphérique virtuel | VB-Cable dans l'image de base | Voir §3. Seule voie sans écrire ni signer de pilote |
| Transport | Piste média WebRTC montante, Opus | Le navigateur fournit déjà encodage, horodatage, séquencement, FEC et DTX. Un canal de données obligerait à les réécrire |
| Topologie SDP | Seconde m-line audio **`sendonly`**, distincte de celle de A | Voir §5. Découple les deux sens : sources, puits et activation n'ont rien en commun |
| Activation | À la demande, bouton de bascule, `replaceTrack` | Le micro capte l'utilisateur chez lui : il reste fermé tant qu'il n'est pas allumé. `replaceTrack` évite la renégociation (§5) |
| Concurrence | Exclusivité : un seul flux montant accepté par l'agent | Il n'y a qu'un câble. La règle est fixée maintenant pour que le comportement à deux fenêtres soit choisi, pas subi |
| Annulation d'écho | Celle du navigateur (`echoCancellation`) | Le son de la VM est joué par l'onglet lui-même, donc connu de l'AEC de Chrome. En écrire une côté agent serait refaire ce qui existe |
| Rééchantillonnage | **Refusé** ; un format ≠ 48 kHz est rejeté avec un message nommant la fréquence | Même règle que A §5. Écrire un rééchantillonneur pour un cas dont on ignore s'il se produit serait spéculatif |

## 5. Pourquoi `replaceTrack`, et pas une renégociation

Le signaling ne sait pas renégocier. `connectSession` fait un aller-retour
unique offre → réponse (`client/src/webrtc.ts:199-208`), sans trickle ICE et sans
aucun chemin pour une seconde offre. Or « micro à la demande » suggère
naïvement d'ajouter une piste en cours de session — c'est-à-dire exactement une
renégociation.

La solution est celle que WebRTC prévoit pour ce cas :

1. l'offre initiale déclare un transceiver `sendonly`, **sans piste** — rien
   n'est capté, aucune permission n'est demandée, aucun octet n'est émis ;
2. au clic, `getUserMedia` puis `sender.replaceTrack(micTrack)` : le flux part.
   Le codec ne changeant pas, aucune renégociation n'est nécessaire ;
3. à l'extinction, `sender.replaceTrack(null)` **et** `track.stop()`.

Le coût est une m-line négociée en permanence, même inutilisée. Le gain est que
« à la demande » devient réalisable sans construire d'abord la renégociation,
qui serait un chantier à elle seule.

### Pourquoi une m-line séparée plutôt que `sendrecv`

La visioconférence classique emploie une seule m-line audio bidirectionnelle. Ce
serait ici un couplage sans contrepartie : le même `mid` porterait l'écriture du
loopback de A **et** la lecture du micro, dans le même code de transport, alors
que les deux directions n'ont ni la même source, ni le même puits, ni la même
activation, ni le même cycle de vie. Deux m-lines gardent les deux chantiers
indépendants — E ne dépend pas du SDP de A.

## 6. Modules

Le découpage suit celui de A, avec la même ligne de partage : ce qui est
testable sous Linux est séparé de ce qui exige Windows.

| Fichier | Responsabilité | Portable |
| --- | --- | --- |
| `agent/src/opus.rs` (modifié) | Décodage : trame normale, dissimulation de perte (PLC), reconstruction FEC | oui |
| `agent/src/mic.rs` (créé) | Trait `MicSink`, tampon de gigue `JitterBuffer`, puits de test | oui |
| `agent/src/wasapi_render.rs` (créé) | Rendu WASAPI sur un endpoint désigné par nom ; mode partagé ; piloté par événement | Windows |
| `agent/src/windows_mic.rs` (créé) | Assemble tampon + décodeur + rendu sur un fil dédié ; implémente `MicSink` | Windows |
| `agent/src/transport.rs` (modifié) | `mic_mid`, `Event::MediaData` → dépôt, garde d'exclusivité | |
| `agent/src/main.rs` (modifié) | Construit le puits micro, ou consigne son indisponibilité | |
| `proto/` (modifié) | `ReadyMessage` gagne `mic: boolean` | |
| `client/src/mic.ts` (créé) | Bascule : `getUserMedia`, `replaceTrack`, états | |
| `client/src/webrtc.ts` (modifié) | Transceiver `sendonly`, sender exposé dans `SessionHandle` | |
| `client/src/stats.ts` (modifié) | Métriques montantes dans l'incrustation de mesure | |

`mic.rs` ne référence jamais `windows` : il se compile et se teste sous Linux,
comme `audio.rs` et `source.rs`.

Le rendu diffère de la capture de A sur un point : **`AUDCLNT_STREAMFLAGS_EVENTCALLBACK`
fonctionne ici**, car il s'agit d'un flux de rendu ordinaire et non d'un
loopback. Le fil est réveillé par WASAPI, non par un sondage.

## 7. Flux de données

```
[navigateur]  getUserMedia({ audio: { echoCancellation, noiseSuppression,
                                      autoGainControl } })
      → sender.replaceTrack(track)          (aucune renégociation)
      → Opus ; cadence et durée de trame décidées par Chrome
[agent, boucle transport]  Event::MediaData sur mic_mid
      → dépôt NON bloquant dans le JitterBuffer, et rien d'autre
[agent, fil de rendu]  réveil par événement WASAPI
      → trame due selon l'horloge de rendu
      → décodage Opus (FEC si la suivante est là, sinon PLC, sinon silence)
      → conversion vers le format du câble : i16 → flottant 32 bits,
        mono → stéréo par duplication
      → IAudioRenderClient : « CABLE Input »
[Windows]  les applications voient « CABLE Output » comme microphone
```

L'invariant de A est préservé **en miroir** : la boucle de transport dépose, le
fil dédié travaille. Décoder de l'Opus dans `act_on_timeout` réintroduirait
exactement le défaut que A a chassé, et que le remplacement de `set_read_timeout`
par un sondage en petites tranches avait déjà coûté à corriger.

La conversion est celle de A prise à l'envers : le décodeur libopus rend des
entiers signés 16 bits, là où le format de mixage partagé attend typiquement du
flottant 32 bits. Ce n'est pas un rééchantillonnage — la fréquence, elle, n'est
jamais convertie (§4).

**Aucune durée de trame n'est supposée.** Chrome émet du 20 ms par défaut, là où
A en produit du 10 ms, et rien ne garantit qu'il s'y tienne. Le nombre
d'échantillons est lu de ce que rend le décodeur, jamais présumé.

## 8. Le tampon de gigue

C'est la pièce qui n'est pas le symétrique de A, et le point de conception le
moins évident du chantier.

En émission, A jette le vieux et garde le frais : la ligne de temps est la
sienne. En réception, on **subit** une ligne de temps distante qu'il faut
restituer à cadence dure sur une carte son. Quatre problèmes distincts :

### Gigue

Tampon cible **40 ms**, plafond **200 ms**, réordonnancement par horodatage RTP.
Quarante millisecondes est le réglage courant pour de la voix sur un réseau
correct ; le rendre configurable avant d'avoir mesuré serait spéculatif.

### Perte

Le FEC in-band d'Opus reconstruit la trame perdue à partir de la **suivante** ;
avec 40 ms d'avance, celle-ci est généralement déjà présente. À défaut, PLC.

### Silence

Chrome cesse d'émettre quand l'utilisateur se tait (DTX). Le câble doit
néanmoins être alimenté **en continu** :

> ⚠️ **LA PRÉMISSE EST PLUS ÉTROITE QUE CETTE PHRASE, ET C'EST MESURÉ (recette
> E2, tâche 12, critère ④, 20 août 2026). LA CONCLUSION, ELLE, TIENT SANS
> CHANGEMENT.** Chrome cesse d'émettre quand la **source de la piste** s'arrête
> — c'est le silence que E1 a mesuré, `packetsSent` figé. Mais un
> **périphérique de capture vivant qui produit du silence** fait émettre Chrome
> **50 paquets/s sans interruption** : il n'y a alors ni DTX, ni trame
> manquante, ni dissimulation. **Un utilisateur qui se tait devant un micro
> branché est probablement dans ce second cas, et non dans le premier** —
> ⚠️ *probablement, car la recette E2 a mesuré du silence NUMÉRIQUE, pas une
> pièce calme, et l'écart entre les deux n'est éprouvé par rien.*
>
> ✅ **« Le câble doit être alimenté en continu » reste juste, et pour les deux
> silences** : le fil de rendu écrit du silence par défaut, et le juge relève
> `AMPLITUDE = 0,000000` aussi bien micro allumé sur du silence que piste
> arrêtée.
 une application qui écoute un tampon
vide n'entend pas « du silence », elle voit un flux qui s'interrompt. Le fil de
rendu écrit donc du silence par défaut — le pendant exact du complément de
silence de A §5, et pour une raison de même nature : la continuité de la ligne
de temps.

### Dérive d'horloge

L'horloge du microphone de l'utilisateur et celle de la carte virtuelle diffèrent
de quelques ppm. En dix minutes, le tampon dérive de plusieurs dizaines de
millisecondes — vers la famine, ou vers une latence qui ne redescend jamais.

Correction par surveillance de l'occupation : au-dessus de **120 ms** on saute
une trame, en dessous de **20 ms** on en insère une. Grossier, audible une fois
par plusieurs minutes. L'alternative propre — un rééchantillonnage adaptatif —
est refusée pour la raison qui a fait refuser celui de A : du travail écrit avant
d'avoir constaté le besoin.

### Compteurs

Chaque saut, chaque insertion, chaque rejet incrémente un compteur journalisé
périodiquement. Aucun n'est silencieux.

## 9. Exclusivité et vie privée

### Exclusivité

Le puits micro est une ressource **de l'agent**, non de la connexion : il n'y a
qu'un câble. La première connexion qui présente un flux montant l'acquiert ; une
seconde est refusée, journalisée une fois, et sa piste ignorée.

Avec la connexion unique d'aujourd'hui, la garde tient en un drapeau atomique.
Elle est écrite maintenant parce qu'elle rend le comportement à deux fenêtres
**choisi** plutôt qu'accidentel le jour où le chantier D arrivera.

La libération est déclenchée par la **fermeture de la connexion propriétaire**
(`Event::Closed`, ou la disparition ICE déjà traitée par `transport.rs`), et non
par l'extinction du bouton : un utilisateur qui coupe puis rallume son micro doit
retrouver le sien, pas se le faire prendre entre-temps.

### Vie privée

Le micro est la seule fonction du produit qui capte l'utilisateur chez lui.

- La permission est demandée **au clic**, jamais à l'ouverture de session.
- L'extinction fait `replaceTrack(null)` **et** `track.stop()`. `enabled = false`
  seul laisserait le micro ouvert et l'indicateur de Chrome allumé : un mensonge
  visuel, inacceptable sur cette fonction précisément.
- L'agent n'écrit jamais l'audio sur disque, et aucune option ne le permet.
- Les journaux ne portent que des compteurs, jamais de contenu.

Le bouton reflète trois états : fermé, actif, refusé par le navigateur.

## 10. Erreurs et dégradation

Principe repris de A : **le micro ne tue jamais une session qui fonctionne.**

| Situation | Comportement |
| --- | --- |
| VB-Cable absent sur la VM | `ready` porte `mic: false`, le bouton n'apparaît pas, journal explicite côté agent |
| Format du câble ≠ 48 kHz | Idem, avec la fréquence rencontrée nommée |
| Permission refusée par l'utilisateur | État « refusé », message indiquant comment la rétablir, session intacte |
| Aucun périphérique d'entrée côté navigateur | Bouton désactivé, avec l'explication |
| Piste montante non négociée | Aucun paquet attendu, avertissement unique (calqué sur `warn_negotiation_once`) |
| Tampon en famine | Silence ou PLC, compteur agrégé journalisé |
| Tampon saturé | Trame la plus ancienne jetée, compteur |
| Second flux montant | Refusé, avertissement unique |
| Échec d'écriture WASAPI | Journalisé ; le fil tente une réinitialisation ; la session continue |

`ReadyMessage` gagne `mic: boolean` plutôt qu'un nouveau type de message : la
disponibilité du câble est connue à l'établissement et ne change pas en cours de
session. Le champ est optionnel à la lecture et **son absence vaut `false`** :
un client récent parlant à un agent ancien n'affiche pas un bouton qui ne mènerait
nulle part. `CONTROL_VERSION` reste donc à 1.

Un échec silencieux serait le pire cas : un bouton qui s'allume sans que rien
n'arrive côté VM est indiagnosticable. Chaque chemin de dégradation journalise
une fois, explicitement.

## 11. Tests

### Sous Linux, sans VM

- **`opus.rs`** : le décodage d'une trame manquante rend le bon nombre
  d'échantillons ; la reconstruction FEC depuis la trame suivante restitue un
  signal corrélé à l'original. Vérifier seulement qu'il rend des octets
  prouverait qu'il rend du bruit tout aussi bien.
- **`mic.rs`** : une arrivée désordonnée est restituée dans l'ordre.
- **`mic.rs`** : à saturation, c'est la trame la plus **ancienne** qui part ;
  en famine, le puits rend du silence sans jamais bloquer.
- **`mic.rs`** : les seuils de dérive déclenchent saut et insertion aux bons
  moments, chacun avec son compteur.
- **`mic.rs`** : des trames de 10, 20 et 40 ms passent toutes. C'est le test qui
  interdit de re-supposer une durée fixe (§7).
- **`transport.rs`** : le test de bouclage existant (`transport.rs:1320`) étendu
  au sens montant — un paquet Opus écrit par le pair est retrouvé dans le tampon
  côté agent, avec le PT attendu. **C'est aussi la sonde n°1 du §12.**
- **`transport.rs`** : un second flux montant est refusé.

### Côté client (Vitest)

- L'offre contient un `m=audio` en `sendonly` ;
- au départ, le sender n'a pas de piste ;
- la bascule appelle `getUserMedia` une seule fois, puis `replaceTrack` ;
- l'extinction appelle `replaceTrack(null)` **et** `track.stop()` ;
- un refus de permission mène à l'état « refusé » sans exception.

Par injection des dépendances (`getUserMedia`, sender), sans toucher aux objets
globaux — la technique retenue pour `reset-origin.js` dans le spike, qui a permis
de tester le même module sous Vitest et dans le navigateur sans build.

### Sur la VM — recette

Automatiser ceci honnêtement n'est pas possible : il faut entendre.

1. L'enregistreur vocal Windows, réglé sur CABLE Output, restitue la voix.
2. Un appel réel se tient : le correspondant entend, sans écho.
3. La latence bouche → câble est mesurée.
4. Dix minutes de conversation continue ne montrent ni dérive audible ni latence
   croissante ; les compteurs de saut et d'insertion sont relevés.
5. Un silence de 60 s puis une reprise ne provoquent ni coupure ni à-coup.

## 12. Risques et sondes

Le chantier ouvre par ses sondes. Rien n'est construit avant.

| # | Question | Comment on la tranche |
| --- | --- | --- |
| 1 | str0m 0.21 dépaquetise-t-il l'Opus entrant et l'expose-t-il via `Event::MediaData` ? | Test de bouclage, **sans VM**. Le sens montant n'a jamais été exercé dans ce dépôt : `transport.rs` ne traite `MediaData` que dans un test. **Bloquant** — si la réponse est non, le chantier change de forme : dépaquetisation à écrire, ou repli sur un canal de données |
| 2 | VB-Cable s'installe-t-il en silencieux ? Quel format expose-t-il ? Apparaît-il bien comme microphone pour les applications ? | Installation sur la VM, `Get-PnpDevice`, puis enregistreur vocal |
| 3 | Peut-on faire de CABLE Output le périphérique d'entrée **par défaut** ? | Aucune API publique. Réglage dans l'image de base, ou `IPolicyConfig` (non documentée). Sans cela, chaque application doit être réglée à la main — dégradation d'usage, pas panne |
| 4 | L'annulation d'écho de Chrome couvre-t-elle le son de la VM revenant par les haut-parleurs ? | À l'oreille, en appel réel. Le son étant joué par l'onglet lui-même, elle devrait le couvrir — à confirmer, pas à supposer. En multi-fenêtres (chantier D), le son pourrait être joué par une **autre** fenêtre que celle qui capte : à réexaminer à ce moment-là |

La sonde 1 passe en premier et ne demande pas la VM : c'est le seul risque
capable de remettre en cause l'architecture. Les sondes 2 à 4 exigent la VM
démarrée (`virsh start Windows`, voir `CLAUDE.md`).

## 13. Critère de fin

- Une application Windows lancée dans la VM **entend** l'utilisateur parler dans
  son navigateur, à travers Pomerium.
- Un correspondant en appel réel ne perçoit pas d'écho.
- La latence **ajoutée par l'agent** — réception du paquet jusqu'à l'écriture sur
  le câble — reste sous 100 ms, tampon de gigue compris. Le trajet réseau, lui,
  n'est pas de notre ressort et ne figure donc pas dans le critère : le mesurer
  est utile, s'y engager ne le serait pas.
- Dix minutes de conversation continue ne dérivent pas.
- Le micro est fermé tant qu'il n'a pas été allumé, et l'est **réellement** quand
  on l'éteint : indicateur de Chrome éteint, piste arrêtée.
- La suite de tests est verte sous Linux, sans VM.

## 14. Hors périmètre

- **La renégociation SDP.** Contournée par `replaceTrack` (§5). Le jour où une
  autre fonction l'exigera, elle sera construite pour elle.
- **La webcam.** Même chaîne conceptuelle, mais une caméra virtuelle est un autre
  pilote, un autre format et un autre chantier.
- **Le choix du périphérique d'entrée dans notre interface.** Chrome le propose
  déjà, à côté de la demande de permission.
- **Le mixage de plusieurs sources montantes.** L'exclusivité (§9) le rend sans
  objet.
- **L'annulation d'écho côté agent.** On s'appuie sur celle du navigateur.
- **Le débit audio adaptatif.** Relève du chantier C, qui traitera l'adaptation
  réseau pour tous les médias à la fois.
