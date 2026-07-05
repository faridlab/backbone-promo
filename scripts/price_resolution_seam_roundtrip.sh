#!/usr/bin/env bash
# §5 round-trip: prove the promo seam survives a full codegen regen.
#
# The price-resolution + loyalty seam lives entirely in user-owned custom files
# (promo_events.rs, promo_ports.rs, promo_write_service.rs). Regeneration must
# leave them byte-identical and the CUSTOM re-export block in service/mod.rs
# intact, and the seam tests must stay green afterwards.
set -euo pipefail
cd "$(dirname "$0")/.."

export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5433/backbone_promo}"

SEAM=(
  src/application/service/promo_events.rs
  src/application/service/promo_ports.rs
  src/application/service/promo_write_service.rs
)

before=$(shasum "${SEAM[@]}")
echo "== regenerating (--force) =="
metaphor schema schema generate --force >/dev/null
after=$(shasum "${SEAM[@]}")

if [[ "$before" != "$after" ]]; then
  echo "FAIL: seam files changed across regen"; diff <(echo "$before") <(echo "$after"); exit 1
fi
echo "OK: seam files byte-identical across regen"

echo "== re-running the seam + oracle suite =="
SQLX_OFFLINE=false cargo test --test promo_golden_cases --test integrity_probes \
  --test price_resolution_seam 2>&1 | grep -E "test result"
echo "OK: §5 round-trip holds"
