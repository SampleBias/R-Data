#!/bin/bash

# Secure API Key Setup Script for R-Data Agent
# Usage: ./setup_api_key.sh "YOUR_API_KEY"

set -e

API_KEY="${1:-}"

if [ -z "$API_KEY" ]; then
    echo "Usage: $0 <your-api-key>"
    echo ""
    echo "This script securely sets your Z.ai API key."
    echo "The key will be stored in ~/.config/r-data-agent/config.toml"
    echo ""
    echo "To get an API key, visit: https://z.ai"
    exit 1
fi

echo "Setting up secure configuration..."

# Create config directory
mkdir -p ~/.config/r-data-agent

# Write config with API key
cat > ~/.config/r-data-agent/config.toml << EOF
# R-Data Agent Configuration
# API key for Z.ai GLM 4.7
api_key = "$API_KEY"

# Visualization settings
viz_width = 800
viz_height = 600
default_bins = 20
EOF

# Set secure permissions (read/write for owner only)
chmod 600 ~/.config/r-data-agent/config.toml

echo "✓ API key configured securely"
echo "✓ File permissions set to 600 (owner read/write only)"
echo ""
echo "Your API key is now protected and will not be tracked by git."
echo ""
echo "To verify: cat ~/.config/r-data-agent/config.toml"
