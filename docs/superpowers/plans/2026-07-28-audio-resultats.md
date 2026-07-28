# Chantier A — Audio : recette et résultats

**Date d'exécution** : 28 juillet 2026
**Navigateur (harnais de recette, CDP sans interface)** : Google Chrome
150.0.7871.181 (build officiel, 64 bits), `HeadlessChrome/150.0.0.0`, relevé
par `google-chrome --version` et `navigator.userAgent` — pas supposé.
**Plateforme du navigateur** : Debian GNU/Linux 13 (trixie), noyau
6.12.96+deb13-amd64, x86_64. C'est la machine de développement (hôte du
dépôt), **pas** un poste client réel ni un Chromebook — à la différence du
spike multi-fenêtres (`2026-07-28-spike-multifenetres-resultats.md`), cette
recette n'a pas été menée depuis la plateforme cliente privilégiée du
cadrage. Voir « Réserves ».
**Plateforme agent** : VM Windows (build 20348), GPU NVIDIA RTX 4070,
adaptateur d'affichage virtuel « SudoMaker Virtual Display Adapter ».
**Périphérique de rendu audio et format de mixage** (sonde de la tâche 5,
reconfirmés à chaque session de cette tâche) : deux endpoints de rendu
actifs (`Get-CimInstance Win32_SoundDevice` / `Get-PnpDevice -Class
AudioEndpoint`, état `OK` sur les deux) — « Haut-parleurs (Steam Streaming
Speakers) », **virtuel**, et « HDP-V104 (NVIDIA High Definition Audio) ».
Format de mixage WASAPI réellement relevé, systématiquement identique sur
toutes les sessions : **48 000 Hz, 2 canaux, 32 bits flottant**.

---

## Verdict

**Le son passe.** La chaîne WASAPI loopback → assemblage de trames →
encodage Opus → tampon circulaire → transport WebRTC → décodage navigateur
est prouvée par des compteurs `getStats()` qui **augmentent** pendant la
lecture d'un son connu et **ne perdent aucun paquet**, avec un silence
prolongé (63 s mesurées en continu) qui ne produit ni coupure, ni dérive, ni
resynchronisation brutale.

**Ce que cette mesure ne prouve pas** : que le signal reçu est
perceptuellement le bon son (intelligible, sans bruit) plutôt qu'un flux de
la bonne taille par coïncidence. C'est la limite explicite de toute preuve
fondée sur `getStats()` — voir « À confirmer par écoute humaine ».

---

## 1. Tableau des mesures

| # | Mesure | Valeur obtenue | Attendu (brief) | Verdict |
|---|---|---|---|---|
| 1 | Son audible et intelligible | **non mesuré par cet agent** — voir section dédiée | oui | ⚠️ À CONFIRMER PAR ÉCOUTE HUMAINE |
| 2 | Débit audio pendant lecture (Windows Ding en boucle, fenêtre de 20 s) | Δ bytes = 278 365 → **111,3 kb/s** ; Δ packets = 2002 → 100,1 paquets/s | ~130 kb/s | Atteint (proche, cohérent) |
| 3 | Débit audio en silence (DTX, fenêtre continue de 63,2 s) | Δ bytes = 6623 → **0,84 kb/s** ; Δ packets = 6315 → 99,9 paquets/s | quelques kb/s | Atteint (mieux que l'estimation) |
| 4 | Pertes de paquets | **0** sur tous les essais (silence 63 s + relance sonore + essais avec son) | nulles/marginales sur LAN | Atteint |
| 5 | Gigue (`jitter` `getStats()`) | 1–3 ms sur tous les essais | faible attendu sur LAN | Atteint |
| 6 | Décalage A/V (`estimatedPlayoutTimestamp` audio − vidéo) | **non mesurable** : le champ est absent de `getStats()` sur ce Chrome/cette plateforme (vérifié : absent de TOUTE entrée `inbound-rtp`, y compris vidéo) | < 45 ms avance, < 125 ms retard (ITU-R BT.1359) | NON MESURÉ — limite du navigateur testé, pas un échec de la correction tâche 7 |
| 7 | Comportement après 60 s de silence | Continu sur 63,2 s : 0 perte, cadence paquets stable à 99,9–100,2/s (10 ms par trame, sans à-coup), transition vers le son suivante sans coupure ni resynchronisation (bytes/packets progressent sans discontinuité au moment du son) | pas de coupure/dérive/resynchro | Atteint |

Le débit en jeu (mesure #2) a nécessité une deuxième tentative : la première
(boucle de 15 sons, ~15 s) s'était **déjà tue** avant le premier relevé du
harnais, qui attend jusqu'à 20 s (vidéo qui ne progressait pas dans cet essai)
+ 1,5 s avant de prendre sa première mesure. Voir « Ce que la mesure a coûté »
pour le détail — sans cette relecture, le rapport aurait affiché à tort un
débit « en jeu » quasi identique au silence.

## 2. Preuve automatique (harnais `client/verify-webrtc.mjs` étendu)

Trois relevés bruts, représentatifs des essais menés (fenêtre de 20 s, son
en boucle sur la VM) :

```
--- Relevé 1 ---
  [audio] bytesReceived=300121 packetsReceived=2153 packetsLost=0 jitter=2.0ms estimatedPlayoutTimestamp=absent
--- Relevé 2 (+20000ms) ---
  [audio] bytesReceived=578486 packetsReceived=4155 packetsLost=0 jitter=3.0ms estimatedPlayoutTimestamp=absent

Δ audio bytesReceived sur 20000ms : 278365
Δ audio packetsReceived sur 20000ms : 2002
audio : bytesReceived et packetsReceived augmentent — PREUVE que l'audio traverse la chaîne.
décalage A/V : non mesurable (pas de piste audio active sur ce relevé).
```

Trace de continuité sur 63 s de silence pur (échantillonnage toutes les 9 s,
même session, sans interruption ni redémarrage d'agent) :

```
t=0.0s  {"bytesReceived":95,   "packetsReceived":89,   "packetsLost":0,"jitter":0.002}
t=9.0s  {"bytesReceived":1041, "packetsReceived":991,  "packetsLost":0,"jitter":0.002}
t=18.0s {"bytesReceived":1987, "packetsReceived":1893, "packetsLost":0,"jitter":0.001}
t=27.1s {"bytesReceived":2933, "packetsReceived":2795, "packetsLost":0,"jitter":0.001}
t=36.1s {"bytesReceived":3880, "packetsReceived":3698, "packetsLost":0,"jitter":0.002}
t=45.1s {"bytesReceived":4828, "packetsReceived":4602, "packetsLost":0,"jitter":0.002}
t=54.2s {"bytesReceived":5774, "packetsReceived":5504, "packetsLost":0,"jitter":0.002}
t=63.2s {"bytesReceived":6718, "packetsReceived":6404, "packetsLost":0,"jitter":0.001}
>>> déclenchement du son sur la VM
t=final (+son) : {"bytesReceived":22749,"packetsReceived":7509,"packetsLost":0,"jitter":0.001}
```

L'incrément entre chaque paire de relevés (~946 octets et ~902 paquets par
tranche de 9 s) est remarquablement stable — c'est la signature attendue du
complément de silence de `frames.rs` : aucune trame n'est jamais sautée,
DTX réduit seulement la taille de chaque trame, pas leur cadence. La
transition au son (dernier relevé) montre une augmentation nette de la taille
moyenne par paquet (~14,5 octets/paquet contre ~1,05 en silence) sans aucune
perte ni discontinuité de compteur.

### Garde-fous du harnais, vérifiés

- **Le harnais échoue quand l'audio a été vu puis s'arrête de progresser** :
  logique implémentée dans `client/verify-webrtc.mjs` (`audioGrowing` exigé
  quand `!audioAbsent`), non reproduite par un essai réel dans cette
  recette (aurait exigé une panne injectée, jugé disproportionné) mais
  vérifiée par relecture du code.
- **Le harnais reste vert sur une session vidéo seule** : vérifié en
  conditions réelles avec `TEST_FILE=C:\dev\agent\testdata\testsrc.264`
  (voir §4 « Dégradation ») — code de sortie **0**, `audio : aucun octet
  reçu sur les deux relevés ... session vidéo seule` imprimé, aucune ligne
  d'échec liée à l'audio.

## 3. À confirmer par écoute humaine

**Je n'ai pas pu entendre le son.** La preuve automatique établit que des
paquets Opus arrivent, sont décodés en continu, sans perte, et que leur
volume varie de façon cohérente avec la présence ou l'absence de son côté
VM — mais elle ne prouve pas que le signal porte le bon contenu plutôt que
du bruit ou un silence numérique de la bonne taille par coïncidence.

**Reste à faire, avec un humain** :

1. Ouvrir le client (`http://localhost:5174/?session=demo` avec le
   signaling et l'agent actifs — voir « Reproduction » ci-dessous) et
   cliquer une fois dans la page pour lever la politique d'autoplay.
2. Jouer un son connu sur la VM : `(New-Object Media.SoundPlayer
   'C:\Windows\Media\Windows Ding.wav').PlaySync()`.
3. Confirmer que le son est audible, sans coupure ni artefact grossier
   (métallique, haché), et reconnaissable comme un « ding » plutôt que du
   bruit.
4. Si possible, confirmer subjectivement que le son perçu est synchrone
   avec une action visible à l'écran (utile puisque la mesure automatique du
   décalage A/V — §1, mesure #6 — n'a pas pu être obtenue sur ce navigateur).

## 4. Dégradation : session sans audio

`TEST_FILE=C:\dev\agent\testdata\testsrc.264 scripts/run-agent.sh`, puis
harnais sur la même session :

```
--- Relevé 1 ---
  framesDecoded=2146 framesReceived=2147 frameWidth=1280 frameHeight=720 ...
  aucune entrée inbound-rtp audio dans getStats()
--- Relevé 2 (+6000ms) ---
  framesDecoded=2748 framesReceived=2749 frameWidth=1280 frameHeight=720 ...
  aucune entrée inbound-rtp audio dans getStats()

Δ framesDecoded sur 6000ms : 602
audio : aucun octet reçu sur les deux relevés (pas de piste audio active — session vidéo seule, ou silence total).
PREUVE : la vidéo traverse la chaîne (framesDecoded et framesReceived augmentent, dimensions plausibles).
```

Code de sortie du harnais : **0**. Vidéo normale (602 images décodées en
6 s), aucune piste audio (`windows_audio::WindowsAudioSource` n'est jamais
construite quand `config.test_file.is_some()`, cf. `agent/src/main.rs`),
aucune erreur fatale dans `agent.log`. Conforme à l'attendu de l'étape 8 du
brief.

## 5. Sonde process loopback (spec §11, sonde n°4)

**Résultat : non déterminé.**

Ce qui a été fait :

- La sonde a été implémentée dans `agent/src/wasapi.rs`
  (`probe_process_loopback`) et câblée dans `agent/src/main.rs`
  (`PROCESS_LOOPBACK_PROBE`), conformément au brief : activation via
  `ActivateAudioInterfaceAsync` + `AUDIOCLIENT_ACTIVATION_PARAMS`
  (`AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`,
  `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`), avec un gestionnaire
  de complétion COM (`IActivateAudioInterfaceCompletionHandler`) implémenté
  via la macro `#[windows::core::implement]`, synchronisé par un couple
  `Mutex`/`Condvar` borné à 10 s.
- **Le code compile et lie sur la cible Windows réelle**
  (`scripts/build-agent.sh`, profil `release`), après trois corrections
  d'API découvertes par la compilation elle-même (non trouvables sans
  compiler sur la cible) :
  1. `windows-core` doit être une dépendance **directe** du crate (pas
     seulement transitive via `windows`) — la macro `#[implement]` génère
     du code qui référence `windows_core::...` littéralement.
  2. L'auto-déréférencement de `ManuallyDrop` n'est pas appliqué
     automatiquement sur un champ d'union COM (`PROPVARIANT.Anonymous`) :
     il faut un `*` explicite (`(*propriete.Anonymous.Anonymous).vt = ...`).
- **L'exécution sur la VM n'a pas pu être confirmée.** Plusieurs tentatives
  (via `scripts/run-agent.sh` avec `PROCESS_LOOPBACK_PROBE=<PID>`, PID d'un
  processus Firefox réel de la session interactive) ont produit, de façon
  reproductible :
  - la tâche planifiée rapporte un code de sortie **0** (succès) et une
    exécution rapide (le processus `agent.exe` n'apparaît déjà plus lors
    d'un sondage à 2 s d'intervalle) ;
  - mais **aucune ligne n'atteint `agent.log`** — ni la trace de succès
    (`sonde process loopback`), ni celle d'échec
    (`sonde process loopback échouée`) ;
  - une invocation synchrone équivalente, directement via WinRM (hors tâche
    planifiée), **reste bloquée sans jamais rendre la main** ;
  - `Get-WinEvent` sur le journal `Application` ne montre **aucun rapport de
    plantage** (WER) corrélé avec ces tentatives — à la différence de deux
    plantages historiques d'`agent.exe` retrouvés dans ce même journal
    (`0xc0000005` dans `ntdll.dll`), mais à des horodatages qui ne
    correspondent à aucune de mes tentatives (probablement une session
    antérieure, sans rapport avec cette sonde).
- Hypothèse non tranchée : le pipeline `Tee-Object -FilePath` +
  redirection `*>&1` à travers la tâche planifiée peut ne pas se
  fermer/vider correctement si un fil du pool COM de rappel
  (`ActivateAudioInterfaceAsync`) reste actif après le retour de `main()`,
  gardant un descripteur ouvert sans que le processus ne se termine
  proprement au sens où `Tee-Object` l'attend — non vérifiée faute d'outil
  de diagnostic Windows disponible (débogueur attaché, Process Monitor).

**Conformément à l'instruction du brief** (« si cette machinerie dépasse une
heure de travail, arrête-toi et signale-le [...] un résultat "non déterminé,
à reprendre au chantier D" est un résultat acceptable, et de loin préférable
à une demi-journée passée sur du code jetable »), l'investigation s'arrête
ici. Le code reste dans l'arbre (jetable, comme prévu) : il constitue un
point de départ vérifié-compilable pour le chantier D, qui devra reprendre
le diagnostic runtime avec de meilleurs outils (sortie vers un fichier dédié
plutôt que `Tee-Object`/stdout à travers une tâche planifiée cachée, ou un
débogueur attaché).

**Implication pour le chantier D** : le risque « la VM est-elle éligible au
process loopback » (build 20348, condition nécessaire) **reste ouvert**. Rien
ne l'infirme (pas de plantage, pas de refus d'activation observé), mais rien
ne le confirme non plus.

## 6. Réserves — ce qui n'a pas été vérifié

- **Rééchantillonnage** : refusé par conception
  (`agent/src/wasapi.rs::LoopbackCapture::open`, `bail!` si la fréquence de
  mixage n'est pas 48 000 Hz). Jamais mis en défaut dans cette recette : le
  format de mixage a été 48 000 Hz sur toutes les sessions observées, donc
  ce chemin de refus n'a jamais été exercé.
- **Microphone** : hors périmètre du chantier A (capture loopback de rendu
  uniquement, jamais de capture d'entrée). Non testé, non applicable.
- **Réseau non local** : toutes les mesures ont été prises sur le réseau
  virtuel LAN de la VM (gigue 1–3 ms, 0 perte). Le comportement sur un
  réseau à latence/perte réelles (FEC in-band, DTX sous perte, tampon de
  gigue du navigateur) n'a pas été testé — hors périmètre de cette tâche,
  qui répond à la sonde n°4 et produit le livrable, pas au chantier C
  (adaptation réseau).
- **Décalage A/V non mesuré** (voir §1, mesure #6) : `estimatedPlayoutTimestamp`
  est absent de `getStats()` sur Chrome 150.0.7871.181 / Debian 13 Linux
  headless — vérifié explicitement (absent de **toute** entrée
  `inbound-rtp`, vidéo comprise, pas seulement audio). Le code du harnais
  reste écrit conformément au brief (il fonctionnera dès que/si ce champ
  devient disponible) et gère l'absence proprement (affiche « non
  mesurable », n'échoue jamais dessus). La correction de la tâche 7
  (`wallclock` = instant de capture) n'a donc **pas** pu être vérifiée
  objectivement par cette méthode précise ; elle reste vérifiée
  indirectement par la continuité et la régularité des compteurs (§1,
  mesures #3 et #7) et par la relecture du code de la tâche 7.
- **Plateforme cliente non privilégiée** : contrairement au spike
  multi-fenêtres, cette recette tourne sur l'hôte de développement
  (Debian Linux), pas sur un Chromebook (plateforme cliente privilégiée du
  cadrage, §6.1). Rien ne suggère un écart de comportement audio entre les
  deux (Opus/WebRTC est standard), mais ce n'est pas vérifié ici.
- **Vidéo peu représentative dans les essais purement audio** : avec
  Firefox statique (aucune page en défilement), Desktop Duplication ne
  produit quasiment aucune image neuve (comportement documenté dans
  `agent/src/main.rs`, pas un défaut introduit par ce chantier) — les
  essais audio ci-dessus montrent donc souvent `framesDecoded` figé à 2.
  Contourné pour un essai en ouvrant une page à contenu défilant
  (`client/recette/scroll-test.html`) ; confirmé sain indépendamment par
  l'essai `TEST_FILE` (§4, 602 images décodées en 6 s). N'affecte aucune
  des mesures audio, qui ne dépendent pas de la vidéo.
- **VM instable pendant la recette** : la VM s'est arrêtée spontanément une
  fois en cours de mesure (comportement déjà documenté à plusieurs reprises
  dans ce projet, cause non identifiée). `virsh start Windows` a suffi à la
  relancer (session interactive retrouvée en quelques secondes). Aucune
  mesure conservée dans ce rapport n'a été prise à cheval sur cet incident.

## 7. Ce que la mesure a coûté, et ce qu'elle a évité

- **Faux négatif de timing évité de justesse** : le premier essai de mesure
  du débit « en jeu » a fait jouer une boucle de 15 sons Windows Ding
  (~15 s) *avant* de lancer le harnais, sans tenir compte du fait que le
  harnais attend jusqu'à 20 s que la vidéo « coule » (jamais le cas ici,
  Firefox statique) plus 1,5 s avant son premier relevé — la boucle sonore
  s'était donc déjà tue avant la moindre mesure, produisant un débit « en
  jeu » quasi identique au silence (526 octets/5 s). Si ce chiffre avait été
  retenu tel quel, le rapport aurait conclu à tort que DTX ne distingue pas
  le silence du son. Corrigé en allongeant la boucle sonore à 50 itérations
  (~50 s, recouvrant largement la fenêtre de mesure réelle) : le débit
  mesuré est alors passé de ~0,8 kb/s à ~111 kb/s, cohérent avec l'attendu.
- **Collision de tâche planifiée découverte et corrigée** : `schtasks
  /create /f ... ; schtasks /run` n'interrompt pas une instance déjà en
  cours (politique par défaut « ne pas démarrer de nouvelle instance ») —
  un agent laissé actif par un essai précédent (typiquement parce que
  Chrome venait d'être fermé côté harnais mais que l'agent n'avait pas
  encore détecté la déconnexion ICE) absorbait silencieusement les demandes
  `/run` suivantes, produisant des relevés `agent.log` obsolètes qui
  semblaient provenir d'une nouvelle session. Corrigé en insérant
  systématiquement `scripts/stop-agent.sh` avant chaque redémarrage
  d'agent — cette précaution n'était pas nécessaire dans la recette du
  jalon 1 (une seule session longue), elle le devient dès qu'un enchaînement
  rapide de sessions courtes est nécessaire, comme dans cette tâche.
- **Machinerie COM de la sonde process loopback** : ~1 heure de travail
  (recherche d'API dans les sources vendorées de `windows`/`windows-core`,
  trois itérations de correction de compilation, plusieurs tentatives
  d'exécution infructueuses) pour un résultat non tranché. Conforme à
  l'attente du brief : la valeur de cette sonde est informative, pas
  structurante, et le seuil d'arrêt a été respecté plutôt que dépassé.
