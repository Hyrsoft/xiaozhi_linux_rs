#!/bin/bash
# test_tool.sh - Get system load, memory, and uptime information
# Ignores input schema and returns system stats

echo "=== System Uptime & Load ==="
uptime

echo ""
echo "=== Memory Usage ==="
free -h

echo ""
echo "=== Disk Usage (Root) ==="
df -h /

exit 0
