#!/usr/bin/env bash
# Add a GitHub Actions self-hosted runner to this machine.
#
# Usage (interactive):
#   bash <(curl -fsSL https://raw.githubusercontent.com/unix-relay/relay/main/install-runner.sh)
#
# Usage (non-interactive, all flags):
#   bash <(curl -fsSL …/install-runner.sh) \
#     --repo   unix-relay/relay  \
#     --token  <REGISTRATION_TOKEN> \
#     --name   my-runner           \
#     --labels self-hosted,linux   \
#     --work   /opt/runner/_work   \
#     --user   github-runner
#
# Flags:
#   --repo   <owner/repo>    GitHub repository  (prompted if omitted)
#   --token  <token>         Runner registration token from
#                            GitHub → Repo → Settings → Actions → Runners → New
#                            (prompted if omitted)
#   --name   <name>          Runner name (default: hostname-N, auto-incremented)
#   --labels <a,b,c>         Comma-separated extra labels
#   --work   <dir>           Work directory (default: <install-dir>/_work)
#   --dir    <dir>           Install directory (default: /opt/actions-runner-N, auto-incremented)
#   --user   <user>          OS user to run the runner as (default: github-runner, created if missing)
#   --no-start               Install but do not enable/start the service
#   --uninstall              Stop and remove the runner service + files

set -euo pipefail

RUNNER_REPO=""
RUNNER_TOKEN=""
RUNNER_NAME=""
RUNNER_LABELS=""
RUNNER_WORK=""
RUNNER_DIR=""
RUNNER_USER=""
START=1
UNINSTALL=0

# ── auto-increment index ─────────────────────────────────────────────────────
_next_index() {
  local i=1
  while [[ -d "/opt/actions-runner-${i}" ]]; do (( i++ )); done
  echo "$i"
}
_IDX="$(_next_index)"
_DEFAULT_DIR="/opt/actions-runner-${_IDX}"
_DEFAULT_NAME="$(hostname -s)-${_IDX}"

log()    { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()   { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
die()    { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; exit 1; }
prompt() {
  local var="$1" msg="$2" def="${3:-}"
  if [[ -n "${!var:-}" ]]; then return; fi
  local display_default=""
  [[ -n "$def" ]] && display_default=" [${def}]"
  printf '\033[1;36m???\033[0m %s%s: ' "$msg" "$display_default" >&2
  local input
  IFS= read -r input </dev/tty
  if [[ -z "$input" && -n "$def" ]]; then
    printf -v "$var" '%s' "$def"
  else
    printf -v "$var" '%s' "$input"
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)       RUNNER_REPO="$2";   shift 2 ;;
    --token)      RUNNER_TOKEN="$2";  shift 2 ;;
    --name)       RUNNER_NAME="$2";   shift 2 ;;
    --labels)     RUNNER_LABELS="$2"; shift 2 ;;
    --work)       RUNNER_WORK="$2";   shift 2 ;;
    --dir)        RUNNER_DIR="$2";    shift 2 ;;
    --user)       RUNNER_USER="$2";   shift 2 ;;
    --no-start)   START=0;            shift   ;;
    --uninstall)  UNINSTALL=1;        shift   ;;
    -h|--help)    sed -n '2,32p' "$0"; exit 0 ;;
    *) die "unknown flag: $1" ;;
  esac
done

# ── self-elevate ────────────────────────────────────────────────────────────
[[ $EUID -eq 0 ]] || {
  tmp="$(mktemp /tmp/install-runner.XXXXXX.sh)"
  case "$0" in
    /dev/fd/*) cat "$0" > "$tmp" ;;
    *) cp "$0" "$tmp" ;;
  esac
  chmod +x "$tmp"
  exec sudo bash "$tmp" "$@"
}
case "$0" in /tmp/install-runner.*.sh) trap 'rm -f "$0"' EXIT ;; esac

# ── resolve runner user ──────────────────────────────────────────────────────
if [[ -z "$RUNNER_USER" ]]; then
  RUNNER_USER="${SUDO_USER:-}"
  if [[ -z "$RUNNER_USER" || "$RUNNER_USER" == "root" ]]; then
    RUNNER_USER="github-runner"
  fi
fi
[[ "$RUNNER_USER" == "root" ]] && die "--user cannot be root; config.sh refuses to run as root"

# ── uninstall ────────────────────────────────────────────────────────────────
if [[ "$UNINSTALL" -eq 1 ]]; then
  [[ -n "$RUNNER_DIR" ]] || die "please specify --dir for uninstall"
  [[ -d "$RUNNER_DIR" ]] || die "runner directory $RUNNER_DIR not found"
  cd "$RUNNER_DIR"
  log "stopping and removing runner service"
  ./svc.sh stop   2>/dev/null || true
  ./svc.sh uninstall 2>/dev/null || true
  log "removing runner registration from GitHub"
  if [[ -n "${RUNNER_TOKEN}" ]]; then
    sudo -u "$RUNNER_USER" ./config.sh remove --token "$RUNNER_TOKEN" 2>/dev/null || true
  else
    warn "no --token provided; skipping GitHub-side runner removal"
    warn "remove it manually at: https://github.com/<owner>/<repo>/settings/actions/runners"
  fi
  PURGE=0
  if { [[ -t 0 ]] || [[ -e /dev/tty ]]; }; then
    [[ -t 0 ]] || exec </dev/tty
    echo
    read -r -p "Delete $RUNNER_DIR entirely? [y/N] " ans </dev/tty
    case "${ans:-N}" in [Yy]*) PURGE=1 ;; esac
  fi
  [[ "$PURGE" -eq 1 ]] && rm -rf "$RUNNER_DIR" && log "removed $RUNNER_DIR"
  exit 0
fi

# ── interactive prompts ──────────────────────────────────────────────────────
_parse_config_cmd() {
  local input="$1" url token
  url="$(echo "$input" | grep -oE -- '--url +[^ ]+' | awk '{print $2}')"
  token="$(echo "$input" | grep -oE -- '--token +[^ ]+' | awk '{print $2}')"
  if [[ -n "$url" && -n "$token" ]]; then
    [[ -z "$RUNNER_REPO"  ]] && RUNNER_REPO="${url#https://github.com/}"
    [[ -z "$RUNNER_TOKEN" ]] && RUNNER_TOKEN="$token"
    return 0
  fi
  return 1
}

if [[ -z "$RUNNER_TOKEN" ]]; then
  printf '\033[1;36m???\033[0m Paste the GitHub runner registration command or token: ' >&2
  _raw=""
  IFS= read -r _raw </dev/tty
  if ! _parse_config_cmd "$_raw"; then
    RUNNER_TOKEN="$_raw"
  fi
fi

prompt RUNNER_REPO   "GitHub repository (owner/repo)"
prompt RUNNER_NAME   "Runner name" "$_DEFAULT_NAME"
prompt RUNNER_LABELS "Extra labels (comma-separated, leave blank for none)" ""
prompt RUNNER_DIR    "Install directory" "$_DEFAULT_DIR"

[[ -n "$RUNNER_REPO"  ]] || die "repository is required"
[[ -n "$RUNNER_TOKEN" ]] || die "registration token is required"
[[ -z "$RUNNER_WORK"  ]] && RUNNER_WORK="${RUNNER_DIR}/_work"

RUNNER_URL="https://github.com/${RUNNER_REPO}"

# ── detect platform ──────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS-$ARCH" in
  Linux-x86_64)              PLATFORM="linux-x64"   ;;
  Linux-aarch64|Linux-arm64) PLATFORM="linux-arm64" ;;
  Darwin-x86_64)             PLATFORM="osx-x64"     ;;
  Darwin-arm64)              PLATFORM="osx-arm64"   ;;
  *) die "unsupported platform: $OS $ARCH" ;;
esac

command -v curl >/dev/null || die "curl is required"
command -v tar  >/dev/null || die "tar is required"

# ── ensure runner OS user exists (Linux only) ────────────────────────────────
if [[ "$OS" == "Linux" ]]; then
  if ! id "$RUNNER_USER" &>/dev/null; then
    log "creating system user: $RUNNER_USER"
    useradd --system --create-home --shell /bin/bash "$RUNNER_USER"
  else
    log "runner user: $RUNNER_USER (already exists)"
  fi
fi

RUNNER_HOME="$(getent passwd "$RUNNER_USER" | cut -d: -f6)"

# ── install CI dependencies ──────────────────────────────────────────────────
if [[ "$OS" == "Linux" ]]; then
  # protoc + Node.js (system-wide)
  _apt_needed=0
  command -v protoc &>/dev/null || _apt_needed=1
  command -v node   &>/dev/null || _apt_needed=1
  if [[ "$_apt_needed" -eq 1 ]]; then
    log "updating apt"
    apt-get update -qq
  fi

  if ! command -v protoc &>/dev/null; then
    log "installing protoc"
    apt-get install -y -qq protobuf-compiler
  else
    log "protoc already installed: $(protoc --version)"
  fi

  if ! command -v node &>/dev/null; then
    log "installing Node.js LTS"
    curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - >/dev/null
    apt-get install -y -qq nodejs
  else
    log "Node.js already installed: $(node --version)"
  fi

  # Rust (installed into runner user's home)
  if ! sudo -u "$RUNNER_USER" bash -c 'command -v cargo &>/dev/null'; then
    log "installing Rust for $RUNNER_USER"
    sudo -u "$RUNNER_USER" bash -c \
      'curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --component rustfmt,clippy'
  else
    log "Rust already installed: $(sudo -u "$RUNNER_USER" bash -c 'source ~/.cargo/env && rustc --version')"
    # Ensure rustfmt and clippy are present
    sudo -u "$RUNNER_USER" bash -c 'source ~/.cargo/env && rustup component add rustfmt clippy 2>/dev/null || true'
  fi

  # Bun (installed into runner user's home)
  if ! sudo -u "$RUNNER_USER" bash -c 'command -v bun &>/dev/null'; then
    log "installing Bun for $RUNNER_USER"
    sudo -u "$RUNNER_USER" bash -c \
      'curl -fsSL https://bun.sh/install | bash'
    # Ensure all bun binaries are executable (avoids "Permission denied" on node shim)
    chmod -R +x "${RUNNER_HOME}/.bun/bin/" 2>/dev/null || true
  else
    log "Bun already installed: $(sudo -u "$RUNNER_USER" bash -c 'source ~/.bun/env 2>/dev/null; bun --version')"
    chmod -R +x "${RUNNER_HOME}/.bun/bin/" 2>/dev/null || true
  fi
fi

# ── resolve latest runner version ────────────────────────────────────────────
log "resolving latest actions/runner release"
RUNNER_VERSION="$(curl -fsSL \
  "https://api.github.com/repos/actions/runner/releases/latest" \
  | grep -oE '"tag_name": *"[^"]+"' | head -1 | cut -d'"' -f4)"
[[ -n "$RUNNER_VERSION" ]] || die "failed to resolve latest runner version (rate-limited?)"
log "runner version: $RUNNER_VERSION"

ARCHIVE="actions-runner-${PLATFORM}-${RUNNER_VERSION#v}.tar.gz"
DOWNLOAD_URL="https://github.com/actions/runner/releases/download/${RUNNER_VERSION}/${ARCHIVE}"

# ── download ──────────────────────────────────────────────────────────────────
mkdir -p "$RUNNER_DIR"
cd "$RUNNER_DIR"

log "downloading $ARCHIVE"
curl -fsSL "$DOWNLOAD_URL" -o runner.tar.gz
log "extracting"
tar -xzf runner.tar.gz
rm -f runner.tar.gz

chown -R "$RUNNER_USER": "$RUNNER_DIR"

# install runner OS dependencies (runs as root)
if [[ "$OS" == "Linux" ]] && [[ -f ./bin/installdependencies.sh ]]; then
  log "installing runner OS dependencies"
  ./bin/installdependencies.sh
fi

# ── configure ────────────────────────────────────────────────────────────────
if [[ -f ".runner" ]]; then
  log "existing config detected; removing before reconfigure"
  ./svc.sh stop 2>/dev/null || true
  sudo -u "$RUNNER_USER" ./config.sh remove --token "$RUNNER_TOKEN" 2>/dev/null || true
fi

log "configuring runner as $RUNNER_USER: $RUNNER_NAME → $RUNNER_URL"

CONFIG_ARGS=(
  --unattended
  --url   "$RUNNER_URL"
  --token "$RUNNER_TOKEN"
  --name  "$RUNNER_NAME"
  --work  "$RUNNER_WORK"
  --replace
)
[[ -n "$RUNNER_LABELS" ]] && CONFIG_ARGS+=(--labels "$RUNNER_LABELS")

sudo -u "$RUNNER_USER" ./config.sh "${CONFIG_ARGS[@]}"

# ── inject PATH into runner .env ─────────────────────────────────────────────
if [[ "$OS" == "Linux" ]]; then
  ENV_FILE="$RUNNER_DIR/.env"
  existing_path="$(grep -E '^PATH=' "$ENV_FILE" 2>/dev/null | cut -d= -f2- || echo '/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin')"
  new_path="${RUNNER_HOME}/.cargo/bin:${RUNNER_HOME}/.bun/bin:${existing_path}"
  { grep -v '^PATH=' "$ENV_FILE" 2>/dev/null || true; echo "PATH=${new_path}"; } > "${ENV_FILE}.tmp"
  mv "${ENV_FILE}.tmp" "$ENV_FILE"
  chown "$RUNNER_USER": "$ENV_FILE"
  log "wrote PATH to $ENV_FILE (cargo + bun)"
fi

# ── service ───────────────────────────────────────────────────────────────────
if [[ "$START" -eq 1 ]]; then
  log "installing and starting runner service"
  ./svc.sh install "$RUNNER_USER"
  ./svc.sh start
  log "runner service status:"
  ./svc.sh status || true
else
  log "skipping service start (--no-start)"
  log "to start manually: cd $RUNNER_DIR && ./svc.sh install $RUNNER_USER && ./svc.sh start"
fi

log "完成。runner \"$RUNNER_NAME\" 已注册到 $RUNNER_URL (user: $RUNNER_USER)"
