#!/usr/bin/env bash
# Local test setup for the container provider.
#
# Run inside `nix develop .#backend` (cargo, postgres, ansible, protoc):
#
#   scripts/container-dev/run.sh up            build, start Postgres + malboxd, create and provision the machine
#   scripts/container-dev/run.sh submit FILE   submit FILE as a Linux task, wait, print the result
#   scripts/container-dev/run.sh submit-auto FILE
#                                              submit FILE without a platform/snapshot (profile routing),
#                                              wait, print the artifact tree
#   scripts/container-dev/run.sh status        daemon/Postgres state, machines, snapshots
#   scripts/container-dev/run.sh logs          follow the daemon log
#   scripts/container-dev/run.sh down          stop malboxd and Postgres (state is kept)
#   scripts/container-dev/run.sh clean         down, then delete the machine, its images and all state
#
# Environment:
#   MALBOX_DEV_ENGINE   podman (default; rootful, uses sudo) or docker (user must reach the socket)
#   MALBOX_DEV_PORT     daemon HTTP port (default 8080)
#   MALBOX_DEV_PG_PORT  dedicated Postgres port (default 5433)
#   MALBOX_DEV_TIMEOUT  task timeout for `submit` in seconds (default 60)
#   MALBOX_DEV_STATE    state directory (default back-end/.data/container-dev)
#
# Host prerequisites: rootful Podman or Docker; the community.docker Ansible
# collection (`ansible-galaxy collection install community.docker`).
#
# Only one malboxd can run per host (its plugin IPC is host-wide): stop any
# other daemon before `up`. Machines, network and state stay separate from it.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BACKEND=$(cd "$HERE/../.." && pwd)
STATE=${MALBOX_DEV_STATE:-$BACKEND/.data/container-dev}
PORT=${MALBOX_DEV_PORT:-8080}
PG_PORT=${MALBOX_DEV_PG_PORT:-5433}
TIMEOUT=${MALBOX_DEV_TIMEOUT:-60}
ENGINE=${MALBOX_DEV_ENGINE:-podman}

API=http://127.0.0.1:$PORT
# Must match machines.toml and malbox.toml.in. Distinct from names a regular
# setup would use: container names, snapshot labels and the network live in
# the engine, shared with any other malbox daemon on this host.
MACHINE=container-dev
IMAGE=container-dev-alpine
NETWORK=malbox-dev
PLUGIN=guest-exec-checker
SNAPSHOT=with-plugins
BUILD_IMAGE=malbox-dev-alpine
PLAYBOOK=$BACKEND/configuration/infrastructure/ansible/playbooks/linux/deploy-guest-plugin-container.yml
# .cargo/config.toml pins [build] target.
TARGET_DIR=$BACKEND/target/x86_64-unknown-linux-gnu

MALBOXD=$STATE/bin/malboxd
MALBOX_BIN=$STATE/bin/malbox
PIDFILE=$STATE/malboxd.pid
LOG=$STATE/malboxd.log
PGDATA=$STATE/pg

case $ENGINE in
podman)
    ENG=(sudo podman)
    SOCKET=/run/podman/podman.sock
    ;;
docker)
    ENG=(docker)
    SOCKET=/var/run/docker.sock
    ;;
*)
    echo "error: MALBOX_DEV_ENGINE must be podman or docker" >&2
    exit 1
    ;;
esac

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die() {
    printf '\033[1;31merror:\033[0m %s\n' "$*" >&2
    exit 1
}

require() {
    local cmd
    for cmd in "$@"; do
        command -v "$cmd" >/dev/null || die "'$cmd' not found; run inside 'nix develop .#backend'"
    done
}

malbox() { "$MALBOX_BIN" --api-url "$API" "$@"; }
api() { curl -sf "$API$1"; }
psql_q() { psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -tAc "$@"; }

# --- engine -----------------------------------------------------------------

engine_up() {
    if [[ $ENGINE == podman ]]; then
        log "Starting rootful podman.socket and granting $USER access"
        sudo systemctl start podman.socket
        # The ACLs are lost whenever the socket (or /run after a reboot) is
        # recreated, so set them every time. /run/podman can come up root-only
        # (0700), so the user also needs traverse permission on it.
        sudo setfacl -m "u:$USER:x" "$(dirname "$SOCKET")"
        sudo setfacl -m "u:$USER:rw" "$SOCKET"
    fi
    [[ -w $SOCKET ]] || die "no write access to $SOCKET (Docker: add yourself to the docker group)"
}

engine_release() {
    # The socket ACL stays: a regular malboxd of the same user may need it.
    if [[ $ENGINE == podman ]]; then
        # Podman's compat API leaves two conmon processes per attached exec
        # (Ansible provisioning); reap the ones for this machine.
        sudo pkill -f "conmon .*-n malbox-$MACHINE .*--exec" 2>/dev/null || true
    fi
}

# --- postgres ---------------------------------------------------------------

pg_up() {
    if [[ ! -f $PGDATA/PG_VERSION ]]; then
        log "Initialising dedicated Postgres in $PGDATA"
        initdb -D "$PGDATA" -U postgres --auth=trust >/dev/null
    fi
    if ! pg_ctl -D "$PGDATA" status >/dev/null 2>&1; then
        log "Starting Postgres on 127.0.0.1:$PG_PORT"
        pg_ctl -D "$PGDATA" -l "$STATE/postgres.log" -w \
            -o "-p $PG_PORT -c listen_addresses=127.0.0.1 -c unix_socket_directories=''" \
            start >/dev/null
    fi
    psql_q "select 1 from pg_database where datname = 'malbox'" | grep -qx 1 ||
        createdb -h 127.0.0.1 -p "$PG_PORT" -U postgres malbox
}

pg_down() {
    if [[ -f $PGDATA/PG_VERSION ]] && pg_ctl -D "$PGDATA" status >/dev/null 2>&1; then
        log "Stopping Postgres"
        pg_ctl -D "$PGDATA" -m fast -w stop >/dev/null
    fi
}

image_registered() {
    # Before the daemon's first start the table does not exist: not registered.
    psql -h 127.0.0.1 -p "$PG_PORT" -U postgres -d malbox -tAc \
        "select 1 from images where name = '$IMAGE'" 2>/dev/null | grep -qx 1
}

# --- builds -----------------------------------------------------------------

build_daemon() {
    log "Building malboxd (container provider) and the malbox CLI"
    (
        cd "$BACKEND"
        export SQLX_OFFLINE=true
        cargo build -q -p malboxd --no-default-features --features provider-container,provisioner-ansible
        # Copy before the next build: `cargo build -p malboxd` with default
        # features overwrites the same path.
        install -D -m 0755 "$TARGET_DIR/debug/malboxd" "$MALBOXD"
        cargo build -q -p malbox
        install -D -m 0755 "$TARGET_DIR/debug/malbox" "$MALBOX_BIN"
    )
}

build_plugin() {
    local example=$BACKEND/examples/plugins/$PLUGIN
    local src=$STATE/build/$PLUGIN
    log "Building $PLUGIN as a static musl binary"
    mkdir -p "$src"
    # Copy the example (minus its target/) and swap in the Linux manifest; the
    # plugin's runtime settings are compiled in from plugin.toml.
    find "$example" -mindepth 1 -maxdepth 1 ! -name target -exec cp -r {} "$src/" \;
    cp "$HERE/$PLUGIN.plugin.toml" "$src/plugin.toml"
    sed -i "s|path = \"../../../crates/malbox-plugin-sdk\"|path = \"$BACKEND/crates/malbox-plugin-sdk\"|" "$src/Cargo.toml"
    grep -q "path = \"$BACKEND/crates/malbox-plugin-sdk\"" "$src/Cargo.toml" ||
        die "could not repoint the plugin SDK dependency in $src/Cargo.toml"
    # Standalone workspace: the copy may live under back-end/, whose workspace
    # does not list it.
    printf '\n[workspace]\n' >>"$src/Cargo.toml"
    (cd "$src" && cargo build -q --release --target x86_64-unknown-linux-musl)

    install -D -m 0644 "$src/plugin.toml" "$STATE/plugins/$PLUGIN/plugin.toml"
    install -D -m 0755 "$src/target/x86_64-unknown-linux-musl/release/$PLUGIN" "$STATE/plugins/$PLUGIN/$PLUGIN"

    build_host_plugin host-archive-extractor
}

# Regular (host-side) build of an example host plugin, installed with its
# own plugin.toml.
build_host_plugin() {
    local name=$1
    local example=$BACKEND/examples/plugins/$name
    local target=$STATE/build/target-$name
    log "Building host plugin $name"
    cargo build -q --release --manifest-path "$example/Cargo.toml" \
        --target x86_64-unknown-linux-gnu --target-dir "$target"
    install -D -m 0644 "$example/plugin.toml" "$STATE/plugins/$name/plugin.toml"
    install -D -m 0755 "$target/x86_64-unknown-linux-gnu/release/$name" "$STATE/plugins/$name/$name"
}

register_image() {
    log "Building image $BUILD_IMAGE and registering it as '$IMAGE'"
    "${ENG[@]}" build -q -t "$BUILD_IMAGE" -f "$HERE/Containerfile" "$HERE" >/dev/null
    rm -f "$STATE/$BUILD_IMAGE.tar"
    local quiet=()
    [[ $ENGINE == podman ]] && quiet=(-q)
    "${ENG[@]}" save "${quiet[@]}" -o "$STATE/$BUILD_IMAGE.tar" "$BUILD_IMAGE"
    [[ $ENGINE == podman ]] && sudo chown "$USER" "$STATE/$BUILD_IMAGE.tar"
    # The daemon moves the archive into [images].store_path.
    malbox image register --name "$IMAGE" --platform linux --arch x64 \
        --path "$STATE/$BUILD_IMAGE.tar" --image-format tar
}

# --- daemon -----------------------------------------------------------------

write_config() {
    local with_machines=$1
    sed -e "s|@PORT@|$PORT|g" -e "s|@PG_PORT@|$PG_PORT|g" \
        -e "s|@SOCKET@|$SOCKET|g" -e "s|@STATE@|$STATE|g" \
        "$HERE/malbox.toml.in" >"$STATE/malbox.toml"
    if [[ $with_machines == yes ]]; then
        cat "$HERE/machines.toml" >>"$STATE/malbox.toml"
    fi
}

daemon_running() { [[ -f $PIDFILE ]] && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; }

daemon_start() {
    if curl -s -o /dev/null "$API/healthz"; then
        die "port $PORT is already in use (another malboxd?); stop it or set MALBOX_DEV_PORT"
    fi
    log "Starting malboxd on $API (log: $LOG)"
    # Own config and XDG dirs: nothing under ~/.config or ~/.local is touched.
    MALBOX_CONFIG=$STATE/malbox.toml \
        XDG_CONFIG_HOME=$STATE/xdg/config XDG_DATA_HOME=$STATE/xdg/data \
        XDG_CACHE_HOME=$STATE/xdg/cache XDG_STATE_HOME=$STATE/xdg/state \
        nohup "$MALBOXD" >>"$LOG" 2>&1 &
    echo $! >"$PIDFILE"

    local _
    for _ in $(seq 240); do
        curl -sf -o /dev/null "$API/healthz" && return 0
        if ! daemon_running; then
            tail -n 30 "$LOG" >&2
            die "malboxd exited during startup"
        fi
        sleep 0.5
    done
    die "malboxd did not answer on $API within 120s"
}

daemon_stop() {
    daemon_running || return 0
    log "Stopping malboxd"
    local pid _
    pid=$(cat "$PIDFILE")
    kill "$pid"
    for _ in $(seq 60); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.5
    done
    kill -0 "$pid" 2>/dev/null && kill -9 "$pid"
    rm -f "$PIDFILE"
}

# --- machine ----------------------------------------------------------------

# Prints "<id> <status>" for the dev machine, nothing if it does not exist.
machine_info() {
    api /v1/machines | python3 -c '
import json, sys
for m in json.load(sys.stdin):
    if m["name"] == sys.argv[1]:
        print(m["id"], m["status"])
' "$MACHINE"
}

# Prints the ID of snapshot $SNAPSHOT on machine $1, nothing if missing.
snapshot_id() {
    api "/v1/machines/$1/snapshots" | python3 -c '
import json, sys
for s in json.load(sys.stdin):
    if s["name"] == sys.argv[1]:
        print(s["id"])
' "$SNAPSHOT"
}

wait_machine_ready() {
    local id status _
    for _ in $(seq 180); do
        read -r id status < <(machine_info) || true
        case ${status:-} in
        ready) return 0 ;;
        failed) die "machine $MACHINE failed; see $LOG" ;;
        esac
        sleep 1
    done
    die "machine $MACHINE not ready after 180s (status: ${status:-missing}); see $LOG"
}

provision() {
    local id status
    read -r id status < <(machine_info) || die "machine $MACHINE not found; see $LOG"
    if [[ -n $(snapshot_id "$id") ]]; then
        log "Snapshot '$SNAPSHOT' exists, skipping provisioning"
        return 0
    fi
    log "Provisioning $PLUGIN via Ansible (community.docker.docker_api)"
    malbox machine provision "$MACHINE" --provisioner ansible --snapshot "$SNAPSHOT" \
        --config "{\"playbook\": \"$PLAYBOOK\"}" --plugins "$PLUGIN"
    [[ -n $(snapshot_id "$id") ]] || die "provisioning did not create snapshot '$SNAPSHOT'; see $LOG"
    wait_machine_ready
}

# --- commands ---------------------------------------------------------------

cmd_up() {
    require cargo initdb pg_ctl psql createdb ansible-playbook ansible-galaxy python3 curl
    ansible-galaxy collection list community.docker 2>/dev/null | grep -q community.docker ||
        die "Ansible collection missing: ansible-galaxy collection install community.docker"
    mkdir -p "$STATE/images" "$STATE/plugins" "$STATE/bin"

    engine_up
    pg_up
    build_daemon
    build_plugin
    daemon_stop

    if ! image_registered; then
        # Machines are created at startup, and only if their image is
        # registered; registering needs a running daemon. Hence two starts.
        write_config no
        daemon_start
        register_image
        daemon_stop
    fi

    write_config yes
    daemon_start
    log "Waiting for machine $MACHINE"
    wait_machine_ready
    provision

    log "Ready. Submit a sample with:"
    echo "  $0 submit <file>"
}

cmd_submit() {
    [[ $# -eq 1 ]] || die "usage: $0 submit FILE"
    [[ -f $1 ]] || die "no such file: $1"
    daemon_running || die "malboxd is not running; run '$0 up' first"

    local id status snap
    read -r id status < <(machine_info) || die "machine $MACHINE not found; run '$0 up'"
    snap=$(snapshot_id "$id")
    [[ -n $snap ]] || die "snapshot '$SNAPSHOT' missing; run '$0 up'"

    submit_and_wait get "$1" --platform linux --snapshot-id "$snap"
}

# Like submit, but detonation comes from [[orchestration.detonation_profiles]].
cmd_submit_auto() {
    [[ $# -eq 1 ]] || die "usage: $0 submit-auto FILE"
    [[ -f $1 ]] || die "no such file: $1"
    daemon_running || die "malboxd is not running; run '$0 up' first"

    submit_and_wait tree "$1"
}

# submit_and_wait VIEW FILE [SUBMIT_ARGS...]: submit, wait for a terminal
# status, then run `malbox task VIEW <id>`.
submit_and_wait() {
    local view=$1 file=$2
    shift 2
    local out task status
    out=$(malbox task submit "$file" "$@" --timeout "$TIMEOUT" 2>&1) || {
        echo "$out" >&2
        die "task submission failed"
    }
    echo "$out"
    task=$(grep -oE 'ID: [0-9]+' <<<"$out" | grep -oE '[0-9]+')

    log "Waiting for task $task (up to $((TIMEOUT + 120))s; '$0 logs' shows progress)"
    local deadline=$((SECONDS + TIMEOUT + 120))
    while ((SECONDS < deadline)); do
        status=$(api "/v1/tasks/$task" | python3 -c 'import json, sys; print(json.load(sys.stdin)["status"])')
        case $status in
        completed | failed | canceled) break ;;
        esac
        sleep 2
    done
    malbox task "$view" "$task"
}

cmd_status() {
    if daemon_running; then
        echo "malboxd:  running (pid $(cat "$PIDFILE"), $API)"
    else
        echo "malboxd:  stopped"
    fi
    if [[ -f $PGDATA/PG_VERSION ]] && pg_ctl -D "$PGDATA" status >/dev/null 2>&1; then
        echo "postgres: running (127.0.0.1:$PG_PORT)"
    else
        echo "postgres: stopped"
    fi
    if daemon_running; then
        echo
        malbox machine list
        echo
        malbox machine snapshot list "$MACHINE" || true
    fi
}

cmd_logs() { tail -n 50 -f "$LOG"; }

cmd_down() {
    daemon_stop
    pg_down
    engine_release
}

cmd_clean() {
    cmd_down
    log "Removing machine container, images and network"
    "${ENG[@]}" rm -f "malbox-$MACHINE" >/dev/null 2>&1 || true
    local ref
    # Remove by tag: snapshot images form parent/child chains, which
    # untagging handles and ID removal refuses.
    while read -r ref; do
        [[ -n $ref ]] && "${ENG[@]}" rmi -f "$ref" >/dev/null 2>&1 || true
    done < <("${ENG[@]}" images --format '{{.Repository}}:{{.Tag}}' --filter "label=malbox.machine=$MACHINE")
    # Base tags carry no machine label. Plain (non-forced) rmi only removes
    # tags no container uses, so other daemons' machines are left alone.
    while read -r ref; do
        [[ -n $ref ]] && "${ENG[@]}" rmi "$ref" >/dev/null 2>&1 || true
    done < <("${ENG[@]}" images --format '{{.Repository}}:{{.Tag}}' | grep -E '(^|/)malbox-base:' || true)
    "${ENG[@]}" rmi "$BUILD_IMAGE" >/dev/null 2>&1 || true
    "${ENG[@]}" network rm "$NETWORK" >/dev/null 2>&1 || true
    log "Deleting $STATE"
    rm -rf "$STATE"
}

case ${1:-} in
up) cmd_up ;;
submit)
    shift
    cmd_submit "$@"
    ;;
submit-auto)
    shift
    cmd_submit_auto "$@"
    ;;
status) cmd_status ;;
logs) cmd_logs ;;
down) cmd_down ;;
clean) cmd_clean ;;
*)
    sed -n '2,27p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
    ;;
esac
