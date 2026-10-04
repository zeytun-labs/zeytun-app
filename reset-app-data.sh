#!/bin/bash

echo "Resetting Castle app data..."

# macOS
MAC_DIR="$HOME/Library/Application Support/com.castle.app"
if [ -d "$MAC_DIR" ]; then
    echo "Found macOS app data. Deleting..."
    rm -rf "$MAC_DIR"
    echo "Deleted $MAC_DIR"
fi

# Linux
LINUX_DIR="$HOME/.local/share/com.castle.app"
if [ -d "$LINUX_DIR" ]; then
    echo "Found Linux app data. Deleting..."
    rm -rf "$LINUX_DIR"
    echo "Deleted $LINUX_DIR"
fi

echo "App data reset successfully! You can now start the app fresh."
