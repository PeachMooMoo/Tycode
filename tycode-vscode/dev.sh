#!/bin/bash

# Development helper script for TyCode VSCode extension

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}TyCode VSCode Extension Development Helper${NC}"
echo "==========================================="
echo ""

case "$1" in
    setup)
        echo -e "${YELLOW}Setting up development environment...${NC}"
        
        # Check for Node.js
        if ! command -v node &> /dev/null; then
            echo -e "${RED}Node.js is not installed. Please install Node.js first.${NC}"
            exit 1
        fi
        
        # Check for Rust
        if ! command -v rustc &> /dev/null; then
            echo -e "${RED}Rust is not installed. Please install Rust first.${NC}"
            exit 1
        fi
        
        # Install Node dependencies
        echo -e "${YELLOW}Installing Node dependencies...${NC}"
        npm install
        
        echo -e "${GREEN}Setup complete!${NC}"
        ;;
        
    build)
        echo -e "${YELLOW}Building extension with subprocess bridge...${NC}"
        
        # Build native Rust CLI
        echo -e "${YELLOW}Building native Rust CLI...${NC}"
        cd ..
        cargo build --release
        cd tycode-vscode
        
        # Build TypeScript
        echo -e "${YELLOW}Compiling TypeScript...${NC}"
        npm run compile
        
        # Copy webview assets
        echo -e "${YELLOW}Copying webview files...${NC}"
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        # Verify files were copied
        if [ -f "out/webview/chat.css" ] && [ -f "out/webview/chat.js" ]; then
            echo -e "${GREEN}Webview files copied successfully${NC}"
        else
            echo -e "${RED}Warning: Some webview files may not have been copied${NC}"
        fi
        
        echo -e "${GREEN}Build complete!${NC}"
        echo -e "${YELLOW}Note: The extension uses the native CLI at ../target/release/tycode${NC}"
        ;;
        
    quick-build)
        echo -e "${YELLOW}Quick building extension (debug mode)...${NC}"
        
        # Build native Rust CLI in debug mode
        echo -e "${YELLOW}Building native Rust CLI (debug mode)...${NC}"
        cd ..
        cargo build
        cd tycode-vscode
        
        # Build TypeScript
        echo -e "${YELLOW}Compiling TypeScript...${NC}"
        npm run compile
        
        # Copy webview assets
        echo -e "${YELLOW}Copying webview files...${NC}"
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        echo -e "${GREEN}Quick build complete!${NC}"
        echo -e "${YELLOW}Note: The extension uses the native CLI at ../target/debug/tycode${NC}"
        ;;
        
    watch)
        echo -e "${YELLOW}Starting watch mode...${NC}"
        
        # First do a full build to ensure everything is copied
        echo -e "${YELLOW}Doing initial build...${NC}"
        npm run compile
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        echo -e "${GREEN}Initial build complete!${NC}"
        echo -e "${YELLOW}Starting TypeScript watch mode...${NC}"
        echo -e "${YELLOW}Note: You'll need to manually rebuild the Rust CLI if you change it${NC}"
        npm run watch
        ;;
        
    package)
        echo -e "${YELLOW}Creating VSIX package...${NC}"
        
        # Full build first
        echo -e "${YELLOW}Building native Rust CLI (release mode)...${NC}"
        cd ..
        cargo build --release
        cd tycode-vscode
        
        echo -e "${YELLOW}Compiling TypeScript...${NC}"
        npm run compile
        
        echo -e "${YELLOW}Copying webview files...${NC}"
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        echo -e "${YELLOW}Creating VSIX package...${NC}"
        npm run package
        
        if [ -f *.vsix ]; then
            echo -e "${GREEN}Package created successfully!${NC}"
            ls -la *.vsix
        else
            echo -e "${RED}Failed to create package${NC}"
        fi
        ;;
        
    test)
        echo -e "${YELLOW}Running tests...${NC}"
        npm test
        ;;
        
    clean)
        echo -e "${YELLOW}Cleaning build artifacts...${NC}"
        rm -rf out/
        rm -rf node_modules/
        rm -f *.vsix
        # Clean Rust build
        cd ..
        cargo clean
        cd tycode-vscode
        echo -e "${GREEN}Clean complete!${NC}"
        ;;
        
    copy-webview)
        echo -e "${YELLOW}Copying webview files...${NC}"
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        if [ -f "out/webview/chat.css" ] && [ -f "out/webview/chat.js" ]; then
            echo -e "${GREEN}Webview files copied successfully${NC}"
            ls -la out/webview/
        else
            echo -e "${RED}Failed to copy webview files${NC}"
        fi
        ;;
        
    *)
        echo "Usage: ./dev.sh {setup|build|quick-build|watch|package|test|clean|copy-webview}"
        echo ""
        echo "Commands:"
        echo "  setup        - Install dependencies and set up development environment"
        echo "  build        - Full build with native Rust CLI (release mode) and TypeScript"
        echo "  quick-build  - Fast development build (debug mode)"
        echo "  watch        - Start TypeScript watch mode for development"
        echo "  package      - Create VSIX package for distribution"
        echo "  test         - Run tests"
        echo "  clean        - Remove build artifacts"
        echo "  copy-webview - Copy webview HTML/CSS/JS files to output"
        echo ""
        echo "Notes:"
        echo "  - The extension uses the native Rust CLI via subprocess bridge"
        echo "  - CLI binary location: ../target/debug/tycode (debug) or ../target/release/tycode (release)"
        echo "  - Use 'quick-build' for faster development iteration"
        echo "  - Use 'build' for production releases"
        exit 1
        ;;
esac
