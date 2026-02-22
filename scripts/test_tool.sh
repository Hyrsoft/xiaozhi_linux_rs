#!/bin/bash
# test_tool.sh - Test script for MCP Gateway
# Reads JSON from stdin, extracts the "command" field, and executes it.

cd /home/hao/projects/xiaozhi_linux_rs

# Read from stdin
INPUT=$(cat)

# Extract command using jq or grep
# Simple fallback if jq is not installed:
# COMMAND=$(echo "$INPUT" | grep -oP '(?<="command":")[^"]*')
if command -v jq &> /dev/null; then
  COMMAND=$(echo "$INPUT" | jq -r '.command')
else
  COMMAND=$(echo "$INPUT" | sed 's/.*"command"\s*:\s*"\([^"]*\)".*/\1/')
fi

if [ -z "$COMMAND" ] || [ "$COMMAND" == "null" ]; then
    echo "Error: No command provided in input schema" >&2
    exit 1
fi

# Execute the parsed command safely and return standard output
eval "$COMMAND"
