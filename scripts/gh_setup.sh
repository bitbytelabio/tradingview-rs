#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# GitHub Project Setup — tradingview-rs
#
# Creates milestones, labels, and initial issues for the project roadmap.
# Requires: gh CLI authenticated with repo scope on bitbytelabio/tradingview-rs
#
# Usage:  bash scripts/gh_setup.sh
# ---------------------------------------------------------------------------
set -euo pipefail

REPO="bitbytelabio/tradingview-rs"

# ---- ANSI helpers ----
green()  { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }
red()    { printf '\033[31m%s\033[0m\n' "$*"; }

# ---------------------------------------------------------------------------
# 1. Delete existing labels (optional — skip if you want to keep old ones)
# ---------------------------------------------------------------------------
echo ""
yellow "==> Purging default labels..."
gh label list -R "$REPO" --json name -q '.[].name' | while read -r label; do
    gh label delete "$label" -R "$REPO" --confirm 2>/dev/null || true
done
green "   Done."

# ---------------------------------------------------------------------------
# 2. Create custom labels
# ---------------------------------------------------------------------------
echo ""
yellow "==> Creating labels..."

declare -A LABELS=(
    ["bug"]="d73a4a;Bug report or defect"
    ["enhancement"]="a2eeef;New feature or improvement"
    ["documentation"]="0075ca;Docs, README, examples, doc comments"
    ["performance"]="5319e7;Optimization work"
    ["testing"]="0e8a16;Tests, fuzzing, benchmarks"
    ["ci/cd"]="f9d0c4;CI pipeline, releases, automation"
    ["breaking"]="b60205;Breaking API change"
    ["dependencies"]="0366d6;Dependency updates"
    ["good first issue"]="7057ff;Good for newcomers"
    ["help wanted"]="008672;Extra attention needed"
    ["priority:critical"]="b60205;Must ship in current milestone"
    ["priority:high"]="d93f0b;Should ship in current milestone"
    ["priority:medium"]="fbca04;Nice to have"
    ["priority:low"]="c5def5;Backlog / maybe later"
    ["v0.2.0"]="c2e0c6;Target v0.2.0"
    ["v0.3.0"]="c2e0c6;Target v0.3.0"
    ["v0.4.0"]="c2e0c6;Target v0.4.0"
    ["v0.5.0"]="c2e0c6;Target v0.5.0"
    ["v1.0.0"]="d4c5f9;Target v1.0.0"
    ["wontfix"]="ffffff;Will not fix"
    ["duplicate"]="cfd3d7;Duplicate of another issue"
)

for label in "${!LABELS[@]}"; do
    IFS=';' read -r color desc <<< "${LABELS[$label]}"
    if gh label list -R "$REPO" --search "$label" --json name -q '.[].name' | grep -qx "$label"; then
        echo "   Skip (exists): $label"
    else
        gh label create "$label" -R "$REPO" --color "$color" --description "$desc"
        echo "   Created: $label"
    fi
done
green "   Done."

# ---------------------------------------------------------------------------
# 3. Create milestones
# ---------------------------------------------------------------------------
echo ""
yellow "==> Creating milestones..."

declare -A MILESTONES=(
    ["v0.2.0 – Pipeline & Polish"]="Event-driven pipeline, doc overhaul, bug fixes, publish to crates.io"
    ["v0.3.0 – Data Coverage"]="Fundamental data, economic calendar, screener integration"
    ["v0.4.0 – Advanced Features"]="TA signals, invite-only indicators, multi-timeframe analysis"
    ["v0.5.0 – Performance & Observability"]="Zero-copy deser, tracing, Prometheus metrics, benchmarks, fuzzing"
    ["v1.0.0 – Stable API"]="API freeze, semver, integration tests, security audit, release"
)

for title in "${!MILESTONES[@]}"; do
    desc="${MILESTONES[$title]}"
    if gh api "repos/$REPO/milestones" --jq '.[].title' | grep -qxF "$title"; then
        echo "   Skip (exists): $title"
    else
        gh api "repos/$REPO/milestones" \
            -X POST \
            -f title="$title" \
            -f description="$desc" \
            -f state=open >/dev/null
        echo "   Created: $title"
    fi
done
green "   Done."

# ---------------------------------------------------------------------------
# 4. Create initial issues for v0.2.0 (current milestone)
# ---------------------------------------------------------------------------
echo ""
yellow "==> Creating v0.2.0 issues..."

# Fetch the v0.2.0 milestone number
MILESTONE_NUM=$(gh api "repos/$REPO/milestones" --jq '.[] | select(.title | startswith("v0.2.0")) | .number')
echo "   v0.2.0 milestone number: $MILESTONE_NUM"

create_issue() {
    local title="$1" body="$2" labels="$3"
    if gh issue list -R "$REPO" --search "$title" --state open --json title -q '.[].title' | grep -qxF "$title"; then
        echo "   Skip (exists): $title"
    else
        gh issue create -R "$REPO" \
            --title "$title" \
            --body "$body" \
            --label "$labels" \
            --milestone "$MILESTONE_NUM"
        echo "   Created: $title"
    fi
}

create_issue \
    "Fix parse_packet / SocketMessage round-trip test failures" \
    "7 tests in \`utils::tests\` fail due to \`SocketMessage\` \`serde(untagged)\` enum deserialization mismatches.

**Expected behavior:** \`SocketMessage::Other(Value)\` for unrecognized message shapes.
**Actual behavior:** Falls through to \`SocketMessage::SocketMessage(SocketMessageDe{...})\`.

**Files:** \`src/utils.rs\`, \`src/live/models.rs\`" \
    "bug,priority:critical,v0.2.0"

create_issue \
    "Bounded channels with backpressure in all sink/loader paths" \
    "Replace unbounded \`mpsc::unbounded_channel()\` calls with bounded channels in:

- \`DataLoader\` source → fan-out channel
- Per-sink fan-out channels
- \`WebSocketClient\` internal write path

Add configurable channel capacity to \`LoaderConfig\` and \`WebSocketClient\` builder." \
    "enhancement,priority:high,v0.2.0"

create_issue \
    "Graceful shutdown with in-flight batch draining" \
    "When \`CancellationToken\` fires:

1. Stop accepting new data from source
2. Drain remaining in-flight batches through all sinks
3. Call \`sink.shutdown()\` to flush buffers
4. Return aggregated results

Current behavior: cancellation drops in-flight batches." \
    "enhancement,priority:high,v0.2.0"

create_issue \
    "Rate-limit-aware request throttling in HistoricalClient" \
    "Add configurable rate limiting to \`HistoricalClient::retrieve_batch\`:

- Per-symbol delay between requests
- Token-bucket or leaky-bucket algorithm
- Configurable via \`BatchConfig\`
- Respect TradingView's rate limits to avoid bans" \
    "enhancement,priority:high,v0.2.0"

create_issue \
    "Add cargo doc lint to CI pipeline" \
    "Add a CI step that runs \`cargo doc --no-deps -D warnings\` to block PRs with broken intra-doc links.

**File:** \`.github/workflows/ci.yml\`" \
    "ci/cd,priority:medium,v0.2.0"

create_issue \
    "Publish v0.2.0 to crates.io" \
    "- [ ] Update version in \`Cargo.toml\` to 0.2.0
- [ ] Update \`CHANGELOG.md\` with all v0.2.0 changes
- [ ] Run full test suite (\`cargo test\`)
- [ ] Verify \`cargo doc --no-deps\` is clean
- [ ] \`cargo publish --dry-run\` and review
- [ ] \`cargo publish\`
- [ ] Tag and create GitHub Release" \
    "ci/cd,priority:critical,v0.2.0"

green "   Done."

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
green "======================================================================"
green " Setup complete! Summary:"
green "======================================================================"
echo ""
echo "  Labels:     $(gh label list -R "$REPO" --limit 100 | wc -l | tr -d ' ') created"
echo "  Milestones: $(gh api "repos/$REPO/milestones" --jq 'length') created"
echo "  Issues:     $(gh issue list -R "$REPO" --state open --limit 50 | wc -l | tr -d ' ') total open"
echo ""
echo "  View milestones:  gh api repos/$REPO/milestones --jq '.[].title'"
echo "  View open issues: gh issue list -R $REPO"
echo ""
