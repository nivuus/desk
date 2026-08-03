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
un flux Opus au débit nominal au lieu de quelques octets par trame — à vérifier
à la recette de la tâche 13, où `bytesReceived` le dira.

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
