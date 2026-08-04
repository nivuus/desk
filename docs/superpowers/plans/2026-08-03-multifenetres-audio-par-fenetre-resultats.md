# Sous-bloc D7 — l'audio par fenêtre : résultats

Plan : `docs/superpowers/plans/2026-08-03-multifenetres-audio-par-fenetre.md`
Conception : `docs/superpowers/specs/2026-08-03-multifenetres-audio-par-fenetre-design.md`
Journaux : `docs/superpowers/plans/journaux-multifenetres-d7/` — **UTF-8, CRLF,
séquences ANSI de `tracing` PRÉSENTES** : `sed 's/\x1b\[[0-9;]*m//g'` avant tout
`grep`.

---

## 1. La mesure qui gouverne — le *process loopback* rend des octets, et seulement les siens

**Verdict : la voie est REÇUE.** L'isolation par processus est établie ; le
format qu'Opus attend est accepté tel quel ; le cycle `Stop()`/`Start()` tient
et le flux redélivre après lui. Les trois relevés que la conception §3 exigeait
sont pris, et un quatrième s'y est ajouté en cours de route.

**Binaire mesuré** : bâti depuis `55ddc38`, **9 175 552 octets** (contre
9 146 880 avant — la compilation a duré **12,13 s**, pas les 0,13 s qui auraient
signalé un binaire non rebâti).

⚠️ **UNE exécution par point, quatre au total. Aucun taux n'est revendiqué.**

### 1.1 Le montage

Deux processus lancés en session interactive par tâche planifiée `/it` — la
session 1, pas la session 0 de WinRM :

- **A**, `powershell.exe` PID **25764**, jouant `C:\Windows\Media\Alarm01.wav`
  en boucle par `System.Media.SoundPlayer.PlayLooping()` ;
- **B**, `powershell.exe` PID **37988**, ne faisant que dormir.

**Le même exécutable pour les deux, à dessein.** Si le *process loopback*
isolait par nom d'image plutôt que par PID, ou s'il retombait sur le mix global,
B rendrait une crête comparable à celle de A. C'est ce que le relevé ② mesure.

**`SoundPlayer` et non `[Console]::Beep`** : ce dernier passe par
`kernel32!Beep` et ne traverse pas le périphérique de rendu de cette VM — il a
produit un faux négatif documenté au chantier A (28 juillet 2026).

### 1.2 Les quatre relevés

| # | Ce qui est sondé | `crete` | `echantillons` | Journal |
| --- | --- | --- | --- | --- |
| ⓪ | **témoin** : loopback GLOBAL de session, pendant que A joue | **11677** | 1 152 000 / 12 s | `capture-releve0-temoin-global.log` |
| ① | *process loopback* sur **A, qui joue** | **11679** | 1 439 040 / 15 s | `capture-releve1-cible-qui-joue.log` |
| ② | *process loopback* sur **B, muet**, pendant que A joue | **1** | 1 439 040 / 15 s | `capture-releve2-voisin-muet.log` |
| ②bis | *process loopback* sur **B, muet**, **A arrêté** | **1** | 1 439 040 / 15 s | `capture-releve2bis-voisin-muet-sans-A.log` |

**Le relevé ⓪ n'était pas au plan, et il fallait le prendre.** Sans lui, un zéro
au relevé ① serait resté ambigu entre « le *process loopback* ne capte rien » et
« rien ne jouait ». Il coûte douze secondes et il ferme la question : le son
atteint bien le périphérique de rendu par défaut.

**Le relevé ②bis n'était pas au plan non plus, et c'est lui qui tranche.**
Le critère écrit d'avance disait « crête **nulle** » ; le relevé rend **1**, et
il ne faut pas l'arrondir à zéro. ②bis répond à la seule question que ce 1
laissait ouverte — vient-il de A ? — en resondant B **une fois A éteint** : la
crête vaut **encore 1**. **Ce 1 est donc un plancher du chemin lui-même, et non
une fuite du voisin.** L'audio de A ne contribue rien de mesurable au flux de B.

**Séparation, CALCULÉE** et non relevée : `20·log₁₀(11679/1)` = **81,3 dB**. Une
capture du mix global aurait rendu à B une crête de l'ordre de celle de A.

### 1.3 Le format est accepté tel quel — `GetMixFormat` n'a pas été nécessaire

```
process loopback ouvert pid=25764 format=process loopback pid=25764 — 48000 Hz, 2 canaux, 16 bits entiers
```

`IAudioClient::Initialize` accepte le `WAVEFORMATEX` **imposé** — 48 kHz,
2 canaux, 16 bits entiers, exactement ce qu'`agent/src/opus.rs` attend. **Aucune
conversion n'est nécessaire** sur ce chemin, là où `LoopbackCapture::open`
convertit depuis le flottant 32 bits du mix de session (relevé ⓪ :
`48000 Hz, 2 canaux, 32 bits, flottant`).

**Et le sondage suffit** : `AUDCLNT_STREAMFLAGS_EVENTCALLBACK` n'a pas été posé,
`Initialize` n'a pas refusé, et la doctrine de sondage de `wasapi.rs` tient sur
cette API comme sur l'autre. Zéro `ERROR`, zéro `WARN` aux quatre exécutions.

### 1.4 Le cycle `Stop()` / `Start()` tient, et le flux redélivre

C'est ce dont dépend l'approche retenue au §4.4 de la conception — activer une
fois, ne plus jamais réactiver.

```
cycle Stop puis Start accepte pid=25764
sonde de capture process loopback terminee pid=25764
  echantillons_apres=479040 lectures_vides_apres=337 crete_apres=11662 silencieux_apres=false
```

**`crete_apres` seule n'aurait pas suffi**, et c'est exactement ce qu'une revue
de la tâche 2 avait relevé avant la mesure : `"cycle Stop puis Start accepte"`
ne prouve que le `HRESULT` de `Start`, jamais que l'audio a repris. Les
**479 040 échantillons** comptés après le cycle sont la preuve qui manquait. La
correction a coûté deux lignes et elle a économisé un aller-retour de VM.

### 1.5 Une conséquence produit, à consigner plutôt qu'à découvrir

**Le plancher de 1 LSB affaiblit une hypothèse de la conception.** Le §« DTX »
du cadrage affirmait qu'« une fenêtre dont l'application ne joue jamais rien ne
coûtera quasiment rien », en s'appuyant sur le DTX d'Opus
(`agent/src/opus.rs:76`), qui fait retomber une trame de **silence numérique** à
quelques octets. Un flux à ±1 LSB **n'est pas du silence numérique**, et rien
n'établit que le DTX s'y engage.

⚠️ **Ceci n'a pas été mesuré** : aucune trame issue de ce chemin n'a été soumise
à l'encodeur Opus. C'est une **inférence** tirée du fonctionnement connu du DTX,
pas un relevé.

**La conception l'esquive largement, sans l'annuler** : une fenêtre qui ne porte
pas le son est `arreter()`ée et ne capte donc rien du tout. La question ne se
pose que pour la fenêtre qui **porte** le son pendant que son application est
silencieuse. À une seule fenêtre porteuse par groupe de PID, le coût plafonne à
un flux Opus au débit nominal au lieu de quelques octets par trame.

❌ **Cette phrase renvoyait la vérification « à la recette de la tâche 13 ». La
recette a eu lieu et NE L'A PAS TRANCHÉE** : ses sources jouaient toutes un son
continu, et le cas d'une fenêtre **porteuse dont l'application se tait** n'a
jamais été monté. La question reste entière, et le §2.2 apporte au contraire un
indice qu'elle mérite d'être posée — `bytesReceived` continue de croître sur une
fenêtre dont le spectre est à −1000 dB.

### 1.6 Ce que ce §1 n'établit PAS

- **Aucun taux** : une exécution par point, quatre au total.
- **Le plafond d'activations concurrentes n'est pas mesuré.** Une seule
  activation a existé à la fois. La recette à N fenêtres l'exercera **sans le
  mesurer** — c'est la forme exacte de l'inférence que le sous-bloc D3 a dû
  payer sur DXGI.
- **Rien d'autres formats ni d'autres périphériques** : un seul format demandé,
  accepté du premier coup, sur un seul point de terminaison de rendu.
- **Rien des applications UWP**, dont le rendu audio peut ne pas vivre dans
  l'arbre du processus propriétaire de la fenêtre — `INCLUDE_TARGET_PROCESS_TREE`
  ne les couvrirait pas.
- **Rien de la durée** : la plus longue sonde a duré 20 s.
- **Le plancher de 1 LSB n'est pas expliqué**, et rien ne dit s'il est stable :
  il vaut 1 aux deux exécutions qui l'ont vu, et c'est tout ce qui est su.
- **Deux processus, un seul jouant.** Le cas de **deux** processus jouant
  simultanément, chacun sondé, n'a pas été exercé — c'est pourtant le cas nominal
  du produit.
- **Aucun octet capté n'a été décodé ni écouté** : seule la crête d'amplitude
  est relevée.

---

## 2. La recette — quatre critères sur cinq

**Binaire mesuré** : bâti depuis `826a5ca`, **9 216 512 octets** (compilation de 19,64 s, pas les 0,13 s
qui signaleraient un binaire non rebâti).

⚠️ **Le nombre d'exécutions figure dans chaque énoncé. Aucun taux n'est revendiqué.**

| # | Critère | Verdict | Exécutions |
| --- | --- | --- | --- |
| ① | **Isolation** — deux applications, deux tonalités | **TENU** | **2** |
| ② | **Arbitrage par PID** — deux fenêtres d'un même processus | **TENU** | **1** (2 phases de focus) |
| ③ | **L'audio survit au sommeil** | **NON EXERCÉ** | 0 |
| ④ | **Le budget suit l'arbitrage** | **MESURÉ** | 1 |
| ⑤ | **Aucune régression mono-fenêtre** | **TENU** | 1, plus un témoin A/B |

### 2.1 Critère ① — l'isolation, mesurée à la fréquence

Deux fenêtres Chrome `--app`, deux profils distincts donc deux processus, jouant 440 et 880 Hz.
Relevé sur la piste audio **reçue par le navigateur**, par `AnalyserNode` :

| Exécution | Fenêtre | Sa fréquence | La fréquence du **voisin** | Séparation |
| --- | --- | --- | --- | --- |
| `critere1` | w-2 (440) | **−40 dB** | −125 dB | **85 dB** |
| `critere1` | w-4 (880) | **−41 dB** | −135 dB | **94 dB** |
| `critere1b` | w-2 (440) | **−40 dB** | −129 dB | **89 dB** |
| `critere1b` | w-4 (880) | **−41 dB** | −123 dB | **82 dB** |

Plancher de bruit **−158 dB** aux quatre relevés ; marge de signal 117-118 dB. RTP réel : 238 926 à
279 659 octets reçus par fenêtre. Journal de l'agent : deux *process loopback* sur des **PID
distincts** (49412/44460 puis 47448/34600).

**Ce que l'argmax seul n'aurait pas prouvé.** Une revue a établi avant la mesure qu'un simple « la
raie la plus forte est la bonne » laisse passer une **fuite partielle** — une fenêtre entendant sa
propre tonalité à plein niveau *plus* celle du voisin 10 dB en dessous aurait été déclarée isolée.
Le verdict exige donc **deux marges** : `db(propre) − db(chaque autre) ≥ 10` et
`db(propre) − plancher ≥ 20`. Les écarts relevés (82 à 94 dB) sont un ordre de grandeur au-dessus du
seuil.

### 2.2 Critère ② — l'arbitrage par PID, et le fait de conception qu'il a révélé

Deux fenêtres, **un seul `--user-data-dir`**, donc **un seul processus Chrome** : le cas que la règle
de `agent/src/capteur/audio.rs` existe pour trancher.

| Phase | Fenêtre portante | Ce qu'elle entend | Fenêtre muette |
| --- | --- | --- | --- |
| focus sur w-2 | **w-2** | 440 à **−40 dB** *et* 880 à **−41 dB** | w-4 : **−1000 dB** sur les deux, `piste_muted=true`, `stats_audio=null` |
| focus sur w-4 | **w-4** | idem, miroir | w-2 : **−1000 dB** sur les deux |

`flip_confirme=true`, `assignation_confirmee=true`, et `agent_accord=true` — le journal de l'agent
corrobore par ses `ordre audio applique session=… actif=…`.

⚠️ **La fenêtre porteuse entend LE MÉLANGE des deux tonalités, et c'est structurel.**
`PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` capte l'**arbre de processus**, jamais la fenêtre.
**Deux fenêtres d'une même application ne peuvent donc pas avoir un son séparé, par nature de l'API** —
le focus ne choisit que **laquelle reçoit le flux partagé**. Ce n'est pas une limite de
l'implémentation ; c'est ce que l'isolation par processus veut dire, et la conception ne le disait pas.

⚠️ **Fait annexe, qui justifie après coup un choix d'instrument** : en phase 2, `bytesReceived` de
w-2 **a continué de croître** (274 832 → 275 798) alors que son spectre était à −1000 dB.
**`bytesReceived` seul aurait conclu qu'elle entend encore quelque chose.** C'est exactement pourquoi
le critère se juge à la fréquence et jamais au compte d'octets.

### 2.3 Critères ④ et ⑤

**④ — le budget suit.** Sur `critere1b` : **2** `ordre audio applique`, **tous deux `actif=true`**,
**0** `actif=false` — deux PID distincts, donc les deux portent, et `audio_bps` suit chacun.
Sur `critere2`, l'alternance `true`/`false` accompagne chaque bascule de focus.

**⑤ — aucune régression mono-fenêtre**, avec témoin A/B :

- sans `FENETRE_HWND` : `audio activé format="48000 Hz, 2 canaux, 32 bits, flottant"` — le **mix de
  session**, pas un *process loopback* ;
- avec `AUDIO=0` : **0** `audio activé`, et `son désactivé sur cet agent par AUDIO=0` ;
- la variable retirée, même fenêtre, même commande : le son revient.

### 2.4 La consignation n°2 de D6, vérifiée

`fenetre{session=w-2}` apparaît sur **22** lignes du journal, y compris celles émises par les modules
que le fil de fenêtre appelle (`agent::capteur::sommeil::parts`). Les traces du capteur sont
désormais attribuables sans effort.

## 3. Trois défauts que seule la recette pouvait trouver

**Aucune revue de code ne pouvait voir ceux-là**, et deux d'entre eux visent la spécification de ce
sous-bloc.

1. **`scripts/run-agent.sh` ne transmettait pas `AUDIO`.** La tâche 9 a retiré le poseur du
   superviseur (`lanceur.rs`) et promu `AUDIO=0` en interrupteur global « qu'un agent lancé à la main
   doit pouvoir employer » — mais `run-agent.sh` **est** la façon dont on lance un agent à la main sur
   cette VM, et rien ne l'y avait ajouté. L'implémenteur **et** son relecteur avaient vérifié la
   propriété **en traçant le code** : le tracé était juste, la valeur ne pouvait simplement pas
   atteindre le processus. Corrigé (`a891062`) et vérifié de bout en bout ci-dessus.
2. **La recette d'entrée de D8, telle que la conception la prescrivait, rend 0 sur un journal brut.**
   Les séquences ANSI de `tracing` séparent le nom du champ de sa valeur
   (`[3mactif[0m[2m=[0mtrue`) : `grep 'actif=true'` ne trouve rien. Le `sed` de mise à plat est
   **obligatoire**, pas facultatif.
3. **Et ce même contrôle rend 0 sur toute session de moins de 30 s.** `compteurs audio` est périodique
   (`REPORT_INTERVAL = 30 s`) : sur les sessions de la recette, il n'a **jamais** été émis. Un zéro
   bénin, indiscernable du défaut que le contrôle existe pour révéler — le piège « un contrôle qui se
   déclenche trop tôt ne contrôle rien », rejoué sur un contrôle écrit pour l'éviter.

## 4. Ce que D7 n'établit PAS

- **Aucun taux** : 2 exécutions du critère ①, 1 des critères ② ④ ⑤, 4 relevés au §1.
- **Le critère ③ n'a pas été exercé** : l'audio d'une fenêtre **endormie** (au-delà des huit éveillées
  du vivier de D5) n'a jamais été observé. Le chemin existe et est raisonné — le sommeil ne touche pas
  `emet` —, mais **il n'a pas été vu tourner**. C'est la lacune la plus lourde de cette recette.
- **Le plafond d'activations *process loopback* concurrentes reste inconnu.** Deux au plus ont
  coexisté. La forme exacte de l'inférence que D3 a dû payer sur DXGI.
- **Rien au-delà de deux fenêtres.**
- **Rien des applications UWP**, dont le rendu audio peut ne pas vivre dans l'arbre du processus
  propriétaire.
- **Aucun jugement d'écoute** : la fréquence est mesurée, la qualité perçue ne l'est pas — même lacune
  que `BPP_MIN` traîne depuis le chantier C.
- **Rien de la latence**, rien de la durée (session la plus longue < 1 min), aucun redimensionnement,
  aucun recouvrement, aucun déplacement de fenêtre, aucun clavier.
- **Le plancher de 1 LSB du §1 n'a pas été confronté au DTX** : qu'un flux « muet » à ±1 LSB fasse ou
  non retomber Opus à quelques octets par trame reste **une inférence, jamais mesurée**.
- **Les trois couches inconnues le restent** : le plafond de 8 encodeurs, celui de 4 processus, et le
  mécanisme de l'abandon du mutex DXGI.
