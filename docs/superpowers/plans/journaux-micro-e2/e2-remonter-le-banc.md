# Remonter le banc de la recette E2 — la procédure, sans les secrets

Ce que la recette a exigé côté HÔTE, et qui n'existe dans aucun script du dépôt.
**Aucune valeur de secret n'est écrite ici** : elles vivaient dans un répertoire
de travail temporaire, et elles ont disparu avec lui.

## 1. Une plateforme À SOI, et pourquoi pas celle qui tourne déjà

Un autre chantier tenait le port 8080 avec **sa** base. Écrire dedans (créer un
compte, enrôler une VM) aurait été une mutation d'un état partagé, et dépendre
de son processus aurait fait mourir une mesure de dix minutes s'il l'avait
relancé. D'où une plateforme séparée :

```bash
PLATEFORME_HOTE=0.0.0.0 PLATEFORME_PORT=8091 PLATEFORME_BASE=sqlite \
PLATEFORME_BASE_URL=<un fichier à soi> \
PLATEFORME_SECRET_JETON=<au moins la longueur minimale> \
PLATEFORME_ORIGINE_CLIENT='http://192.168.3.1:5173,http://127.0.0.1:5173' \
npx tsx src/index.ts        # depuis plateforme/
```

⚠️ **8081 est pris par `crowdsec` sur cette machine.** Le symptôme est un
`EADDRINUSE` dans le journal, et un service qui ne démarre pas du tout.

## 2. Les trois gestes d'administration, dans cet ordre

```bash
npm run admin:utilisateur -- --email <compte de recette>   # mot de passe sur stdin
npm run admin:agent       -- --vm <nom> --adresse 192.168.3.2
npm run admin:attribuer   -- --email <compte> --vm <nom>
```

L'enrôlement écrit **une seule fois** le secret et le **préfixe de session**.
Les deux servent ensuite.

🔴 **`AGENT_VM` attend l'IDENTIFIANT de la VM, pas son NOM.**
`verifierEnrolement` fait `lireParVm(p, vmId)` sur la clé primaire de
`agent_enrole`. Posé au nom, l'enrôlement est refusé — et le refus est
**volontairement indiscernable** d'un mauvais secret (aucun oracle
d'énumération), donc rien ne dit lequel des deux est en cause. Le journal de la
plateforme, lui, écrit `enrôlement refusé pour la VM <ce qu'on a posé>` : c'est
là qu'on voit qu'on a posé un nom.

⚠️ **Cinq refus consécutifs arment un FREIN** (`retry_apres_s` ≈ 900 s). Il est
en mémoire : relancer le service le vide.

## 3. Le nom de session porte le préfixe

Le rôle `agent` n'est admis que sur une session que son préfixe préfixe
(`identite/garde.ts`, depuis P3) : `SESSION_ID=<préfixe>:<nom local>`, et le
client ouvre `?session=` avec la même valeur.

🔴 **PIÈGE DE SHELL, REPRODUIT ET ISOLÉ.** Ce dépôt tourne sous **zsh**, où
`"$PREFIXE:e2-v1"` applique le **modificateur d'historique `:e`** (extension du
nom de fichier) au lieu de concaténer :

```
zsh : "$P:e2-v1"    -> 2-v1                      <- la valeur est MANGÉE
zsh : "${P}:e2-v1"  -> njM-…-lkUQyT…:e2-v1       <- correct
bash: "$P:e2-v1"    -> njM-…-lkUQyT…:e2-v1       <- bash n'a pas ce modificateur
```

Le symptôme est un `SESSION_ID` tronqué dans `C:\dev\run-agent.ps1`, donc un
agent qui s'enregistre sous un nom que personne n'appelle. **Toujours des
accolades.**

## 4. Le client doit être sur une ORIGINE SÛRE

`getUserMedia` n'existe pas sur `http://192.168.3.1:5173`, qui n'est pas une
origine sûre — E1 employait cette adresse, mais E1 n'appelait jamais
`getUserMedia`. `http://127.0.0.1:5173` en est une. La page déduisant sinon
l'adresse de la plateforme de sa propre origine, il faut lui passer
`?plateforme=` et `?signaling=` (`client/src/adresse-plateforme.ts`).

## 5. L'ordre de lancement, et la corrélation qu'il faut connaître

🔴 **Lancer le pilote DANS LES SECONDES qui suivent l'agent.** Sur quatre
exécutions : les deux où le client est arrivé **~10 s** après l'agent
(`écoulé=11,5 s`) ont donné `state=Connected` **immédiatement** ; les deux où il
est arrivé **~42 à 48 s** après (`écoulé=42,5 s` et `48,1 s`) sont restées en
`Checking` puis `ICE déconnecté` au bout de 18 s. **La corrélation est nette sur
quatre points ; la cause n'est PAS identifiée**, et rien n'exclut une
coïncidence. Elle est nommée parce qu'elle a coûté deux exécutions.

## 6. Tuer l'agent AVANT chaque tentative

`e2-lancer.sh` le fait, et ce n'est pas de la précaution : un agent survivant
tient `C:\dev\agent.log`, le nouveau `StreamWriter` ne peut pas l'ouvrir, et
**l'on relit le journal de la tentative précédente en croyant lire le sien**.
Payé trois fois sur trois en D8, et une fois de plus ici.
