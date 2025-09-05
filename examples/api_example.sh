#!/bin/bash

# zkC0DL3 API Example Script
# This script demonstrates how to interact with the zkC0DL3 node API

NODE_URL="http://localhost:9944"

echo "=== zkC0DL3 API Examples ==="
echo ""

# Health check
echo "1. Health Check:"
curl -s "$NODE_URL/health" | jq '.'
echo ""

# Network stats
echo "2. Network Statistics:"
curl -s "$NODE_URL/stats" | jq '.'
echo ""

# Hyperchain info
echo "3. Hyperchain Information:"
curl -s "$NODE_URL/hyperchain/info" | jq '.'
echo ""

# L1 batches
echo "4. L1 Batches:"
curl -s "$NODE_URL/hyperchain/batches" | jq '.'
echo ""

# Submit a test transaction
echo "5. Submitting Test Transaction:"
curl -s -X POST "$NODE_URL/submit_transaction" \
  -H "Content-Type: application/json" \
  -d '{
    "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
    "from": "0x1111111111111111111111111111111111111111",
    "to": "0x2222222222222222222222222222222222222222",
    "value": 1000000,
    "gas_price": 20000000000,
    "gas_limit": 21000,
    "nonce": 0,
    "data": [],
    "signature": "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "status": "Pending"
  }' | jq '.'
echo ""

# Get a specific block
echo "6. Get Block (height 1):"
curl -s "$NODE_URL/blocks/1" | jq '.'
echo ""

# Get a specific transaction
echo "7. Get Transaction:"
curl -s "$NODE_URL/transactions/0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" | jq '.'
echo ""

# Merge-mining statistics
echo "8. Merge-Mining Statistics:"
curl -s "$NODE_URL/merge-mining/stats" | jq '.'
echo ""

# Fuego blocks
echo "9. Fuego Blocks:"
curl -s "$NODE_URL/merge-mining/fuego-blocks" | jq '.'
echo ""

# Specific Fuego block
echo "10. Specific Fuego Block:"
curl -s "$NODE_URL/merge-mining/fuego-blocks/1" | jq '.'
echo ""

echo "=== API Examples Complete ==="
