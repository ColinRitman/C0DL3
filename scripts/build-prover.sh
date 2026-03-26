#!/usr/bin/env bash
# Build the SP1 guest ELF and export the verification key.
#
# Prerequisites:
#   curl -L https://sp1.succinct.xyz | bash && sp1up
#   cargo install cargo-prove
#
# Usage:
#   ./scripts/build-prover.sh              # build guest + export vkey
#   ./scripts/build-prover.sh --elf-only   # build guest ELF only
#   ./scripts/build-prover.sh --vkey-only  # export vkey only (requires ELF)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GUEST_DIR="$ROOT/program"
ELF_PATH="$GUEST_DIR/elf/riscv32im-succinct-zkvm-elf"
VKEY_DIR="$ROOT/keys"
VKEY_PATH="$VKEY_DIR/sp1_vk.bin"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() { echo -e "${GREEN}[build]${NC} $1"; }
warn() { echo -e "${YELLOW}[warn]${NC} $1"; }
err() { echo -e "${RED}[error]${NC} $1" >&2; exit 1; }

# Check SP1 toolchain
check_sp1() {
    if ! command -v cargo-prove &>/dev/null; then
        err "cargo-prove not found. Install SP1 toolchain:\n  curl -L https://sp1.succinct.xyz | bash && sp1up"
    fi
    log "SP1 toolchain found: $(cargo prove --version 2>/dev/null || echo 'unknown')"
}

# Build guest ELF
build_elf() {
    log "Building guest ELF (RISC-V binary for SP1 zkVM)..."
    cd "$GUEST_DIR"
    cargo prove build
    if [ ! -f "$ELF_PATH" ]; then
        err "ELF not found at $ELF_PATH after build. Check cargo prove output."
    fi
    local size
    size=$(wc -c < "$ELF_PATH")
    log "Guest ELF built: $ELF_PATH ($size bytes)"
}

# Export verification key
export_vkey() {
    if [ ! -f "$ELF_PATH" ]; then
        err "ELF not found at $ELF_PATH. Run build first."
    fi
    mkdir -p "$VKEY_DIR"
    log "Exporting SP1 verification key..."
    cd "$ROOT"
    cargo run -p coldl3-prover --release -- \
        --elf-path "$ELF_PATH" \
        --export-vkey "$VKEY_PATH" \
        --prover-address dummy
    local size
    size=$(wc -c < "$VKEY_PATH")
    log "Verification key exported: $VKEY_PATH ($size bytes)"
    log ""
    log "Start the node with:"
    log "  cargo run --release -- --prover-vkey $VKEY_PATH"
}

# Build prover binary
build_prover() {
    log "Building prover binary..."
    cd "$ROOT"
    cargo build -p coldl3-prover --release
    log "Prover binary: target/release/coldl3-prover"
}

# Main
case "${1:-all}" in
    --elf-only)
        check_sp1
        build_elf
        ;;
    --vkey-only)
        export_vkey
        ;;
    --prover-only)
        build_prover
        ;;
    all|*)
        check_sp1
        build_elf
        build_prover
        export_vkey
        log ""
        log "=== Build complete ==="
        log "Guest ELF:  $ELF_PATH"
        log "Prover:     target/release/coldl3-prover"
        log "Vkey:       $VKEY_PATH"
        log ""
        log "Start proving:"
        log "  # Terminal 1: Start node with vkey"
        log "  cargo run --release -- --prover-vkey $VKEY_PATH"
        log ""
        log "  # Terminal 2: Start prover"
        log "  ./target/release/coldl3-prover \\"
        log "    --node-url http://localhost:8545 \\"
        log "    --prover-address 0xYOUR_ADDRESS \\"
        log "    --mode groth16"
        ;;
esac
