# Chantier B — Input jeu : recette et résultats

**Date d'exécution** : 29 juillet 2026
**Binaire agent** : recompilé pour cette recette (`scripts/build-agent.sh`,
`Finished \`release\` profile [optimized] target(s) in 4.86s`, seuls 3
avertissements `dead_code` préexistants sans rapport avec ce chantier).
**Plateforme agent** : VM Windows (build 10.0.20348.5386), GPU NVIDIA RTX
4070, résolution de session interactive **2400×1080**.
**Navigateur client (mesures 3, 4, 5 et non-régression bureautique)** :
Google Chrome 150.0.7871.181 (`--headless=new`), piloté par CDP brut depuis
l'hôte de développement (Debian GNU/Linux 13, noyau 6.12.96+deb13-amd64) —
**pas** un poste client réel ni un Chromebook ; voir « Réserves » pour ce que
ça implique.
**Application capturée pendant les mesures 3, 4, 5 et la non-régression** :
Firefox, lancé sur la VM (`WINDOW_TITLE=firefox`), en session interactive
fraîche.

---

## Le point le plus important de cette recette : la VM a été redémarrée à froid

`virsh list --all` donnait la VM `Windows` **fermée** au début de cette tâche
— une occasion qui ne se représentera pas facilement, puisque
`SPI_SETMOUSE`/`SPI_SETMOUSESPEED` neutralisent l'accélération pointeur pour
la **session Windows entière**, pas pour le processus, et ce réglage n'est
jamais explicitement restauré (voir tâche 8). Sur la session longue-durée
utilisée par toutes les tâches précédentes, l'écart nul mesuré « sans
neutralisation » pouvait tout aussi bien refléter une session déjà neutralisée
par un essai antérieur qu'une VM sans accélération native — réserve restée
ouverte faute de session fraîche.

Conformément à la consigne reçue pour cette tâche, la toute première action
sur la VM (après `virsh start Windows` et l'attente de WinRM) a donc été la
mesure de référence, **avant** tout autre lancement de l'agent :

```bash
virsh list --all
# ID   Nom       État
# --------------------------------------
#  -    Windows   fermé
virsh start Windows
until timeout 3 bash -c 'echo > /dev/tcp/192.168.3.2/5985' 2>/dev/null; do sleep 5; done
set -a && source .env && set +a
scripts/build-agent.sh
INPUT_LINEARITY_PROBE=1 INPUT_LINEARITY_NEUTRALISER=0 scripts/run-agent.sh
```

**Sortie brute** (`agent.log`, `iconv -f UTF-16LE -t UTF-8`) :

```
WARN neutralisation SAUTÉE au démarrage (INPUT_LINEARITY_NEUTRALISER=0, mesure de référence)
WARN neutralisation SAUTÉE (mesure de référence)
INFO sonde de linéarité terminée attendu=1000 obtenu_x=1000 obtenu_y=1000 ecart_x=0 ecart_y=0
```

**Verdict : écart nul, sur session fraîche, jamais neutralisée auparavant.**
La réserve de la tâche 8 est levée : ce n'est pas une session polluée par un
essai antérieur qui produisait un écart nul, c'est une propriété réelle de
cette VM (Windows Server, build 20348) — plausiblement l'absence par défaut de
« Améliorer la précision du pointeur » sur les éditions serveur. Cela **ne
prouve pas que la neutralisation serait inutile en général** : elle protège
contre un réglage utilisateur ou une image système différente qui activerait
l'accélération ; elle confirme seulement qu'elle n'a rien à corriger sur
cette VM précise. Après cette mesure, l'agent a normalement neutralisé la
session pour toutes les mesures suivantes — une session fraîche pour rejouer
cette référence n'est donc plus possible sans un nouveau redémarrage complet
de la VM.

---

## Verdict global

**Sur les cinq mesures : deux atteignent leur critère sans aucune réserve
(1, linéarité ; 2, manette relue par XInput). Deux atteignent leur critère
mais avec une réserve méthodologique explicite (3, vibration — manette
simulée faute de matériel physique ; 5, bascule de mode — latence isolée par
une méthode de mesure corrigée en cours de tâche, voir §6). Une est
partielle (4, Échap en plein écran — le critère utilisateur passe, le
critère agent ne peut pas être vérifié tel que le brief le prescrit).**
Aucune mesure ne passe un critère qu'elle n'atteint pas réellement, et aucune
n'est comptée deux fois entre les catégories « sans réserve » et « avec
réserve » — un lecteur pressé qui ne retiendrait qu'un chiffre doit repartir
avec **2 sur 5 sans réserve**, pas un score arrondi vers le haut.

Trois discordances notables entre le brief et le code livré ont été
découvertes et documentées plutôt que contournées en silence : le mécanisme
de comptage par `agent.log` prescrit pour les mesures 3 et 5 n'existe pas
dans le code réel (aucune ligne de journal n'est émise à l'envoi réussi d'un
message de contrôle), le mécanisme de comptage par `agent.log` prescrit pour
le critère 1 de la mesure 4 (`RUST_LOG=debug`, `Key { scancode: 1`) est du
code mort sur la cible Windows réelle (`#[cfg(not(windows))]`), et la
première méthode de déclenchement de la mesure 5 (tâche planifiée) s'est
révélée mesurer surtout du bruit de lancement de processus Windows plutôt
que le chemin de signal visé par le critère — corrigée en cours de tâche
(§6, ronde de correction 1). Les contournements ont été faits par une
instrumentation client temporaire, décrite et **retirée avant chaque
commit**. L'installation de Steam/TF2/Dota 2 est bloquée par une
authentification à deux facteurs hors de portée d'un agent ; documentée
comme étape utilisateur.

---

## 1. Tableau des cinq mesures instrumentées

| # | Mesure | Résultat | Critère | Verdict |
|---|---|---|---|---|
| 1 | Linéarité de la visée (deux réglages de pas) | `ecart_x=0 ecart_y=0` aux deux réglages (pas=10/rép=100 et pas=200/rép=5) | `\|écart\| ≤ 1` sur 1000 px | **Atteint, sans réserve** |
| 2 | Manette relue par XInput | `B=4096 (0x1000) LT=128 RT=0 LX=16384 LY=0 RX=0 RY=0`, `retour=0` | champs conformes ±1 LSB | **Atteint, sans réserve** (exact, écart 0) |
| 3 | Vibration reçue côté client | `rumble left=156 right=78` reçu pour `L=40000,R=20000` appliqués ; cadence 0,1–0,2 msg/s | cadence ≤ 50/s | **Atteint, avec réserve méthodologique** — manette simulée (pas de matériel physique) ; méthode de mesure du brief (grep `agent.log`) non applicable, voir §4 |
| 4 | Échap en plein écran | `fullscreenElement` non nul après Échap (3 essais) ; le client envoie bien le scancode 1 sur le canal d'entrée | scancode 0x01 journalisé par l'agent **et** plein écran maintenu | **Partiel** — critère « plein écran maintenu » atteint ; critère « scancode journalisé par l'agent » non vérifiable tel que prescrit, voir §5 |
| 5 | Bascule de mode (curseur masqué) | latence isolée du chemin de signal, hors bruit de lancement de processus : **131–166 ms** (3 essais exploitables) — voir §6 pour la méthode corrigée et la première mesure (234–308 ms), confondue par du bruit, supplantée ; **zéro oscillation** après stabilisation sur 60 s (essais initiaux) | < 250 ms **et** zéro oscillation | **Atteint, avec réserve méthodologique** — voir §6, ronde de correction 1 |

---

## 2. Mesure n°1 — Linéarité de la visée

### Commandes exactes

```bash
INPUT_LINEARITY_PROBE=1 scripts/run-agent.sh && sleep 10
INPUT_LINEARITY_PROBE=1 INPUT_LINEARITY_PAS=200 INPUT_LINEARITY_REPETITIONS=5 scripts/run-agent.sh && sleep 10
grep "sonde de linéarité" /media/vm/dev/agent.log
```

### Sortie brute

Pas=10, répétitions=100 (défauts) :

```
INFO neutralisation appliquée rapport="seuils relus = [0, 0, 0], vitesse relue = 10 (attendu : [0, 0, 0] et 10)"
INFO point de départ de la sonde largeur=2400 hauteur=1080 centre_x=20 centre_y=20 amplitude=1000
INFO sonde de linéarité terminée attendu=1000 obtenu_x=1000 obtenu_y=1000 ecart_x=0 ecart_y=0
```

Pas=200, répétitions=5 :

```
INFO neutralisation appliquée rapport="seuils relus = [0, 0, 0], vitesse relue = 10 (attendu : [0, 0, 0] et 10)"
INFO point de départ de la sonde largeur=2400 hauteur=1080 centre_x=20 centre_y=20 amplitude=1000
INFO sonde de linéarité terminée attendu=1000 obtenu_x=1000 obtenu_y=1000 ecart_x=0 ecart_y=0
```

### Critère et verdict

`|ecart_x| ≤ 1` et `|ecart_y| ≤ 1` sur 1000 px, aux deux réglages : **atteint,
exactement (écart nul)**, sur le binaire final recompilé pour cette recette —
confirme la mesure de la tâche 8 sur une base fraîche. Combinée à la mesure de
référence sans neutralisation (ci-dessus), la conclusion est maintenant
complète : l'injection relative est linéaire 1:1 que la neutralisation soit
appliquée ou non **sur cette VM précise**, et la neutralisation reste la
protection correcte pour toute VM/image qui aurait une accélération par
défaut.

---

## 3. Mesure n°2 — Manette relue par XInput

### Commande exacte

```bash
VIGEM_PROBE=1 VIGEM_PROBE_SECS=30 scripts/run-agent.sh
sleep 6
node scripts/winrm.js "powershell -NoProfile -ExecutionPolicy Bypass -File C:\dev\xinput-read.ps1 2>&1 | Out-String"
```

où `C:\dev\xinput-read.ps1` (déposé depuis Linux via `/media/vm/dev/`, même
motif que la sonde de la tâche 1 — l'heredoc `@'...'@` direct casse le
parseur PowerShell une fois traversé par `node`) contient :

```powershell
Add-Type -TypeDefinition @"
using System.Runtime.InteropServices;
public class XI2 {
  [StructLayout(LayoutKind.Sequential)] public struct PAD { public ushort B; public byte LT; public byte RT; public short LX; public short LY; public short RX; public short RY; }
  [StructLayout(LayoutKind.Sequential)] public struct ST { public uint Packet; public PAD Gamepad; }
  [DllImport("xinput1_4.dll")] public static extern int XInputGetState(int i, ref ST s);
}
"@
$s = New-Object XI2+ST
$r = [XI2]::XInputGetState(0, [ref]$s)
"retour=$r"
$s.Gamepad | Format-List | Out-String
```

L'état appliqué par la sonde (`agent/src/gamepad.rs::probe`) est fixe et
documenté dans le code : `buttons: XButtons!(A)` (0x1000), `left_trigger:
128`, `thumb_lx: 16384`, le reste à zéro.

### Sortie brute

```
retour=0

B  : 4096
LT : 128
RT : 0
LX : 16384
LY : 0
RX : 0
RY : 0
```

### Critère et verdict

`4096 = 0x1000` (bouton A), `LT=128`, `LX=16384` : **conformes exactement**
(écart de 0 LSB, pas seulement ≤ 1), `RT`, `LY`, `RX`, `RY` à zéro comme
attendu. `retour=0` (`ERROR_SUCCESS`) confirme que XInput voit bien un
contrôleur connecté. **Atteint.**

Journal agent correspondant (`sonde ViGEmBus`) :

```
INFO agent: sonde ViGEmBus rapport="branchement OK, 4 notification(s) de vibration reçue(s)"
```

Les 4 notifications sont du bruit d'énumération (valeurs 0/0), comme déjà
observé et documenté en tâche 1 — aucune vibration n'a été demandée par cette
mesure précise, seule la lecture d'état comptait ici.

---

## 4. Mesure n°3 — Vibration reçue côté client

### Ce que le brief prescrivait, et pourquoi ça ne fonctionne pas tel quel

Le brief propose deux méthodes : compter les messages `rumble` dans la
console du navigateur, ou plus simplement `grep -c '"type":"rumble"'
/media/vm/dev/agent.log`. **La seconde ne fonctionne pas avec le code livré.**
Vérifié par lecture de `agent/src/transport.rs` (fonction `act_on_timeout`,
autour de la ligne 650) : le message de contrôle sortant est bien sérialisé
en JSON (`serde_json::to_string(&message)`) et écrit sur le canal, mais
**aucune ligne de journal n'est produite à cet endroit en cas de succès** —
seul un `tracing::warn!` existe, et seulement en cas d'**échec** d'écriture.
`grep -c '"type":"rumble"' /media/vm/dev/agent.log` a été exécuté pour
vérifier cette lecture de code par les faits : **0** occurrences, y compris
pendant une vibration réellement en cours (confirmée par ailleurs, voir
ci-dessous). Ce n'est pas un défaut de la mesure : c'est un vrai manque
d'observabilité du code de ce chantier, à signaler pour un chantier de
suivi (ajouter un `tracing::debug!` au point d'écriture réussie).

### Méthode effectivement utilisée

Deux substitutions, documentées comme telles :

1. **Aucune manette physique n'est disponible dans cet environnement.** La
   Gamepad API n'expose une manette qu'après un appui physique dessus — pas
   simulable par un simple script. `navigator.getGamepads` a donc été
   surchargé par un script injecté avant navigation (CDP
   `Page.addScriptToEvaluateOnNewDocument`), fournissant une manette
   fabriquée (`connected: true`, mapping `standard`, boutons/axes neutres,
   `vibrationActuator.playEffect` instrumenté). C'est une **fabrication de
   test assumée**, pas du matériel réel — mais elle exerce le vrai code de
   `client/src/gamepad.ts` (sondage à 4 ms, émission `GamepadState`,
   réception `rumble`), pas un contournement de ce code.
2. **Aucune ligne `agent.log` n'existant pour les messages de contrôle
   sortants** (voir ci-dessus) **ni pour les messages `rumble` reçus côté
   client** (`client/src/main.ts` ne loggue rien dans sa branche
   `message.type === 'rumble'`), une instrumentation **temporaire** a été
   ajoutée à `client/src/main.ts` (un `console.log` dans les branches
   `pointer` et `rumble` de `onControl`), utilisée pour cette mesure puis
   **retirée avant tout commit** (`git checkout -- client/src/main.ts`,
   confirmé par `git status` propre en fin de tâche).

### Commande exacte (VM, script déposé sur le partage)

```powershell
Add-Type -TypeDefinition @"
using System.Runtime.InteropServices;
public class XIV {
  [StructLayout(LayoutKind.Sequential)] public struct VIB { public ushort L; public ushort R; }
  [DllImport("xinput1_4.dll")] public static extern int XInputSetState(int i, ref VIB v);
}
"@
$v = New-Object XIV+VIB
$v.L = 40000
$v.R = 20000
1..50 | ForEach-Object { [XIV]::XInputSetState(0, [ref]$v); Start-Sleep -Milliseconds 200 }
"vibration envoyee"
```

invoqué via `node scripts/winrm.js "powershell ... -File C:\dev\xinput-vibe-t16.ps1"`
pendant qu'une session CDP (Chrome headless, `client/src/main.ts` instrumenté
temporairement) observait la console de la page.

### Sortie brute (console du client, capturée par CDP)

```
[log] [recette] rumble left=0 right=0 t=990.6
[log] [recette] rumble left=156 right=78 t=11990.3
[log] [recette] rumble left=0 right=0 t=22145.2
[log] [recette] playEffect type=dual-rumble params={"duration":200,"strongMagnitude":0.611764705882353,"weakMagnitude":0.3058823529411765}
```

`156/255 = 0,6118` et `78/255 = 0,3059` : conversion exacte des magnitudes
`strongMagnitude`/`weakMagnitude` côté `gamepad.ts`. `156` et `78`
correspondent exactement à la conversion 16 bits → 8 bits de `L=40000,
R=20000` (`40000/65535*255 ≈ 156`, `20000/65535*255 ≈ 78`) — même conversion
que celle déjà observée en tâche 1.

### Critère et verdict

**Trois** messages `rumble` sur une fenêtre de 10 à 21 s selon les essais
(deux essais indépendants) : `0,1` à `0,2` message/s, très en deçà de la
limite de 50/s. Ce chiffre est structurellement bas parce que l'émission se
fait **sur changement de valeur**, jamais en continu — ViGEmBus lui-même ne
notifie que sur changement (déjà observé en tâche 1), et le code de vibration
de l'agent respecte le même principe (§6 de la spec : « au plus une toutes
les 20 ms », jamais atteint ici puisque la valeur ne change que deux fois).
**Atteint**, avec la réserve méthodologique ci-dessus sur la manette simulée.

---

## 5. Mesure n°4 — Échap en plein écran

### Ce que le brief prescrivait, et pourquoi le critère 1 n'est pas vérifiable tel quel

Le brief demande de lancer l'agent avec `RUST_LOG=debug` et de chercher
`Key { scancode: 1` dans `agent.log`. **Vérifié par lecture de code
(`agent/src/main.rs`, fermeture `on_input` autour de la ligne 1137-1180)** :
la seule ligne qui journalise le `Debug` complet d'un `InputMessage`
(`tracing::debug!(?message, "entrée reçue")`) est gardée par
`#[cfg(not(windows))]` — c'est du code **mort sur la cible Windows réelle**,
qui compile l'agent avec `#[cfg(windows)]` actif. Sur le chemin réellement
emprunté (Windows), le message part directement dans
`injector.inject(message)`, sans journal. Confirmé en pratique : agent
relancé avec `RUST_LOG=debug`, plein écran + Échap exercés, **zéro**
occurrence de `Key { scancode` dans `agent.log` malgré un Échap réellement
envoyé (voir ci-dessous). C'est un deuxième manque d'observabilité du même
type que celui de la mesure 3, à signaler pour suivi.

### Méthode effectivement utilisée pour le critère 1

Le canal d'entrée (`RTCDataChannel`) a été instrumenté côté client par script
injecté CDP (interception de `createDataChannel`/`send`, décodage du format
binaire documenté dans `proto/ts/input.ts` : octet 1 = type, `4` pour
`Key`, octets 2-3 = scancode LE, octet 4 = pressed, octet 5 = extended).
Cette instrumentation était strictement côté navigateur piloté par CDP — pas
une modification du dépôt.

### Commande exacte (CDP, résumé du script)

1. Clic (CDP `Input.dispatchMouseEvent`, trusted) sur `#fullscreen`.
2. Attente de `document.fullscreenElement !== null`.
3. Frappe Échap (CDP `Input.dispatchKeyEvent`, `code: 'Escape'`,
   `windowsVirtualKeyCode: 27`) — un événement clavier **synthétisé par le
   protocole DevTools**, traité par Chrome comme un geste réel (pas un simple
   `dispatchEvent` JS, qui n'aurait pas cette portée).
4. Relecture de `document.fullscreenElement !== null`.

### Sortie brute

```
plein écran engagé après clic : true
fullscreenElement avant Échap : true / après Échap : true
CRITÈRE 2 (reste en plein écran) : PASS
lignes "envoi Key" interceptées sur le canal d'entrée : 2
  [log] [recette] envoi Key scancode=1 pressed=1 extended=0
  [log] [recette] envoi Key scancode=1 pressed=0 extended=0
```

Reproduit **trois fois** au total (trois sessions agent distinctes,
`recette3`, `recette5`, `recette6`) : `fullscreenElement !== null` après
Échap systématiquement vrai.

### Critère et verdict

- **Le navigateur reste en plein écran après Échap** : **atteint**, confirmé
  trois fois. `navigator.keyboard.lock()` est bien disponible sur Chrome
  150 headless et intercepte la sortie sur simple pression.
- **Le scancode 0x01 atteint l'agent** : **non vérifiable par journal**, pour
  la raison de code mort ci-dessus. Ce qui **est** établi : le client envoie
  bien, sur le canal d'entrée réel (pas un mock), l'octet exact du protocole
  v2 (`type=4, scancode=1, pressed=1` puis `pressed=0`) au moment de l'appui
  Échap sous Keyboard Lock — la trame part bien vers l'agent. La connexion
  WebRTC de cette même session étant par ailleurs prouvée active (vidéo et
  audio en cours de réception, voir la mise en place ci-dessous), il est
  raisonnable d'inférer que ces octets sont arrivés côté agent, mais ce n'est
  **pas une preuve directe** faute de journal — c'est la limite honnête de
  cette mesure, pas un résultat maquillé.

**Verdict global de la mesure : partiel.** Le critère qui compte pour
l'utilisateur (le plein écran ne casse pas) est pleinement atteint ; le
critère instrumenté sur l'agent ne peut pas être vérifié tel que le brief le
prescrit, faute de journal existant.

---

## 6. Mesure n°5 — Bascule de mode (curseur masqué)

### Ce que le brief prescrivait, et pourquoi le critère de comptage ne s'applique pas non plus

Même famille de manque que la mesure 3 : `grep -c '"type":"pointer"'
agent.log` prescrit par le brief repose sur une ligne de journal qui n'existe
pas (`agent/src/cursor.rs`, point d'envoi du message `Pointer`, aucun
`tracing::info!`/`debug!` au moment de l'émission). Vérifié : 0 occurrence
dans `agent.log` sur toute la fenêtre de mesure. Remplacé par la même
instrumentation client temporaire que la mesure 3 (retirée avant commit).

### Commande exacte

Déclenchement du masquage (script déposé sur le partage puis exécuté par
tâche planifiée, comme `run-agent.sh`) :

```powershell
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object System.Windows.Forms.Form
$f.WindowState = 'Maximized'
$f.Add_Shown({ [System.Windows.Forms.Cursor]::Hide() })
$f.Add_FormClosing({ [System.Windows.Forms.Cursor]::Show() })
$f.ShowDialog()
```

Observation côté client : console capturée par CDP dès l'instant où
`schtasks /run` retourne (le déclenchement lui-même), jusqu'à 60 s après.

### Sortie brute (trois essais indépendants, premier jet — voir la ronde de correction plus bas)

| Essai | Latence déclenchement → `visible=false` | Messages `pointer` sur 60 s après stabilisation | Transitions |
|---|---|---|---|
| 1 | 308 ms | 1 | 1 |
| 2 | 293 ms | 3 (2 transitoires + 1 stable) | 3 — voir note [^transitoires] |
| 3 | 234 ms | 1 | 1 |

[^transitoires] : les deux premiers messages de l'essai 2 précèdent la
stabilisation en `visible=false` et ne comptent pas comme une oscillation
au sens du critère — voir « Analyse des transitoires » juste après.

Extrait brut de l'essai 3 (le plus propre) :

```
[log] [recette] pointer visible=false shape=default t=23839.8
```

seul message `pointer` sur toute la fenêtre de 60 s suivant le déclenchement.

Extrait brut de l'essai 2, montrant les transitoires :

```
[log] [recette] pointer visible=true shape=ns-resize t=18613.8
[log] [recette] pointer visible=true shape=default t=18666.7
[log] [recette] pointer visible=false shape=default t=18868.4
```

### Analyse des transitoires (essai 2) — pas une oscillation au sens du critère

Les deux premiers messages (`ns-resize`, puis `default`) arrivent **avant**
la stabilisation en `visible=false`, à quelques dizaines de millisecondes
d'écart : ils correspondent au **survol réel** de la bordure de
redimensionnement pendant que la fenêtre `WindowState = Maximized` achève sa
transition d'agrandissement, avant que `Cursor.Hide()` (posé dans
`Add_Shown`) ne prenne effet. Après le troisième message (`visible=false`),
**aucun** message supplémentaire sur le reste des 60 s — la définition du
critère (« aucune oscillation », c'est-à-dire un décompte de messages
stable) porte sur l'état **après stabilisation**, qui est bien atteint dans
les trois essais.

### Sur la latence mesurée (234–308 ms) : ce qu'elle mesure vraiment

**Ce premier jet n'isolait pas proprement le critère qu'il prétendait
chiffrer.** Le point de départ du chronomètre (`Date.now()` juste après que
`schtasks /run` a rendu la main) incluait :

- le délai avant que le Planificateur de tâches Windows démarre réellement le
  processus (`schtasks /run` rend la main avant exécution effective — déjà
  documenté dans `check-session.sh` et le rapport de tâche 1) ;
- le chargement de l'assembly `System.Windows.Forms` et la construction de la
  fenêtre jusqu'à l'événement `Shown` qui appelle `Cursor.Hide()` ;
- **seulement ensuite** le chemin que le critère du brief vise réellement :
  sondage `GetCursorInfo` toutes les 50 ms côté agent, hystérésis de 3
  échantillons cohérents (~150 ms par construction, §5 de la spec), puis
  transport jusqu'au client.

En l'état, ce premier jet donnait un verdict littéral d'échec sur deux essais
sur trois (293 ms et 308 ms, tous deux au-dessus de 250 ms) — un jugement
honnête aurait dû le dire tel quel plutôt que le qualifier de « non tranché »
en attendant une hypothèse non vérifiée. C'est corrigé ci-dessous, pas
seulement reformulé.

### Ronde de correction 1 — isolement effectif du chemin de signal

**Choix : trancher, pas requalifier.** Une seconde mesure a été construite
pour isoler le chemin de signal (sondage agent + hystérésis + transport) du
bruit de lancement de processus Windows, en retirant ce bruit de la fenêtre
chronométrée plutôt qu'en le supposant.

**Méthode** : un script PowerShell unique, lancé **une seule fois** en tâche
planifiée et laissé « chaud » (assemblage `System.Windows.Forms` déjà chargé,
formulaire déjà maximisé et **au premier plan** — condition nécessaire,
découverte en cours de route : voir plus bas), attend en boucle serrée
(`Timer` à 3 ms) qu'un fichier déclencheur apparaisse sur le partage monté,
puis appelle immédiatement `Cursor.Hide()` — sans relancer de processus, sans
recharger d'assembly, sans reconstruire de fenêtre. Le déclenchement se fait
en écrivant ce fichier **directement depuis Linux** (écriture sur le partage
9p déjà monté, pas un aller-retour WinRM/`schtasks`) : la seule horloge de
référence est celle du script Node qui pilote la mesure, **des deux côtés**
(écriture du déclencheur et réception du message `pointer` sur la même
horloge `Date.now()`) — aucune synchronisation d'horloge Linux/Windows n'est
nécessaire, contrairement à ce qu'une mesure par horodatage croisé aurait
exigé.

**Écueil rencontré et corrigé en cours de route** : la toute première version
de ce script n'utilisait aucun formulaire (juste `Cursor.Hide()` appelé
depuis le fil principal d'un script sans fenêtre). Résultat : **zéro**
message `pointer` reçu sur 4 essais, alors que le fichier d'acquittement
confirmait que `Cursor.Hide()` avait bien été appelé côté Windows. Cause :
`ShowCursor()` (que `Cursor.Hide`/`Show` enveloppent) maintient un compteur
**par fil**, et son effet sur le curseur système affiché n'est visible que
si le fil appelant possède réellement la fenêtre au premier plan — un fil
sans fenêtre n'a aucune prise sur ce qui est physiquement affiché, même s'il
peut appeler la fonction sans erreur. Corrigé en donnant au script un vrai
formulaire maximisé (`$f.TopMost = $true`, boucle de messages via
`[System.Windows.Forms.Application]::Run($f)`), qui devient le véritable
propriétaire du curseur affiché.

**Commande exacte** (script déposé une fois sur `C:\dev\warm-hide-cursor.ps1`,
lancé une seule fois par tâche planifiée, réutilisé pour tous les essais) :

```powershell
Add-Type -AssemblyName System.Windows.Forms
$f = New-Object System.Windows.Forms.Form
$f.WindowState = 'Maximized'
$f.TopMost = $true
$triggerPath = 'C:\dev\trigger-hide.txt'
$ackPath = 'C:\dev\ack-hide.txt'
$script:hidden = $false
$script:count = 0
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 3
$timer.Add_Tick({
    if (-not $script:hidden) {
        if (Test-Path $triggerPath) {
            [System.Windows.Forms.Cursor]::Hide()
            (Get-Date).ToString('o') | Out-File -FilePath $ackPath -Encoding ascii -NoNewline
            Remove-Item $triggerPath -ErrorAction SilentlyContinue
            $script:hidden = $true; $script:hiddenAt = Get-Date
        }
    } else {
        if (((Get-Date) - $script:hiddenAt).TotalMilliseconds -gt 1800) {
            [System.Windows.Forms.Cursor]::Show()
            $script:hidden = $false; $script:count++
            if ($script:count -ge 8) { $timer.Stop(); $f.Close() }
        }
    }
})
$f.Add_Shown({ $timer.Start() })
[System.Windows.Forms.Application]::Run($f)
```

côté Linux, pour chaque essai : écrire `go` dans
`/media/vm/dev/trigger-hide.txt` (le partage monté), horodater `t0 =
Date.now()`, puis attendre le premier message console `[recette] pointer
visible=false` postérieur à `t0`.

### Sortie brute (5 essais, sur une session agent fraîche dédiée à cette ronde)

```
essai 1/5 : latence = 139 ms (ack Windows=null, lu avant écriture complète — sans incidence sur la mesure côté client)
essai 2/5 : AUCUN message visible=false observé dans les 5s (ack=2026-07-29T11:08:40.6864433+02:00)
essai 3/5 : latence = 166 ms (ack=2026-07-29T11:08:46.0476230+02:00)
essai 4/5 : AUCUN message visible=false observé dans les 5s (ack=2026-07-29T11:08:47.8691696+02:00)
essai 5/5 : latence = 131 ms (ack=2026-07-29T11:08:53.2344039+02:00)

résultats (ms) : 139, 166, 131
moyenne : 145.3 ms, min=131 max=166
```

**Sur les deux essais sans message (2 et 4) : la cause est en réalité
établissable à partir des horodatages ci-dessus et du script lui-même — ce
n'est pas un incident du transport, c'est le comportement correct de
l'hystérésis face à un artefact du montage de mesure.**

Le script « chaud » ne fait pas qu'attendre le déclencheur écrit depuis
Linux : une fois masqué, il **se remontre lui-même** au bout de 1800 ms
(`if (((Get-Date) - $script:hiddenAt).TotalMilliseconds -gt 1800) {
[System.Windows.Forms.Cursor]::Show(); $script:hidden = $false; ... }`), sans
attendre aucun signal externe. Si le fichier déclencheur de l'essai suivant a
déjà été écrit à ce moment-là (ce qui arrive dès que le côté Linux écrit le
prochain déclencheur avant l'expiration des 1800 ms de l'essai précédent),
le **tick de minuterie suivant, 3 ms plus tard** (`$timer.Interval = 3`), voit
`$script:hidden` retombé à `false`, trouve le fichier déjà présent
(`Test-Path $triggerPath`) et rappelle `Cursor.Hide()` immédiatement.

C'est exactement ce que montre l'écart entre les accusés de réception de
l'essai 3 (qui a produit un message) et de l'essai 4 (qui n'en a pas
produit) : `11:08:47.8691696 − 11:08:46.0476230 = 1,8215 s`, soit la fenêtre
fixe de 1800 ms du script plus un tick de 3 ms (plus la latence
d'ordonnancement Windows habituelle sur ce genre de mesure) — l'essai 4 a
été déclenché par le **propre réaffichage automatique** du script hérité de
l'essai 3, pas par une action distincte. Le curseur n'a donc été visible que
le temps d'un seul tick de minuterie, ~3 ms, avant d'être immédiatement
remasqué. C'est très en dessous des ~150 ms que l'hystérésis de l'agent
exige avant de retenir un changement (`agent/src/cursor.rs` : `SEUIL = 3`
échantillons cohérents à `PERIODE = 50 ms`) — le sondage à 50 ms de l'agent
n'a structurellement aucune chance d'observer 3 échantillons consécutifs
« visible » dans une fenêtre de 3 ms, et le plus probable est qu'il n'en
observe même aucun. `Hysteresis::courant` n'a donc jamais quitté `false`,
`observer()` ne rend jamais `Some`, et **aucun message n'est émis : c'est le
comportement correct et voulu de la machine à états**, pas une perte sur le
chemin de signal. La même mécanique explique l'essai 2 par construction,
même si l'accusé de réception de l'essai qui le précède (essai 1) n'a pas pu
être lu proprement pour en donner l'arithmétique exacte.

Ce n'est donc pas une question ouverte : c'est un artefact du montage de
mesure (un script « chaud » réutilisé d'un essai à l'autre, dont le
réarmement automatique à 1800 ms peut coïncider avec l'écriture anticipée du
déclencheur suivant), pas une perte de messages du mécanisme de bascule. La
correction précédente (retrait de l'hypothèse « fenêtre de 1,8 s trop
courte pour l'hystérésis ») restait de toute façon juste : ce n'est pas la
durée du masquage qui est en cause, c'est la durée de la **réapparition**
intercalée, elle, bien trop courte.

### Critère et verdict

- **Zéro oscillation après stabilisation, sur 60 s** : **atteint**, dans les
  trois essais du premier jet (§ précédente).
- **Latence < 250 ms** : **atteint**, sur la base de la mesure isolée
  ci-dessus (131–166 ms, moyenne 145 ms sur 3 essais exploitables) — très
  proche de l'estimation de conception (~150 ms d'hystérésis, §5 de la
  spec). Le premier jet (234–308 ms, échec littéral sur 2 essais sur 3) est
  **supplanté**, pas effacé : il reste documenté ci-dessus comme la preuve
  qu'une méthode de déclenchement naïve mesure surtout le bruit
  d'ordonnancement Windows, et comme rappel que la correction n'a pas
  consisté à relancer la mesure jusqu'à obtenir le bon chiffre, mais à
  changer la méthode pour retirer un bruit identifié et expliqué.

**Verdict global de la mesure : atteint, avec réserve méthodologique** — les
deux critères passent sur la mesure corrigée ; la réserve porte sur le
caractère artisanal du montage de déclenchement « à chaud » (script unique,
non représentatif d'un masquage de curseur déclenché par un vrai jeu) et sur
les deux essais sans résultat exploitable (discutés ci-dessus).

---

## 7. Step 9 — Non-régression bureautique

Critère du brief (et point 5 du §12 de la spec) : une session ordinaire
affiche un curseur visible qui change de forme au survol d'un champ texte, et
la souris reste absolue.

### Commande exacte

Dans la même session Chrome/CDP que les mesures 3-4-5 (agent capturant
Firefox, mode absolu par défaut, aucun jeu ne masquant le curseur) :
déplacement du curseur (CDP `Input.dispatchMouseEvent`, `mouseMoved`) vers
huit points de la zone vidéo, lecture de
`getComputedStyle(document.querySelector('#remote')).cursor` après chaque
déplacement.

### Sortie brute

```
rect vidéo : {"x":0,"y":0,"w":780,"h":493}
  (0.50,0.06) -> px(390,30) cursor=default
  (0.50,0.09) -> px(390,44) cursor=default
  (0.50,0.12) -> px(390,59) cursor=text
  (0.30,0.09) -> px(234,44) cursor=default
  (0.70,0.09) -> px(546,44) cursor=default
  (0.50,0.15) -> px(390,74) cursor=text
  (0.20,0.06) -> px(156,30) cursor=default
  (0.80,0.06) -> px(624,30) cursor=default
```

### Critère et verdict

Le curseur change bien de forme (`default` → `text`) au survol d'une zone de
la fenêtre Firefox correspondant à un champ texte (barre d'adresse ou
équivalent) — reproduit à l'identique dans deux sessions distinctes
(`recette3`, `recette5`). Aucun message `pointer visible=false` n'a été émis
pendant toute cette phase : la souris est restée en mode absolu. **Atteint.**
Les coordonnées exactes du champ texte n'ont pas été identifiées
précisément (approche par balayage de quelques points plausibles) : suffisant
pour prouver que le mécanisme fonctionne, pas pour cartographier l'UI de
Firefox.

---

## 8. Jeu réel — Team Fortress 2 et Dota 2 : bloqué, étape utilisateur

### Ce qui bloque, et pourquoi ça ne peut pas être contourné par un agent

```bash
node scripts/winrm.js "winget install --id Valve.Steam --accept-source-agreements --accept-package-agreements"
```

L'installation de Steam elle-même n'est pas le problème (comme pour
ViGEmBus en tâche 1, `winget` peut échouer sur la source `msstore` — repli
possible sur l'installeur officiel). **Le blocage réel est en aval** :
la connexion à un compte Steam existant, avec authentification à deux
facteurs (Steam Guard, par email ou par l'app mobile Steam). Aucun compte ni
second facteur n'est disponible dans cet environnement, et il n'a pas été
question d'en fabriquer un — conformément à la consigne reçue, **cette étape
n'a pas été tentée**.

### Mode opératoire exact pour l'utilisateur

1. Démarrer la VM (`virsh start Windows`, attendre WinRM comme d'habitude) et
   ouvrir une session interactive (RDP ou console libvirt).
2. Installer Steam :
   ```powershell
   winget install --id Valve.Steam --accept-source-agreements --accept-package-agreements
   ```
   Si `winget` échoue sur la source `msstore` (comme observé en tâche 1 pour
   ViGEmBus), télécharger et lancer l'installeur officiel
   (`https://cdn.akamai.steamstatic.com/client/installer/SteamSetup.exe`).
3. Lancer Steam, se connecter avec un compte existant, valider le second
   facteur (code envoyé par email ou via l'app mobile Steam Guard).
4. Installer **Team Fortress 2** et **Dota 2** (gratuits tous les deux).
5. Dans les paramètres d'affichage de **chacun** des deux jeux, choisir
   **plein écran fenêtré** (pas plein écran exclusif — le plein écran
   exclusif court-circuite le compositeur Windows et casse la capture Desktop
   Duplication, §6 du cadrage jeux).
6. Ouvrir une session Guacamole sur chaque jeu et observer les critères
   qualitatifs ci-dessous.

### Critères qualitatifs à observer — Team Fortress 2 (mode relatif)

- Viser une cible sans dérive perceptible (corrèle avec la mesure n°1,
  linéarité, déjà validée par sonde).
- Tourner sur 360° sans blocage ni butée du curseur.
- Échap ouvre le menu pause **sans** quitter le plein écran du navigateur
  (corrèle avec la mesure n°4).
- La manette apparaît dans les options d'entrée du jeu (« Xbox 360
  Controller » ou équivalent XInput).
- La vibration est **ressentie** physiquement sur une vraie manette
  branchée côté client (impossible à vérifier autrement que par un humain
  avec du matériel réel).

### Critère qualitatif décisif — Dota 2 (mode absolu)

**Le curseur doit rester visible, et le mode relatif ne doit PAS se
déclencher.** C'est le test que la mesure n°5 (sonde artificielle) ne peut
pas remplacer : Dota 2 est un jeu à curseur visible qui pourrait, selon
son implémentation d'interface, masquer le curseur système par moments tout
en lisant sa position absolue — exactement la limite assumée au §5 de la
spec. **Si le mode relatif se déclenche à tort dans Dota 2** : ce n'est pas
un échec bloquant à corriger dans l'urgence — c'est le signal documenté par
la spec elle-même que `GetClipCursor` (curseur confiné à une zone, sans la
masquer) doit être ajouté comme second critère de bascule. Consigner
l'observation précisément (le curseur disparaît-il par intermittence ? dans
quels menus ? pendant le jeu actif ou seulement les menus ?) plutôt que de
conclure hâtivement dans un sens ou l'autre.

---

## 9. Sondes en attente — matériel humain requis, non fermées ici

Ces deux inconnues du §11 de la spec ont été délibérément reportées depuis la
tâche 1 (`docs/superpowers/plans/2026-07-28-input-jeu-sondes.md`), pour la
même raison qu'elles restent ouvertes ici : elles exigent une souris physique
et un opérateur humain devant l'écran, qu'aucun script ne peut simuler sans
fabriquer un résultat sans valeur probante.

### `movementX` sur les événements coalescés sous Pointer Lock

Fichier prêt : `client/probe-coalesced.html`. Mode opératoire (inchangé
depuis la tâche 1) :

```bash
cd client && npx vite --host 0.0.0.0
# ouvrir http://<hôte>:5173/probe-coalesced.html dans Chrome
```

Cliquer dans la zone grise (verrouille le pointeur), bouger la souris
continûment ~5 s, relever le verdict affiché (« coalescés exploitables » ou
« COALESCÉS INUTILISABLES »). Rien n'est bloqué en attendant : la tâche 12 a
déjà embarqué le repli prévu par la spec (retomber sur le `movementX` de
l'événement principal si la somme des coalescés est nulle).

### Cadence réelle de `setInterval(4 ms)` sous charge

Dans la console DevTools d'un onglet servant une session active :

```js
let n = 0; const t0 = performance.now();
const id = setInterval(() => { n++; }, 4);
setTimeout(() => { clearInterval(id); console.log('Hz réels :', (n / ((performance.now() - t0) / 1000)).toFixed(1)); }, 10000);
```

Seuil attendu : ≥ 200 Hz. Non mesuré ici pour la même raison (nécessite un
opérateur humain sur un vrai onglet, pas un navigateur piloté par script).

---

## 10. Réserves

- **Manette simulée, pas physique** (mesure 3) : `navigator.getGamepads`
  surchargé par script CDP. Le code réel de `gamepad.ts` est exercé, mais le
  comportement des VRAIS navigateurs/manettes physiques (mapping non
  garanti `standard` selon les modèles, latence de sondage réelle) n'est pas
  couvert — documenté comme limite assumée par la spec elle-même (§6).
- **Chrome headless, pas un poste client réel** : toutes les mesures CDP de
  cette recette tournent sur l'hôte de développement (Debian Linux), pas sur
  un Chromebook ni un poste Windows/macOS — contrairement à la mesure
  d'écoute humaine du chantier A qui, elle, portait directement sur la
  plateforme cliente cible. Aucune inférence n'est faite vers ces
  plateformes ici.
- **Mesure 5, latence — méthode corrigée en cours de tâche, réserve
  résiduelle** : la première mesure (234-308 ms) était confondue par du bruit
  de lancement de processus Windows, non représentative du chemin de signal.
  Corrigée par une seconde mesure isolée (131-166 ms, moyenne 145 ms, voir
  §6, ronde de correction 1) qui retire ce bruit plutôt que de le supposer.
  Réserve résiduelle sur **cette seconde mesure** : montage artisanal (script
  PowerShell unique, gardé « chaud », pas un déclenchement représentatif d'un
  vrai jeu qui masquerait le curseur pendant toute une session), dont le
  réarmement automatique à 1800 ms explique d'ailleurs les 2 essais sur 5
  sans message (2 et 4, discutés en §6) : le script s'est remasqué de
  lui-même sur un déclencheur écrit en avance, laissant le curseur visible
  environ 3 ms — bien en dessous des ~150 ms d'hystérésis de l'agent.
  **Ce n'est pas une perte de message, c'est le comportement correct et
  voulu face à un artefact du montage de mesure** — voir §6 pour
  l'arithmétique complète (`47,8691696 − 46,0476230 = 1,8215 s`, soit la
  fenêtre de 1800 ms du script plus un tick).
- **Critère agent de la mesure 4 non vérifiable par journal** — voir §5, un
  vrai manque d'observabilité (code mort `#[cfg(not(windows))]`), pas une
  invention comblée par autre chose que la lecture directe du canal
  d'entrée côté client.
- **Mojibake dans `agent.log` sur cette session fraîche** : les messages
  français accentués de cette session (post-redémarrage complet de la VM)
  s'affichent avec des caractères de substitution (par exemple
  `SAUT├ëE` pour `SAUTÉE`) une fois convertis d'UTF-16LE vers UTF-8 — signature
  d'un codepage console Windows (OEM 437/850) qui réinterprète les octets
  UTF-8 du process Rust avant leur réencodage en UTF-16LE par
  `Tee-Object`. Aucun rapport de tâche antérieur (sur la session
  longue-durée précédente) ne présente ce symptôme ; il est apparu avec
  cette session fraîche. **N'affecte aucune valeur numérique rapportée dans
  ce document** (tous les nombres cités sont en ASCII), seulement la lisibilité
  des messages français dans les extraits de journal — cités ici tels
  quels, sans les « nettoyer », conformément à l'exigence de sortie brute.
  Signalé pour un chantier de suivi (`chcp 65001` ou équivalent, à poser au
  même endroit que la neutralisation SPI si on veut une console UTF-8
  garantie pour toute session future).
- **Steam/TF2/Dota 2 non installés** — voir §8, bloqué par une
  authentification à deux facteurs hors de portée d'un agent. Tous les
  critères qualitatifs de ces deux jeux restent donc non observés.
- **Dialogue PowerShell de masquage du curseur (mesure 5)** : fermé après
  chaque essai (`Stop-Process` sur le PID relevé par le script lui-même dans
  `C:\dev\dialog-pid.txt`) — vérifié qu'aucun processus résiduel ne
  persistait entre les essais.

---

## 11. Ce que la mesure a coûté

- **La toute première tentative de la mesure de vibration (mesure 3) a
  échoué silencieusement**, pas par défaut du code : le script PowerShell
  généré contenait des antislashs superflus devant les guillemets de
  l'attribut `[DllImport(...)]` (artefact d'une précaution d'échappement
  inutile dans un template JS), ce qui a fait échouer la compilation C#
  (`Add-Type`) avec `TypeNotFound` — la boucle `XInputSetState` n'a donc
  jamais réellement modifié l'état de vibration lors du premier essai
  (une seule notification `0/0`, bruit d'énumération pris à tort pour un
  signal). Corrigé en retirant les antislashs ; le second essai a produit
  la preuve attendue (`left=156 right=78`). Rapporté ici parce que le
  premier essai, s'il avait été accepté tel quel, aurait produit un faux
  « la vibration ne fonctionne pas ».
- **Firefox ne se lançait pas sur la VM via `schtasks /tr` avec des
  guillemets imbriqués** (erreur `-2147024894`/`ERROR_FILE_NOT_FOUND` alors
  que le chemin de l'exécutable était correct) — corrigé par le même patron
  que `run-agent.sh` : déposer un script `.ps1` sur le partage et
  l'invoquer par chemin, plutôt que d'imbriquer les guillemets dans l'argument
  `/tr` d'une seule commande `schtasks`.
- **Restauration d'une session agent dégradée** : après la toute première
  vérification de connectivité (`verify-webrtc.mjs`), l'agent restait actif
  en arrière-plan et entrait dans une boucle de `WARN` UDP répétés après la
  fermeture de Chrome, plutôt que de se terminer proprement. Contourné en
  redémarrant systématiquement l'agent (`scripts/stop-agent.sh` puis
  `run-agent.sh` avec un nouvel identifiant de session) avant chaque nouvelle
  tentative de connexion — même précaution que celle déjà documentée dans le
  rapport de recette du chantier A.
- **Un oubli d'ordre a coûté la preuve directe du critère 1 de la mesure 4
  au premier passage** : l'agent a été redémarré (nouvel identifiant de
  session, pour la mesure 3) avant que le journal de la session qui avait
  servi à la mesure 4 n'ait été inspecté — or `Tee-Object` **écrase**
  `agent.log` à chaque redémarrage. Corrigé en rejouant la mesure 4 dans une
  session dédiée et en vérifiant le journal **avant** tout redémarrage
  suivant — c'est cette relecture qui a révélé que la ligne recherchée
  n'existe de toute façon pas sur la cible Windows (voir §5), pas seulement
  qu'elle avait été perdue par l'ordre des opérations.
- **Machine à outil créée pour cette recette** : `/tmp/mesure-t16.mjs`, un
  script Node autonome (CDP brut, même patron que `client/verify-webrtc.mjs`
  et `client/recette/harness.mjs`) qui enchaîne mesures 4, 9 (non-régression),
  3 et 5 dans une seule session de navigateur. **Non committé** — fichier de
  travail hors du dépôt, à usage unique pour cette tâche.
- **Ronde de correction 1 (revue) : le verdict initial de la mesure 5 arrondi
  vers le haut, et la latence non isolée du bruit de lancement.** La revue a
  signalé à raison que qualifier de « non tranché » une mesure dont 2 essais
  sur 3 dépassaient littéralement le seuil de 250 ms revenait à éviter d'écrire
  « échec » plutôt qu'à rapporter ce que la preuve autorisait. Plutôt que de
  requalifier en échec sec, une seconde mesure a été construite
  (`/tmp/mesure-t16-latence-isolee.mjs`, non committé) pour isoler
  effectivement le chemin de signal du bruit de lancement de tâche planifiée
  — voir §6. Cette seconde mesure a elle-même demandé une correction en cours
  de route : sa toute première version (sans fenêtre au premier plan) ne
  produisait aucun effet observable, pour une raison Win32 réelle
  (`ShowCursor()` est un compteur par fil, sans portée hors d'une fenêtre
  possédée par ce fil) découverte en pratique, pas supposée à l'avance. Le
  chiffre retenu (131-166 ms) vient de la version corrigée. La revue a aussi
  signalé un vrai défaut d'arithmétique du résumé (« quatre sur cinq » qui ne
  correspondait pas au tableau) : corrigé en recalibrant le résumé sur les
  catégories réellement distinctes (sans réserve / avec réserve
  méthodologique / partiel), sans arrondir aucune mesure vers le haut.
