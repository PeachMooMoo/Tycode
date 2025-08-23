#!/bin/bash

# Development helper script for TyCode

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}TyCode Development Helper${NC}"
echo "========================="
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
        cd tycode-vscode
        npm install
        cd ..
        
        echo -e "${GREEN}Setup complete!${NC}"
        ;;
        
    build)
        echo -e "${YELLOW}Building extension with subprocess bridge...${NC}"
        
        # Build native Rust CLI
        echo -e "${YELLOW}Building native Rust CLI...${NC}"
        cargo build --release
        
        # Build TypeScript
        echo -e "${YELLOW}Compiling TypeScript...${NC}"
        cd tycode-vscode
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
        
        cd ..
        
        echo -e "${GREEN}Build complete!${NC}"
        echo -e "${YELLOW}Note: The extension uses the native CLI at ./target/release/tycode${NC}"
        ;;
        
    quick-build)
        echo -e "${YELLOW}Quick building extension (debug mode)...${NC}"
        
        # Build native Rust CLI in debug mode
        echo -e "${YELLOW}Building native Rust CLI (debug mode)...${NC}"
        cargo build
        
        # Build TypeScript
        echo -e "${YELLOW}Compiling TypeScript...${NC}"
        cd tycode-vscode
        npm run compile
        
        # Copy webview assets
        echo -e "${YELLOW}Copying webview files...${NC}"
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        cd ..
        
        echo -e "${GREEN}Quick build complete!${NC}"
        echo -e "${YELLOW}Note: The extension uses the native CLI at ./target/debug/tycode${NC}"
        ;;
        
    watch)
        echo -e "${YELLOW}Starting watch mode...${NC}"
        
        # First do a full build to ensure everything is copied
        echo -e "${YELLOW}Doing initial build...${NC}"
        cd tycode-vscode
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
        
        # Create binaries directory structure
        echo -e "${YELLOW}Creating binaries directory structure...${NC}"
        cd tycode-vscode
        rm -rf binaries
        mkdir -p binaries/{darwin-x64,darwin-arm64,linux-x64,win32-x64}
        cd ..
        
        # Build native Rust CLI for current platform first
        echo -e "${YELLOW}Building native Rust CLI (release mode)...${NC}"
        cargo build --release
        
        # Detect current platform and copy binary
        CURRENT_PLATFORM=$(uname -s)
        CURRENT_ARCH=$(uname -m)
        
        if [ "$CURRENT_PLATFORM" = "Darwin" ]; then
            if [ "$CURRENT_ARCH" = "arm64" ]; then
                echo -e "${GREEN}Copying macOS ARM64 binary...${NC}"
                cp target/release/tycode tycode-vscode/binaries/darwin-arm64/tycode
                chmod +x tycode-vscode/binaries/darwin-arm64/tycode
            else
                echo -e "${GREEN}Copying macOS x64 binary...${NC}"
                cp target/release/tycode tycode-vscode/binaries/darwin-x64/tycode
                chmod +x tycode-vscode/binaries/darwin-x64/tycode
            fi
        elif [ "$CURRENT_PLATFORM" = "Linux" ]; then
            echo -e "${GREEN}Copying Linux x64 binary...${NC}"
            cp target/release/tycode tycode-vscode/binaries/linux-x64/tycode
            chmod +x tycode-vscode/binaries/linux-x64/tycode
        fi
        
        # Try to build for other platforms if cross is installed
        if command -v cross &> /dev/null; then
            echo -e "${YELLOW}Cross compilation tool found, building for other platforms...${NC}"
            
            # Build for other platforms based on current platform
            if [ "$CURRENT_PLATFORM" = "Darwin" ]; then
                # On macOS, try to build universal binary
                if [ "$CURRENT_ARCH" = "arm64" ]; then
                    echo -e "${YELLOW}Building for macOS x64...${NC}"
                    cargo build --release --target x86_64-apple-darwin 2>/dev/null && \
                        cp target/x86_64-apple-darwin/release/tycode tycode-vscode/binaries/darwin-x64/tycode && \
                        chmod +x tycode-vscode/binaries/darwin-x64/tycode || \
                        echo -e "${YELLOW}Warning: Could not build for macOS x64${NC}"
                else
                    echo -e "${YELLOW}Building for macOS ARM64...${NC}"
                    cargo build --release --target aarch64-apple-darwin 2>/dev/null && \
                        cp target/aarch64-apple-darwin/release/tycode tycode-vscode/binaries/darwin-arm64/tycode && \
                        chmod +x tycode-vscode/binaries/darwin-arm64/tycode || \
                        echo -e "${YELLOW}Warning: Could not build for macOS ARM64${NC}"
                fi
                
                # Try Linux cross-compilation
                echo -e "${YELLOW}Building for Linux x64...${NC}"
                cross build --release --target x86_64-unknown-linux-gnu 2>/dev/null && \
                    cp target/x86_64-unknown-linux-gnu/release/tycode tycode-vscode/binaries/linux-x64/tycode && \
                    chmod +x tycode-vscode/binaries/linux-x64/tycode || \
                    echo -e "${YELLOW}Warning: Could not build for Linux x64${NC}"
                    
                # Try Windows cross-compilation
                echo -e "${YELLOW}Building for Windows x64...${NC}"
                cross build --release --target x86_64-pc-windows-gnu 2>/dev/null && \
                    cp target/x86_64-pc-windows-gnu/release/tycode.exe tycode-vscode/binaries/win32-x64/tycode.exe || \
                    echo -e "${YELLOW}Warning: Could not build for Windows x64${NC}"
            fi
        else
            echo -e "${YELLOW}Note: Install 'cross' for cross-platform compilation${NC}"
            echo -e "${YELLOW}  cargo install cross${NC}"
        fi
        
        cd tycode-vscode
        
        echo -e "${YELLOW}Compiling TypeScript...${NC}"
        npm run compile
        
        echo -e "${YELLOW}Copying webview files...${NC}"
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        # Show which binaries were included
        echo -e "${GREEN}Binaries included in package:${NC}"
        ls -la binaries/*/tycode* 2>/dev/null || echo -e "${YELLOW}No binaries found${NC}"
        
        echo -e "${YELLOW}Creating VSIX package...${NC}"
        npm run package
        
        if [ -f *.vsix ]; then
            echo -e "${GREEN}Package created successfully!${NC}"
            ls -la *.vsix
        else
            echo -e "${RED}Failed to create package${NC}"
        fi
        
        cd ..
        ;;
        
    test)
        echo -e "${YELLOW}Running all tests...${NC}"
        
        # Run Rust tests
        echo -e "${YELLOW}Running Rust tests...${NC}"
        cargo test
        
        # Run npm tests
        echo -e "${YELLOW}Running VSCode extension tests...${NC}"
        cd tycode-vscode
        npm test
        cd ..
        
        echo -e "${GREEN}All tests complete!${NC}"
        ;;
        
    clean)
        echo -e "${YELLOW}Cleaning build artifacts...${NC}"
        
        # Clean TypeScript/Node artifacts
        cd tycode-vscode
        rm -rf out/
        rm -rf node_modules/
        rm -f *.vsix
        cd ..
        
        # Clean Rust build
        cargo clean
        
        echo -e "${GREEN}Clean complete!${NC}"
        ;;
        
    copy-webview)
        echo -e "${YELLOW}Copying webview files...${NC}"
        cd tycode-vscode
        mkdir -p out/webview
        cp src/webview/*.css out/webview/ 2>/dev/null || true
        cp src/webview/*.js out/webview/ 2>/dev/null || true
        
        if [ -f "out/webview/chat.css" ] && [ -f "out/webview/chat.js" ]; then
            echo -e "${GREEN}Webview files copied successfully${NC}"
            ls -la out/webview/
        else
            echo -e "${RED}Failed to copy webview files${NC}"
        fi
        cd ..
        ;;
        
    build-universal)
        echo -e "${YELLOW}Building universal macOS binary...${NC}"
        
        if [ "$(uname -s)" != "Darwin" ]; then
            echo -e "${RED}Universal binary can only be built on macOS${NC}"
            exit 1
        fi
        
        # Build for both architectures
        echo -e "${YELLOW}Building for x86_64...${NC}"
        cargo build --release --target x86_64-apple-darwin
        
        echo -e "${YELLOW}Building for aarch64...${NC}"
        cargo build --release --target aarch64-apple-darwin
        
        # Create universal binary
        echo -e "${YELLOW}Creating universal binary...${NC}"
        mkdir -p tycode-vscode/binaries/darwin-universal
        lipo -create \
            target/x86_64-apple-darwin/release/tycode \
            target/aarch64-apple-darwin/release/tycode \
            -output tycode-vscode/binaries/darwin-universal/tycode
        
        chmod +x tycode-vscode/binaries/darwin-universal/tycode
        
        echo -e "${GREEN}Universal binary created!${NC}"
        file tycode-vscode/binaries/darwin-universal/tycode
        ;;
        
    *)
        echo "Usage: ./dev.sh {setup|build|quick-build|watch|package|test|clean|copy-webview|build-universal}"
        echo ""
        echo "Commands:"
        echo "  setup           - Install dependencies and set up development environment"
        echo "  build           - Full build with native Rust CLI (release mode) and TypeScript"
        echo "  quick-build     - Fast development build (debug mode)"
        echo "  watch           - Start TypeScript watch mode for development"
        echo "  package         - Create VSIX package for distribution"
        echo "  test            - Run all tests (Rust and VSCode extension)"
        echo "  clean           - Remove build artifacts"
        echo "  copy-webview    - Copy webview HTML/CSS/JS files to output"
        echo "  build-universal - Build universal macOS binary (macOS only)"
        echo ""
        echo "Notes:"
        echo "  - The extension uses the native Rust CLI via subprocess bridge"
        echo "  - CLI binary location: ./target/debug/tycode (debug) or ./target/release/tycode (release)"
        echo "  - Use 'quick-build' for faster development iteration"
        echo "  - Use 'build' for production releases"
        echo "  - For cross-platform packaging, install: cargo install cross"
        exit 1
        ;;
esac