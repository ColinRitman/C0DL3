# zkC0DL3 Prover Service

Standalone binary for GPU holders to earn HEAT rewards by proving blocks.

## Prerequisites

1. **SP1 Toolchain** — install Succinct's SP1 prover:
   ```bash
   curl -L https://sp1.succinct.xyz | bash
   sp1up
   ```

2. **Build the guest ELF** (RISC-V binary that runs inside the zkVM):
   ```bash
   cd program && cargo prove build
   ```

3. **Running COLDL3 node** at a known URL (default: `http://localhost:8545`)

## Build

```bash
cargo build -p coldl3-prover --release
```

## Usage

### Continuous proving (production)
```bash
./target/release/coldl3-prover \
  --node-url http://localhost:8545 \
  --prover-address 0xYOUR_PROVER_ADDRESS \
  --elf-path program/elf/riscv32im-succinct-zkvm-elf \
  --mode groth16
```

### Test execution (no proof, fast)
```bash
./target/release/coldl3-prover \
  --node-url http://localhost:8545 \
  --prover-address 0xYOUR_PROVER_ADDRESS \
  --mode execute \
  --once
```

### Prove a specific block
```bash
./target/release/coldl3-prover \
  --node-url http://localhost:8545 \
  --prover-address 0xYOUR_PROVER_ADDRESS \
  --block-height 42
```

### Export verification key
```bash
./target/release/coldl3-prover \
  --elf-path program/elf/riscv32im-succinct-zkvm-elf \
  --export-vkey keys/sp1_vk.bin \
  --prover-address dummy
```
Then start the node with: `--prover-vkey keys/sp1_vk.bin`

## Proof Modes

| Mode | Speed | Size | On-chain | Use case |
|------|-------|------|----------|----------|
| `execute` | ~1s | 0 | No | Test input correctness |
| `compressed` | 30-60s | ~1MB | No | Internal verification |
| `groth16` | 60-120s | ~260B | Yes | Production (L2 settlement) |
| `plonk` | 60-120s | ~800B | Yes | Alternative on-chain |

## Environment Variables

- `SP1_PROVER=network` — use Succinct's prover network (requires API key)
- `SP1_PROVER=local` — prove locally on GPU (default)
- `RUST_LOG=info` — logging level

## Architecture

```
Prover Service                    COLDL3 Node
+---------------------------+     +------------------+
| 1. Poll /proof/pending    | --> | Returns pending  |
| 2. GET /proof/block_input | --> | Returns witness  |
| 3. Run SP1 guest in zkVM |     |                  |
| 4. POST /proof/submit     | --> | Verify + reward  |
+---------------------------+     +------------------+
```

The guest program (`program/src/main.rs`) proves:
- Correct EVM execution via revm
- Privacy validation (Pedersen conservation, nullifier freshness)
- State root transition (prev_state_root -> new_state_root)

## Rewards

- **Gas fees** — 69% of total block fees routed to proving winner
- First valid proof per block wins reward
