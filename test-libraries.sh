#!/bin/bash

# Test script for Redfire Switch libraries

echo "=== Testing Redfire Libraries ==="
echo

# Test codec engine library
echo "Testing Redfire Codec Engine..."
cd /home/justin/projects/redfire-switch/redfire-codec-engine
if cargo check; then
    echo "✅ Codec engine compiles successfully"
else
    echo "❌ Codec engine compilation failed"
    exit 1
fi

# Test SIP stack library
echo
echo "Testing Redfire SIP Stack..."
cd /home/justin/projects/redfire-switch/redfire-sip-stack
if cargo check; then
    echo "✅ SIP stack compiles successfully"
else
    echo "❌ SIP stack compilation failed"
    exit 1
fi

# Test minimal SIP stack library
echo
echo "Testing Redfire SIP Stack Minimal..."
cd /home/justin/projects/redfire-switch/redfire-sip-stack-minimal
if cargo check; then
    echo "✅ Minimal SIP stack compiles successfully"
else
    echo "❌ Minimal SIP stack compilation failed"
    exit 1
fi

# Test workspace
echo
echo "Testing entire workspace..."
cd /home/justin/projects/redfire-switch
if cargo check --workspace; then
    echo "✅ Workspace compiles successfully"
else
    echo "❌ Workspace compilation failed"
    exit 1
fi

echo
echo "=== All library tests completed successfully! ==="