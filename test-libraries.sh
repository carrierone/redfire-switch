#!/bin/bash

# Test script for Redfire Switch libraries

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "=== Testing Redfire Libraries ==="
echo

# Test codec engine library
echo "Testing Redfire Codec Engine..."
cd "$SCRIPT_DIR/redfire-codec-engine"
if cargo check; then
    echo "✅ Codec engine compiles successfully"
else
    echo "❌ Codec engine compilation failed"
    exit 1
fi

# Test SIP stack library
echo
echo "Testing Redfire SIP Stack..."
cd "$SCRIPT_DIR/redfire-sip-stack"
if cargo check; then
    echo "✅ SIP stack compiles successfully"
else
    echo "❌ SIP stack compilation failed"
    exit 1
fi

# Test minimal SIP stack library
echo
echo "Testing Redfire SIP Stack Minimal..."
cd "$SCRIPT_DIR/redfire-sip-stack-minimal"
if cargo check; then
    echo "✅ Minimal SIP stack compiles successfully"
else
    echo "❌ Minimal SIP stack compilation failed"
    exit 1
fi

# Test workspace
echo
echo "Testing entire workspace..."
cd "$SCRIPT_DIR"
if cargo check --workspace; then
    echo "✅ Workspace compiles successfully"
else
    echo "❌ Workspace compilation failed"
    exit 1
fi

echo
echo "=== All library tests completed successfully! ==="