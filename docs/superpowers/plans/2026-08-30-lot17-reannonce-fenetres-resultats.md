# Lot 17 — un pair qui arrive tard voit les fenêtres (30 août 2026)

> ⚠️ **Document VERSIONNÉ à dessein.** Le rapport de tâche vit sous
> `.superpowers/`, qui est **gitignoré** : ce dépôt a déjà perdu six constats
> de revue de cette façon. Ce qui suit est ce qui doit survivre.

## Le défaut

Capture réseau du propriétaire (`tcpdump` sur `vnet30` + `tshark`) :
`fenetre-ouverte` × 3 à t = 12,0 s, `refus` × 3 à t = 43,1 s, motif
« la page-shell n'a jamais répondu après la relance ». L'utilisateur, retenu
par l'authentification du proxy, arrive devant un bureau vide.

🔴 **La lecture naturelle — « la borne de 30 s est trop courte » — est
FAUSSE.** Même sans borne, les annonces de t = 12 s étaient déjà perdues :
`relais.ts` relaie par `send(peer, message)`, et sur un `peer` absent c'est un
**no-op silencieux**. La borne ne fait que rendre la perte visible.

## La correction

**L'agent RÉANNONCE quand un pair `client` rejoint la session de contrôle.**
Le relais savait dire « ton pair est parti » (`peer-gone`) et ne savait pas
dire « ton pair est arrivé » : ce lot ajoute la moitié manquante d'un
mécanisme existant (`plateforme/src/signaling/pair-present.ts`), jamais un
troisième. Un tampon côté plateforme aurait été une **copie** d'une vérité qui
vit dans l'agent, que rien dans ce service ne saurait expirer.

**La borne des 30 s est INTACTE** : elle court désormais depuis l'arrivée
d'une page-shell — l'instant que sa propre documentation prétend mesurer — et
non depuis le démarrage du superviseur. Une shell présente mais muette perd
toujours sa fenêtre, et sa sortie virtuelle repart au pilote.

Commit `6a7c98e`.

## Ce qui a été mesuré sur la VM

Pilote : `journaux-lot17/instrument/pilote-pair-tardif.mjs`, relevés à côté.

| Bras | Binaire | Attente | `fenetre-ouverte` |
| --- | --- | --- | --- |
| ROUGE | l'ancien (MSVC, 25 août) | 75 s | **0** |
| VERT 1 | le neuf | 90 s | **4** (identifiants NEUFS : chemin « énumération ») |
| VERT 2 | le neuf | 22 s | **5** (identifiants D'ORIGINE `w-1…w-5` : chemin « réannonce ») |

Le zéro du bras rouge est discriminant par **trois** pièces prises dans la
même exécution : une fenêtre ouverte pendant que le pilote est connecté est
annoncée **immédiatement** ; son identifiant est `w-4`, donc trois fenêtres
avaient déjà été annoncées dans le vide ; et elle est refusée 26 s plus tard
avec le motif exact du défaut.

**La remise à zéro de l'horloge est mesurée** : au bras VERT 2, annonces à
01:07:51,26 et refus à 01:08:22,08 — **30,8 s**, alors que l'agent tournait
depuis 53 s.

## 🔴 Legs ouverts que ce lot laisse

1. **LA CONNEXION DE CONTRÔLE DU SUPERVISEUR NE SE RECONNECTE JAMAIS.** Défaut
   **distinct**, trouvé en mesurant celui-ci, **non corrigé**. Après un
   redémarrage du service `desk-plateforme`, le superviseur journalise
   `émission vers la shell échouée erreur=Trying to work with closed connection`
   et **reste ainsi** ; le pont fichiers, lui, se relance
   (`boucle/surveillance_pont.rs`), le superviseur non
   (`superviseur/signalisation.rs` ne fait que journaliser « connexion de
   contrôle au signaling perdue »). **Tant que ce socket est mort,
   `pair-present` ne peut pas être délivré et la correction de ce lot est
   inopérante** ; le seul remède est de relancer l'agent. Ce n'est pas ce que
   le propriétaire a vécu — chez lui les annonces partaient bien à t = 12 s —
   mais c'est une panne muette d'exploitation.
2. **UNE FENÊTRE `Vivante` N'EST PAS REDITE**, à dessein : l'enfant consomme
   **une** offre et ne renégocie jamais (`agent/src/demarrage.rs`), donc la
   page rouverte enverrait une offre que personne ne prendrait. **Un
   rechargement de la page-shell ne récupère donc pas les fenêtres déjà
   vivantes.** La lever suppose de rendre l'enfant renégociable.
3. **LE BINAIRE DE PRODUCTION A CHANGÉ DE CHAÎNE D'OUTILS** : MSVC → mingw
   croisé. Il démarre, tient ses trois processus en session 1, crée sa sortie
   virtuelle, ouvre sa duplication DXGI et s'attache au capteur — ce qu'aucun
   lot n'avait établi (`build-agent-croise.sh` : « il se lie ; il n'a jamais
   tourné ») — mais **il n'a jamais encodé ni diffusé**, faute de pair WebRTC.
   Sauvegarde en place sur la VM :
   `C:\nivuus\agent\agent.exe.sauvegarde-lot17`.
4. **Aucun média n'a traversé, et le juge « un humain ouvre la page et voit ses
   fenêtres » n'a pas été joué** : l'OAuth Google exige un humain (blocage ②
   du critère ⑦ d'`auth-pomerium`, toujours ouvert).
