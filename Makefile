# Nivuus desk package — test target.
#
# test          Discovers and runs each tests/test_desk_*.py suite.
# help          Lists the available targets.
#
# 🔴 THIS TARGET DOES NOT TEST THE PRODUCT — the Rust agent, the platform, the
# client, the shared protocol — IT TESTS THE PACKAGING: the manifest and
# the wizard (nivuus-package.yaml, wizard.yaml), the three hooks
# (resolve/install/activate) and the script that builds then drops
# `agent.exe` (scripts/build-agent-croise.sh). The product is accepted through
# `env -u TURN_URL -u TURN_SECRET ./scripts/verify-all.sh`, NEVER through
# `make test` — see README-package.md, which says which one runs what.

DESK_DIR := $(CURDIR)

.PHONY: test help

help:
	@grep -E '^[a-zA-Z_-]+:.*' $(MAKEFILE_LIST) | sed 's/:.*//' | sort

# Discovery rule, STATED rather than copied by hand as a list: every
# SUITE of tests/ carries the `test_` prefix, and nothing else in this
# directory carries it. `tests/desk_activate_fixtures.py` — the shared
# fixtures module extracted in task 6 — is deliberately named WITHOUT this
# prefix precisely so as not to match this pattern; its own header
# says so ("This file is NOT named `test_*.py`: it is not a suite").
# A suite added tomorrow therefore has NOTHING to wire here: it only needs to
# be named `test_desk_*.py` to be discovered and run, in
# alphabetical order (`sort` on the result of `wildcard`, which guarantees no
# order by itself).
PYTHON ?= python3

test:
	@for t in $(sort $(wildcard $(DESK_DIR)/tests/test_*.py)); do \
	    echo "--- $$(basename $$t .py)"; \
	    $(PYTHON) $$t || exit 1; \
	done
