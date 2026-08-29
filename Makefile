# Nivuus desk package — test target.
#
# test          Découvre et joue chaque suite tests/test_desk_*.py.
# help          Liste les cibles disponibles.
#
# 🔴 CETTE CIBLE N'ÉPROUVE PAS LE PRODUIT — l'agent Rust, la plateforme, le
# client, le protocole partagé — ELLE ÉPROUVE LE PACKAGING : le manifeste et
# le wizard (nivuus-package.yaml, wizard.yaml), les trois hooks
# (resolve/install/activate) et le script qui compile puis dépose
# `agent.exe` (scripts/build-agent-croise.sh). Le produit se recette par
# `env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh`, JAMAIS par
# `make test` — voir README-package.md, qui dit lequel joue quoi.

DESK_DIR := $(CURDIR)

.PHONY: test help

help:
	@grep -E '^[a-zA-Z_-]+:.*' $(MAKEFILE_LIST) | sed 's/:.*//' | sort

# Règle de découverte, ÉNONCÉE plutôt que recopiée en liste à la main : toute
# SUITE de tests/ porte le préfixe `test_`, et rien d'autre dans ce
# répertoire ne le porte. `tests/desk_activate_fixtures.py` — le module de
# fixtures partagées extrait à la tâche 6 — s'appelle délibérément SANS ce
# préfixe précisément pour ne pas matcher ce motif ; son propre en-tête le
# dit (« Ce fichier ne s'appelle PAS `test_*.py` : il n'est pas une suite »).
# Une suite ajoutée demain n'a donc RIEN à câbler ici : il suffit qu'elle
# s'appelle `test_desk_*.py` pour être découverte et jouée, dans l'ordre
# alphabétique (`sort` sur le résultat de `wildcard`, qui ne garantit aucun
# ordre par lui-même).
PYTHON ?= python3

test:
	@for t in $(sort $(wildcard $(DESK_DIR)/tests/test_*.py)); do \
	    echo "--- $$(basename $$t .py)"; \
	    $(PYTHON) $$t || exit 1; \
	done
