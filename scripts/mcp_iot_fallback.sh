#!/bin/bash
# scripts/mcp_iot_fallback.sh
# General purpose script to handle generic "iot" commands from the cloud.
# When Xiaozhi cloud sends an IoT command that isn't wrapped in a tools/call,
# this script receives the raw JSON on stdin and can implement arbitrary local behaviors.

# Read JSON from stdin
INPUT=$(cat)

echo "[IoT Fallback Triggered] Received RAW JSON:"
echo "$INPUT"

# Example: If you want to integrate with Home Assistant via curl or hass-cli:
# 
# HASSIO_URL="http://homeassistant.local:8123"
# HASSIO_TOKEN="your_long_lived_access_token"
# 
# Extract the desired state or entity_id using jq based on your old IoT schema:
# ENTITY_ID=$(echo "$INPUT" | jq -r '.entity_id')
# COMMAND=$(echo "$INPUT" | jq -r '.command')
# 
# if [ "$COMMAND" = "turn_on" ]; then
#     curl -X POST -H "Authorization: Bearer $HASSIO_TOKEN" \
#          -H "Content-Type: application/json" \
#          -d "{\"entity_id\": \"$ENTITY_ID\"}" \
#          $HASSIO_URL/api/services/homeassistant/turn_on
# fi

exit 0
