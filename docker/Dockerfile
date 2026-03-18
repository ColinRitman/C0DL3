# C0DL3 zkSync Era Hyperchain Node — Docker Image
#
# Multi-stage build: compile with Rust, run on minimal Debian.
# Persistent state stored in /app/data (mount a volume).
#
# Build:   docker build -t c0dl3-node -f docker/Dockerfile .
# Run:     docker run -p 9944:9944 -p 30333:30333 -v c0dl3-data:/app/data c0dl3-node

# ── Build stage ───────────────────────────────────────────────────────────────

FROM rust:1.75-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first (cache deps layer)
COPY Cargo.toml Cargo.lock ./

# Create a dummy src to pre-build dependencies
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Copy real source
COPY src/ ./src/
COPY program/ ./program/

# Build the node binary
RUN cargo build --release

# ── Runtime stage ─────────────────────────────────────────────────────────────

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -s /bin/false c0dl3

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/codl3-zksync /app/codl3-zksync

# Copy explorer
COPY explorer/ /app/explorer/

# Create data directory
RUN mkdir -p /app/data && chown -R c0dl3:c0dl3 /app

USER c0dl3

# P2P + RPC
EXPOSE 30333 9944

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:9944/health || exit 1

# Default: testnet mock mode (no settlement key required)
ENTRYPOINT ["./codl3-zksync"]
CMD [ \
    "--data-dir", "/app/data", \
    "--rpc-port", "9944", \
    "--p2p-port", "30333", \
    "--hyperchain-id", "789779", \
    "--era-rpc-url", "https://sepolia.era.zksync.dev" \
]
