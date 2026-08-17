#!/bin/bash

# This script sets up environment variables for API keys

# Copy template environment file without overwriting an existing local file.
echo "Setting up environment variables..."
if [[ ! -f .env ]]; then
    cp .env.example .env
fi

# Function to update API key in .env file
update_api_key() {
    local key_name=$1
    local key_value=$2
    sed -i "" "s|$key_name=.*|$key_name=$key_value|g" .env
}

# Function to check if key needs update
needs_update() {
    local value=$1
    [[ -z "$value" ]]
}

# Update API keys in .env file
for key in ANTHROPIC_API_KEY GROQ_API_KEY OPENAI_API_KEY; do
    # Get current value from environment
    current_value="${!key}"
    
    # Check if key needs to be updated
    if needs_update "$current_value"; then
        echo "$key is not set. Press Enter to skip or enter your API key:"
        read -p "Enter $key (or press Enter to skip): " new_value
        if [ -n "$new_value" ]; then
            update_api_key "$key" "$new_value"
        fi
    else
        update_api_key "$key" "$current_value"
    fi
done

# Report names only. Never print credential values.
echo "Environment setup complete. Credential names configured:"
for key in ANTHROPIC_API_KEY GROQ_API_KEY OPENAI_API_KEY; do
    if grep -q "^${key}=.\+" .env; then
        echo "- $key: configured"
    else
        echo "- $key: empty"
    fi
done
