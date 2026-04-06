#!/usr/bin/env bash
# run.sh — check requirements, build, and start the wiki server
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/wiki-server"
ENV_FILE="$SCRIPT_DIR/.env"

# ---------------------------------------------------------------------------
# Minimum required versions
# ---------------------------------------------------------------------------
MIN_NODE=18
MIN_NPM=9
MIN_RUST="1.70"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
BOLD='\033[1m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; GREEN='\033[0;32m'; RESET='\033[0m'

ok()   { echo -e "${GREEN}  [ok]${RESET}  $*"; }
warn() { echo -e "${YELLOW} [warn]${RESET} $*"; }
fail() { echo -e "${RED} [MISSING]${RESET} $*"; }

# Compares semver-like "1.2.3" strings; returns 0 if $1 >= $2
version_gte() {
  printf '%s\n%s\n' "$2" "$1" | sort -V -C
}

ERRORS=0
check_fail() { fail "$1"; ERRORS=$(( ERRORS + 1 )); }

# ---------------------------------------------------------------------------
# Preflight checks
# ---------------------------------------------------------------------------
echo ""
echo -e "${BOLD}==> Checking build requirements…${RESET}"

# --- Rust / Cargo ---
if command -v cargo &>/dev/null; then
  RUST_VER=$(rustc --version 2>/dev/null | awk '{print $2}')
  if version_gte "$RUST_VER" "$MIN_RUST"; then
    ok "Rust $RUST_VER"
  else
    check_fail "Rust $RUST_VER found but $MIN_RUST+ required. Update via: rustup update stable"
  fi
else
  check_fail "Rust not found. Install from https://rustup.rs:
         curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi

# --- Node.js ---
if command -v node &>/dev/null; then
  NODE_VER=$(node --version | sed 's/v//')
  NODE_MAJOR=$(echo "$NODE_VER" | cut -d. -f1)
  if [[ "$NODE_MAJOR" -ge "$MIN_NODE" ]]; then
    ok "Node.js v$NODE_VER"
  else
    check_fail "Node.js v$NODE_VER found but v$MIN_NODE+ required.
         Install via nvm: https://github.com/nvm-sh/nvm
           nvm install --lts
         Or from https://nodejs.org"
  fi
else
  check_fail "Node.js not found (v$MIN_NODE+ required).
         Install via nvm: https://github.com/nvm-sh/nvm
           nvm install --lts
         Or from https://nodejs.org"
fi

# --- npm ---
if command -v npm &>/dev/null; then
  NPM_VER=$(npm --version)
  if version_gte "$NPM_VER" "$MIN_NPM"; then
    ok "npm $NPM_VER"
  else
    check_fail "npm $NPM_VER found but $MIN_NPM+ required. Run: npm install -g npm"
  fi
else
  check_fail "npm not found (should ship with Node.js)."
fi

# --- SQLite dev library (required by sqlx — not a bundled build) ---
SQLITE_OK=false
# pkg-config check
if command -v pkg-config &>/dev/null && pkg-config --exists sqlite3 2>/dev/null; then
  SQLITE_OK=true
  ok "libsqlite3 $(pkg-config --modversion sqlite3 2>/dev/null || echo '(version unknown)')"
else
  # Fallback: look for the header directly
  for inc in /usr/include/sqlite3.h /usr/local/include/sqlite3.h \
             /opt/homebrew/include/sqlite3.h /usr/include/*/sqlite3.h; do
    # shellcheck disable=SC2086
    ls $inc 2>/dev/null | head -1 | grep -q sqlite3 && { SQLITE_OK=true; break; }
  done
  $SQLITE_OK && ok "libsqlite3 (header found)" || true
fi
if [[ $SQLITE_OK == false ]]; then
  check_fail "SQLite development library not found (required by sqlx — no bundled build).
         Install:
           Debian/Ubuntu : sudo apt install libsqlite3-dev pkg-config
           Fedora/RHEL   : sudo dnf install sqlite-devel pkgconf
           macOS (brew)  : brew install sqlite pkg-config
           Arch Linux    : sudo pacman -S sqlite"
fi

# --- pkg-config (needed by the build system to locate libsqlite3) ---
if command -v pkg-config &>/dev/null; then
  ok "pkg-config $(pkg-config --version)"
else
  # On macOS Apple Silicon, pkgconf may be used instead
  if command -v pkgconf &>/dev/null; then
    ok "pkgconf $(pkgconf --version) (aliased as pkg-config)"
  else
    check_fail "pkg-config not found (needed to link libsqlite3).
         Install:
           Debian/Ubuntu : sudo apt install pkg-config
           Fedora/RHEL   : sudo dnf install pkgconf
           macOS (brew)  : brew install pkg-config
           Arch Linux    : sudo pacman -S pkgconf"
  fi
fi

# ---------------------------------------------------------------------------
# Abort if anything is missing
# ---------------------------------------------------------------------------
if [[ $ERRORS -gt 0 ]]; then
  echo ""
  echo -e "${RED}${BOLD}==> $ERRORS requirement(s) not met. Please install them and re-run ./run.sh${RESET}"
  echo ""
  exit 1
fi

echo -e "${GREEN}${BOLD}==> All requirements met.${RESET}"
echo ""

# Load .env if present
if [[ -f "$ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    set +a
fi

# Defaults
DATABASE_URL="${DATABASE_URL:-sqlite:./wiki.db}"
JWT_SECRET="${JWT_SECRET:-change-me-in-production-please}"
PORT="${PORT:-3000}"
RUST_LOG="${RUST_LOG:-wiki_server=info,tower_http=info}"

export DATABASE_URL JWT_SECRET PORT RUST_LOG

# Build frontend
echo "==> Building React frontend..."
cd "$SCRIPT_DIR/frontend"
npm ci --silent
npm run build
cd "$SCRIPT_DIR"

# Build backend
echo "==> Building Rust backend (release)..."
cargo build --release

echo ""
echo "==> Build complete. Starting wiki server on http://0.0.0.0:${PORT}"
echo "    Database: ${DATABASE_URL}"
echo ""

exec "$BINARY"
