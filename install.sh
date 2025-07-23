#!/bin/bash

# Install script for mdzen

set -e

echo "Installing mdzen..."

# Build the application (suppress warnings for cleaner output)
cargo build --release 2>/dev/null || cargo build --release

# Create installation directory
sudo mkdir -p /usr/local/bin
sudo mkdir -p /usr/local/share/applications

# Install binary
sudo cp target/release/mdzen /usr/local/bin/

# Install desktop entry
sudo cp mdzen.desktop /usr/local/share/applications/

# Update desktop database (suppress errors for missing directories)
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database /usr/local/share/applications/ 2>/dev/null || true
fi

# Update MIME database (suppress errors for missing directories)
if command -v update-mime-database >/dev/null 2>&1; then
    if [ -d /usr/local/share/mime ]; then
        update-mime-database /usr/local/share/mime/ 2>/dev/null || true
    fi
fi

echo "mdzen installed successfully!"
echo "You can now:"
echo "1. Run 'mdzen' from the terminal"
echo "2. Set it as the default application for markdown files in your file manager"
echo "3. Find it in your application launcher"