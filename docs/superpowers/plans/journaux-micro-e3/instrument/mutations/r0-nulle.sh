# RED 0: rewrites a comment IDENTICALLY. The file does not change.
# The harness MUST refuse to play.
f="$1"
sed -i 's|^//! Messages du canal de contrôle (fiable, ordonné, faible débit).$|//! Messages du canal de contrôle (fiable, ordonné, faible débit).|' "$f"
