#!/bin/bash
# Installation script for tycode binaries
# Automatically installs to ~/.local/bin (user) or /usr/local/bin (root)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Installing tycode binaries...${NC}"

# Determine installation directory
if [ "$EUID" -eq 0 ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

echo -e "${YELLOW}Installation directory: $INSTALL_DIR${NC}"

# Build in release mode
echo -e "${GREEN}Building in release mode...${NC}"
cargo build --release

# Install binaries
echo -e "${GREEN}Installing binaries...${NC}"
cp target/release/tycode-chat "$INSTALL_DIR/"
cp target/release/tycode-cli "$INSTALL_DIR/"

# Make them executable
chmod +x "$INSTALL_DIR/tycode-chat"
chmod +x "$INSTALL_DIR/tycode-cli"

# Create convenience symlinks
ln -sf "$INSTALL_DIR/tycode-chat" "$INSTALL_DIR/tycode"
ln -sf "$INSTALL_DIR/tycode-cli" "$INSTALL_DIR/tycli"

echo -e "${GREEN}Binaries installed:${NC}"
echo "  - tycode-chat (also available as 'tycode')"
echo "  - tycode-cli (also available as 'tycli')"

# Check if install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo -e "${YELLOW}Warning: $INSTALL_DIR is not in your PATH${NC}"
    echo -e "${YELLOW}Add the following line to your ~/.bashrc or ~/.zshrc:${NC}"
    echo -e "${GREEN}export PATH=\"\$PATH:$INSTALL_DIR\"${NC}"
    
    # Offer to add it automatically
    read -p "Would you like to add it to your shell config now? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        SHELL_RC=""
        if [ -f "$HOME/.bashrc" ]; then
            SHELL_RC="$HOME/.bashrc"
        elif [ -f "$HOME/.zshrc" ]; then
            SHELL_RC="$HOME/.zshrc"
        fi
        
        if [ -n "$SHELL_RC" ]; then
            echo "export PATH=\"\$PATH:$INSTALL_DIR\"" >> "$SHELL_RC"
            echo -e "${GREEN}Added to $SHELL_RC. Run 'source $SHELL_RC' to apply.${NC}"
        fi
    fi
else
    echo -e "${GREEN}$INSTALL_DIR is already in PATH ✓${NC}"
fi

echo -e "${GREEN}Installation complete!${NC}"