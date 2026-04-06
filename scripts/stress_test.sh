#!/usr/bin/env bash
# =============================================================================
# stress_test.sh — HTTP + WebSocket load test for wiki-server
#
# Usage:
#   ./scripts/stress_test.sh [options]
#
# Options:
#   --url        Base URL of the server         (default: http://localhost:3000)
#   --duration   Test duration per endpoint     (default: 30s)
#   --conns      Concurrent connections         (default: 500)
#   --threads    Worker threads for wrk         (default: nproc)
#   --rps        Target request rate (wrk2 only)(default: unlimited)
#   --ws-clients Concurrent WebSocket clients   (default: 200)
#   --skip-ws    Skip WebSocket test
#   --help       Show this help
#
# Required tools (the script checks and installs if missing on Debian/Ubuntu):
#   wrk          — HTTP load generator (sudo apt install wrk / brew install wrk)
#   wrk2         — Optional; enables --rps rate limiting (brew install wrk2)
#   node         — WebSocket test driver (already required to build the frontend)
#   jq           — JSON parser (sudo apt install jq)
#   curl         — HTTP client (pre-installed on most systems)
#
# Environment variables (override via .env or export before running):
#   BASE_URL     same as --url
#   STRESS_USER  username for the test account  (default: stress_test_user)
#   STRESS_PASS  password for the test account  (default: Str3ssT3st!2026)
#
# Examples:
#   # Quick smoke test (10s, 100 connections)
#   DURATION=10s CONNECTIONS=100 ./scripts/stress_test.sh
#
#   # Full blast — tune to your machine's limits
#   ./scripts/stress_test.sh --conns 2000 --threads 16 --duration 60s
#
#   # With rate cap (requires wrk2)
#   ./scripts/stress_test.sh --rps 50000 --duration 60s
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
BASE_URL="${BASE_URL:-http://localhost:3000}"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-500}"
THREADS="${THREADS:-$(nproc 2>/dev/null || sysctl -n hw.logicalcpu 2>/dev/null || echo 4)}"
TARGET_RPS=""
WS_CLIENTS="${WS_CLIENTS:-200}"
SKIP_WS=false

STRESS_USER="${STRESS_USER:-stress_test_user}"
STRESS_PASS="${STRESS_PASS:-Str3ssT3st!2026}"
STRESS_EMAIL="${STRESS_USER}@stress.local"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/../stress-results"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
REPORT="$RESULTS_DIR/report_$TIMESTAMP.txt"

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
RESET='\033[0m'

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
  case $1 in
    --url)        BASE_URL="$2"; shift 2 ;;
    --duration)   DURATION="$2"; shift 2 ;;
    --conns)      CONNECTIONS="$2"; shift 2 ;;
    --threads)    THREADS="$2"; shift 2 ;;
    --rps)        TARGET_RPS="$2"; shift 2 ;;
    --ws-clients) WS_CLIENTS="$2"; shift 2 ;;
    --skip-ws)    SKIP_WS=true; shift ;;
    --help)
      head -40 "$0" | grep '^#' | sed 's/^# \?//'
      exit 0
      ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
log()    { echo -e "${CYAN}[stress]${RESET} $*"; }
ok()     { echo -e "${GREEN}[  OK  ]${RESET} $*"; }
warn()   { echo -e "${YELLOW}[ WARN ]${RESET} $*"; }
fail()   { echo -e "${RED}[ FAIL ]${RESET} $*"; }
header() { echo -e "\n${BOLD}━━━  $*  ━━━${RESET}"; }

tee_report() { tee -a "$REPORT"; }

# ---------------------------------------------------------------------------
# Tool checks
# ---------------------------------------------------------------------------
header "Checking dependencies"

check_tool() {
  local cmd="$1" install_hint="$2"
  if command -v "$cmd" &>/dev/null; then
    ok "$cmd found at $(command -v "$cmd")"
    return 0
  else
    warn "$cmd not found — $install_hint"
    return 1
  fi
}

HAS_WRK=false
HAS_WRK2=false
HAS_NODE=false
command -v node &>/dev/null && HAS_NODE=true

check_tool wrk    "sudo apt install wrk  OR  brew install wrk"    && HAS_WRK=true  || true
check_tool wrk2   "brew install wrk2  (optional, enables --rps)"  && HAS_WRK2=true || true
check_tool node   "https://nodejs.org (already required to build the frontend)" \
  && ok "Node $(node --version) — WebSocket test will use native Node.js WebSocket" \
  || warn "node not found — WebSocket test will be skipped"
check_tool jq     "sudo apt install jq  OR  brew install jq" || { fail "jq is required"; exit 1; }
check_tool curl   "sudo apt install curl" || { fail "curl is required"; exit 1; }

if [[ $HAS_WRK == false && $HAS_WRK2 == false ]]; then
  fail "Neither wrk nor wrk2 found. Install wrk: sudo apt install wrk"
  exit 1
fi

WRK_CMD="wrk"
[[ $HAS_WRK2 == true && -n "$TARGET_RPS" ]] && WRK_CMD="wrk2"

# ---------------------------------------------------------------------------
# Check server is up
# ---------------------------------------------------------------------------
header "Checking server at $BASE_URL"
if ! curl -sf "$BASE_URL" -o /dev/null; then
  fail "Server not reachable at $BASE_URL — start it with ./run.sh"
  exit 1
fi
ok "Server is up"

# ---------------------------------------------------------------------------
# Setup results directory
# ---------------------------------------------------------------------------
mkdir -p "$RESULTS_DIR"
echo "Wiki stress test — $(date)" > "$REPORT"
echo "URL: $BASE_URL | threads: $THREADS | connections: $CONNECTIONS | duration: $DURATION" >> "$REPORT"
echo "==========================================================" >> "$REPORT"

# ---------------------------------------------------------------------------
# Create / login test user
# ---------------------------------------------------------------------------
header "Setting up test user ($STRESS_USER)"

# Try register (ignore error if already exists)
REG_STATUS=$(curl -sf -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/api/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$STRESS_USER\",\"email\":\"$STRESS_EMAIL\",\"password\":\"$STRESS_PASS\"}" || echo "000")
[[ $REG_STATUS == "200" || $REG_STATUS == "409" ]] && ok "User ready (status $REG_STATUS)" || warn "Register returned $REG_STATUS"

# Login
LOGIN_RESP=$(curl -sf -X POST "$BASE_URL/api/auth/login" \
  -H "Content-Type: application/json" \
  -d "{\"identifier\":\"$STRESS_USER\",\"password\":\"$STRESS_PASS\"}")
JWT=$(echo "$LOGIN_RESP" | jq -r '.token')
if [[ -z "$JWT" || "$JWT" == "null" ]]; then
  fail "Could not obtain JWT. Response: $LOGIN_RESP"
  exit 1
fi
ok "JWT obtained (${JWT:0:20}…)"

# Create a test page to use in read benchmarks
PAGE_RESP=$(curl -sf -X POST "$BASE_URL/api/pages" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $JWT" \
  -d '{"title":"Stress Test Page","content":"# Benchmark\n\nThis page is used by the stress test script."}')
PAGE_ID=$(echo "$PAGE_RESP" | jq -r '.id')
if [[ -z "$PAGE_ID" || "$PAGE_ID" == "null" ]]; then
  warn "Could not create test page (may already exist). Trying to fetch one."
  PAGE_ID=$(curl -sf "$BASE_URL/api/pages" \
    -H "Authorization: Bearer $JWT" | jq -r '.[0].id // empty')
fi
if [[ -n "$PAGE_ID" && "$PAGE_ID" != "null" ]]; then
  ok "Test page ID: $PAGE_ID"
else
  warn "No page ID available — page-specific benchmarks will be skipped"
fi

# ---------------------------------------------------------------------------
# Lua script for wrk (injects Authorization header)
# ---------------------------------------------------------------------------
LUA_AUTH=$(mktemp /tmp/wrk_auth_XXXX.lua)
cat > "$LUA_AUTH" <<EOF
wrk.headers["Authorization"] = "Bearer $JWT"
wrk.headers["Content-Type"]  = "application/json"

-- Track per-thread latency histogram
done = function(summary, latency, requests)
  io.write("-----\n")
  io.write(string.format("  Avg latency : %.2f ms\n", latency.mean / 1000))
  io.write(string.format("  Stdev       : %.2f ms\n", latency.stdev / 1000))
  io.write(string.format("  p50         : %.2f ms\n", latency:percentile(50) / 1000))
  io.write(string.format("  p90         : %.2f ms\n", latency:percentile(90) / 1000))
  io.write(string.format("  p95         : %.2f ms\n", latency:percentile(95) / 1000))
  io.write(string.format("  p99         : %.2f ms\n", latency:percentile(99) / 1000))
  io.write(string.format("  p99.9       : %.2f ms\n", latency:percentile(99.9) / 1000))
  io.write(string.format("  Max latency : %.2f ms\n", latency.max / 1000))
  io.write(string.format("  HTTP errors : %d\n", summary.errors.status))
  io.write(string.format("  Timeouts    : %d\n", summary.errors.timeout))
end
EOF

# Lua script for POST (search)
LUA_SEARCH=$(mktemp /tmp/wrk_search_XXXX.lua)
cat > "$LUA_SEARCH" <<EOF
wrk.method  = "GET"
wrk.headers["Authorization"] = "Bearer $JWT"
wrk.path    = "/api/search?q=stress"

done = function(summary, latency, requests)
  io.write("-----\n")
  io.write(string.format("  Avg latency : %.2f ms\n", latency.mean / 1000))
  io.write(string.format("  p50         : %.2f ms\n", latency:percentile(50) / 1000))
  io.write(string.format("  p95         : %.2f ms\n", latency:percentile(95) / 1000))
  io.write(string.format("  p99         : %.2f ms\n", latency:percentile(99) / 1000))
  io.write(string.format("  Max latency : %.2f ms\n", latency.max / 1000))
  io.write(string.format("  HTTP errors : %d\n", summary.errors.status))
end
EOF

cleanup() {
  rm -f "$LUA_AUTH" "$LUA_SEARCH"
}
trap cleanup EXIT

# ---------------------------------------------------------------------------
# Helper: run one wrk benchmark
# ---------------------------------------------------------------------------
run_wrk() {
  local label="$1" url="$2" lua="$3"
  header "$label"
  echo "" | tee_report
  echo "=== $label ===" >> "$REPORT"
  echo "  URL: $url" | tee_report

  local wrk_args="-t$THREADS -c$CONNECTIONS -d$DURATION --latency -s $lua"
  [[ $WRK_CMD == "wrk2" && -n "$TARGET_RPS" ]] && wrk_args="$wrk_args -R$TARGET_RPS"

  $WRK_CMD $wrk_args "$url" 2>&1 | tee_report
  echo "" | tee_report
}

# ---------------------------------------------------------------------------
# Benchmarks
# ---------------------------------------------------------------------------

# 1. Auth — GET /api/auth/me  (JWT validation on every request)
run_wrk "Benchmark 1 — Auth: GET /api/auth/me (JWT overhead)" \
  "$BASE_URL/api/auth/me" "$LUA_AUTH"

# 2. Pages list — GET /api/pages  (DB read + tree build)
run_wrk "Benchmark 2 — Pages: GET /api/pages (DB read, tree build)" \
  "$BASE_URL/api/pages" "$LUA_AUTH"

# 3. Single page read
if [[ -n "$PAGE_ID" && "$PAGE_ID" != "null" ]]; then
  run_wrk "Benchmark 3 — Pages: GET /api/pages/$PAGE_ID (single row fetch)" \
    "$BASE_URL/api/pages/$PAGE_ID" "$LUA_AUTH"
fi

# 4. Full-text search — FTS5 query
run_wrk "Benchmark 4 — Search: GET /api/search?q=stress (FTS5)" \
  "$BASE_URL/api/search?q=stress" "$LUA_AUTH"

# 5. Shared-with-me
run_wrk "Benchmark 5 — Shared: GET /api/pages/shared-with-me (join query)" \
  "$BASE_URL/api/pages/shared-with-me" "$LUA_AUTH"

# ---------------------------------------------------------------------------
# WebSocket concurrency test — uses Node.js native WebSocket (Node 21+)
# No external tools required; node is already needed to build the frontend.
# ---------------------------------------------------------------------------
if [[ $SKIP_WS == false && $HAS_NODE == true && -n "$PAGE_ID" && "$PAGE_ID" != "null" ]]; then
  header "Benchmark 6 — WebSocket: $WS_CLIENTS concurrent clients"
  echo ""
  echo "=== WebSocket: $WS_CLIENTS concurrent clients ===" >> "$REPORT"

  WS_URL="${BASE_URL/http/ws}/ws/pages/$PAGE_ID?token=$JWT"
  log "Connecting $WS_CLIENTS clients to $WS_URL (each sends 10 messages then closes)…"

  # Write the Node.js driver to a temp file
  NODE_WS_SCRIPT=$(mktemp /tmp/ws_stress_XXXX.mjs)
  # Update cleanup to also remove the Node script
  cleanup() { rm -f "$LUA_AUTH" "$LUA_SEARCH" "$NODE_WS_SCRIPT"; }

  cat > "$NODE_WS_SCRIPT" <<'NODEJS'
import { performance } from 'perf_hooks';

const [,, wsUrl, clientsStr, msgsPerClientStr] = process.argv;
const CLIENTS       = parseInt(clientsStr,       10);
const MSGS_EACH     = parseInt(msgsPerClientStr, 10);
const latencies     = [];
let connected = 0, errors = 0;
const start = performance.now();

function runClient(id) {
  return new Promise(resolve => {
    const ws = new WebSocket(wsUrl);
    const tOpen = performance.now();
    let sent = 0;

    ws.addEventListener('open', () => {
      connected++;
      const send = () => {
        if (sent >= MSGS_EACH) { ws.close(); return; }
        const payload = sent % 2 === 0
          ? JSON.stringify({ content: `stress client ${id} msg ${sent}`, cursor_pos: sent })
          : JSON.stringify({ cursor_pos: sent });
        ws.send(payload);
        sent++;
        setImmediate(send);
      };
      send();
    });

    ws.addEventListener('close', () => {
      latencies.push(performance.now() - tOpen);
      resolve();
    });

    ws.addEventListener('error', () => { errors++; resolve(); });
  });
}

await Promise.all(Array.from({ length: CLIENTS }, (_, i) => runClient(i)));

const elapsed    = performance.now() - start;
const totalMsgs  = CLIENTS * MSGS_EACH;
const throughput = Math.round(totalMsgs / (elapsed / 1000));
const avg        = latencies.reduce((a, b) => a + b, 0) / (latencies.length || 1);
const sorted     = latencies.slice().sort((a, b) => a - b);
const p          = pct => sorted[Math.max(0, Math.floor(sorted.length * pct / 100) - 1)] ?? 0;

console.log(JSON.stringify({
  clients: CLIENTS, msgs_per_client: MSGS_EACH, total_msgs: totalMsgs,
  connected, errors,
  elapsed_ms: Math.round(elapsed),
  throughput_msgs_per_sec: throughput,
  latency_ms: {
    avg:  avg.toFixed(2),
    p50:  p(50).toFixed(2),
    p90:  p(90).toFixed(2),
    p95:  p(95).toFixed(2),
    p99:  p(99).toFixed(2),
    max:  sorted[sorted.length - 1]?.toFixed(2) ?? '0'
  }
}));
NODEJS

  WS_RESULT=$(node "$NODE_WS_SCRIPT" "$WS_URL" "$WS_CLIENTS" 10 2>&1)
  if echo "$WS_RESULT" | jq . &>/dev/null; then
    {
      echo "  Clients           : $(echo "$WS_RESULT" | jq -r '.clients')"
      echo "  Connected         : $(echo "$WS_RESULT" | jq -r '.connected')"
      echo "  Errors            : $(echo "$WS_RESULT" | jq -r '.errors')"
      echo "  Total messages    : $(echo "$WS_RESULT" | jq -r '.total_msgs')"
      echo "  Elapsed           : $(echo "$WS_RESULT" | jq -r '.elapsed_ms') ms"
      echo "  Throughput        : ~$(echo "$WS_RESULT" | jq -r '.throughput_msgs_per_sec') msgs/s"
      echo "  Conn latency avg  : $(echo "$WS_RESULT" | jq -r '.latency_ms.avg') ms"
      echo "  Conn latency p50  : $(echo "$WS_RESULT" | jq -r '.latency_ms.p50') ms"
      echo "  Conn latency p90  : $(echo "$WS_RESULT" | jq -r '.latency_ms.p90') ms"
      echo "  Conn latency p95  : $(echo "$WS_RESULT" | jq -r '.latency_ms.p95') ms"
      echo "  Conn latency p99  : $(echo "$WS_RESULT" | jq -r '.latency_ms.p99') ms"
      echo "  Conn latency max  : $(echo "$WS_RESULT" | jq -r '.latency_ms.max') ms"
    } | tee_report
  else
    fail "WebSocket test error: $WS_RESULT" | tee_report
  fi
  echo "" | tee_report

elif [[ $SKIP_WS == false && $HAS_NODE == false ]]; then
  warn "Skipping WebSocket test — node not found (install Node.js 21+)"
elif [[ $SKIP_WS == false && ( -z "$PAGE_ID" || "$PAGE_ID" == "null" ) ]]; then
  warn "Skipping WebSocket test — no page ID available"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
header "Results summary"

echo ""
echo -e "${BOLD}Full report saved to:${RESET} $REPORT"
echo ""

# Print system info alongside results
{
  echo ""
  echo "=========================================="
  echo "System info"
  echo "=========================================="
  echo "  OS      : $(uname -sr)"
  echo "  CPUs    : $(nproc 2>/dev/null || sysctl -n hw.logicalcpu)"
  echo "  RAM     : $(free -h 2>/dev/null | awk '/Mem:/{print $2}' || sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.1f GB\n",$1/1073741824}')"
  echo "  wrk     : $($WRK_CMD --version 2>&1 | head -1)"
  echo ""
  echo "Test parameters"
  echo "  Threads     : $THREADS"
  echo "  Connections : $CONNECTIONS"
  echo "  Duration    : $DURATION"
  [[ -n "$TARGET_RPS" ]] && echo "  Target RPS  : $TARGET_RPS (wrk2)"
  echo "  WS clients  : $WS_CLIENTS"
} | tee_report

echo ""
log "Done. To compare runs:"
echo "       ls -lh $RESULTS_DIR/"
