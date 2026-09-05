#!/usr/bin/env bash
# LE PANNEAU DES VOIES MORTES — à SOURCER en tête d'un script du chemin de
# développement d'avant la bascule appliance.
#
#     . "$(dirname "$0")/voie-morte.sh"
#     voie_morte "ce que ce script faisait" "ce qui le remplace"
#
# 🔴 POURQUOI CE FICHIER EXISTE, ET POURQUOI IL N'EFFACE RIEN.
#
# Le 29 août 2026, le chantier `package-nivuus` a fait de la VM cible une
# APPLIANCE et a retiré — délibérément — `C:\dev`, la chaîne Rust de l'invité
# et le montage CIFS `/media/vm`. Les scripts de ce répertoire qui visent
# cette machine ne peuvent plus rien faire.
#
# **Les SUPPRIMER perdrait la trace de ce qu'ils faisaient et de pourquoi ils
# ont été remplacés.** **Les laisser tels quels est PIRE** : ce sont des
# scripts MORTS QUI ONT L'AIR VIVANTS, et ce dépôt a payé ce patron un nombre
# de fois qu'il documente lui-même — un journal qui n'affichait qu'une
# constante, un témoin condamné au rouge, une assertion d'absence verte sur un
# plantage. Un exemplaire de plus ne mérite pas d'être conservé en silence.
#
# Donc : ils ÉCHOUENT VITE, en NOMMANT LEUR SUCCESSEUR. C'est bon marché,
# réversible, et cela change un piège muet en panneau.
#
# ⚠️ CE N'EST PAS UN RETRAIT. Le corps de chaque script reste dessous, lisible,
# comme relevé historique. Le retrait réel est une DETTE NOMMÉE, à jouer le
# jour où plus rien ne les cite — au 5 septembre 2026, il reste **48**
# appelants exécutables de `scripts/winrm.js`, **57** de `/media/vm` et **2**
# de `scripts/build-agent.sh` dans l'arbre suivi par git (relevé par la
# commande inscrite dans le § « Legs ouverts » de `CLAUDE.md` — la relancer,
# jamais recopier ces trois nombres).
#
# 🔴 CE GUIDE NE DÉCIDE PAS DU SORT DES SCRIPTS : il le rend visible. Les
# retirer, les réécrire vers l'appliance, ou les garder ainsi reste une
# décision du propriétaire du dépôt.

voie_morte() {
    local faisait="$1" successeur="$2"
    cat >&2 <<FIN
🔴 VOIE MORTE : $(basename "${0}")

  Ce script $(printf '%s' "${faisait}").

  Il vise la VM de DÉVELOPPEMENT, qui n'existe plus sous cette forme depuis la
  bascule appliance du 29 août 2026 (chantier package-nivuus) : \`C:\\dev\`, la
  chaîne Rust de l'invité et le montage CIFS \`/media/vm\` ont été retirés
  DÉLIBÉRÉMENT. Relevé le 5 septembre 2026 : \`mount | grep media/vm\` ne rend
  rien, \`/media/vm\` est un répertoire vide, \`Get-ChildItem C:\\\` ne liste
  aucun \`dev\`, et WinRM refuse le transport Basic.

  CE QUI LE REMPLACE :
${successeur}

  Voir CLAUDE.md § « Cycle de vie de la VM Windows », et
  docs/superpowers/plans/2026-09-05-lot3-campagne-vm-resultats-partiels.md § 1.

  ⚠️ Ce script n'est PAS supprimé : son corps reste lisible sous ce garde,
  comme relevé historique. Pour le lire sans l'exécuter : \`cat \$0\`.
FIN
    exit 78   # EX_CONFIG : la configuration du monde ne permet pas ce geste.
}
