#!/bin/bash
# Deploys the Discord bot server (elodisco binary) to HOST, where it runs as the elodisco systemd
# service. The binary is built locally, so HOST needs a glibc/OpenSSL compatible with this machine.
#
# Usage: ./deploy-elodisco.sh HOST            build, back up the current binary, install, restart
#        ./deploy-elodisco.sh HOST rollback   restore the backup made by the last deploy, restart
set -euo pipefail

PROJECT_ROOT=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

SERVICE=elodisco
BINARY=/var/www/elodisco/elodisco
BACKUP=$BINARY.prev
UPLOAD=/tmp/elodisco.new

usage() {
    echo "Usage: $0 HOST [rollback]" >&2
    exit 1
}

HOST=${1:-}
COMMAND=${2:-deploy}
[ -n "$HOST" ] || usage

show_startup_log() {
    echo "Startup log:"
    sleep 5
    ssh "$HOST" "journalctl -u $SERVICE _SYSTEMD_INVOCATION_ID=\$(systemctl show -p InvocationID --value $SERVICE) --no-pager"
}

case "$COMMAND" in
deploy)
    cd "$PROJECT_ROOT"
    cargo build --release --bin elodisco
    scp target/release/elodisco "$HOST:$UPLOAD"
    # Copied over the old file, so it keeps its owner and mode. The service is stopped first,
    # a running binary can't be overwritten.
    ssh "$HOST" bash -s <<EOF
set -e
cp -p $BINARY $BACKUP
systemctl stop $SERVICE
cp $UPLOAD $BINARY
rm $UPLOAD
systemctl start $SERVICE
EOF
    echo "elodisco deployed to $HOST, previous binary kept as $BACKUP."
    ;;
rollback)
    ssh "$HOST" bash -s <<EOF
set -e
test -f $BACKUP || { echo "No backup at $BACKUP." >&2; exit 1; }
systemctl stop $SERVICE
cp $BACKUP $BINARY
systemctl start $SERVICE
EOF
    echo "elodisco on $HOST rolled back to $BACKUP."
    ;;
*)
    usage
    ;;
esac

show_startup_log
