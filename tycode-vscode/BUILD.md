# Building TyCode VSCode Extension

This guide explains how to build and package the TyCode VSCode extension with the embedded Rust binary.

## Prerequisites

1. **Node.js and npm** - Required for building the TypeScript extension
2. **Rust** - Required for building the native binary
3. **vsce** - VSCode Extension packaging tool (installed via npm)

## Quick Build (Current Platform Only)

To create a VSIX package for your current platform:

```bash
cd tycode-vscode
./dev.sh package
```

This will:
- Build the Rust binary in release mode for your current platform
- Compile the TypeScript code
- Copy webview assets
- Bundle everything into a .vsix file

## Cross-Platform Build

To build binaries for multiple platforms, you need the `cross` tool:

```bash
# Install cross (one-time setup)
cargo install cross

# Then run package
cd tycode-vscode
./dev.sh package
```

With `cross` installed, the package command will attempt to build for:
- macOS x64 (`darwin-x64`)
- macOS ARM64 (`darwin-arm64`)
- Linux x64 (`linux-x64`)
- Windows x64 (`win32-x64`)

## Building Universal macOS Binary

On macOS, you can create a universal binary that works on both Intel and Apple Silicon:

```bash
# First, ensure you have both targets installed
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build universal binary
cd tycode-vscode
./dev.sh build-universal

# Then package
./dev.sh package
```

## Manual Cross-Platform Building

If automatic cross-compilation doesn't work, you can manually build on each platform:

### On macOS (Apple Silicon):
```bash
# Build for ARM64
cargo build --release --target aarch64-apple-darwin
cp ../target/aarch64-apple-darwin/release/tycode binaries/darwin-arm64/

# Build for x64
cargo build --release --target x86_64-apple-darwin
cp ../target/x86_64-apple-darwin/release/tycode binaries/darwin-x64/
```

### On Linux:
```bash
cargo build --release
cp ../target/release/tycode binaries/linux-x64/
```

### On Windows:
```bash
cargo build --release
copy ..\target\release\tycode.exe binaries\win32-x64\
```

## Directory Structure

The extension expects binaries in this structure:

```
tycode-vscode/
├── binaries/
│   ├── darwin-x64/
│   │   └── tycode
│   ├── darwin-arm64/
│   │   └── tycode
│   ├── linux-x64/
│   │   └── tycode
│   └── win32-x64/
│       └── tycode.exe
```

## Installing the VSIX

After building, install the extension:

```bash
# Using VSCode command palette
1. Open Command Palette (Cmd/Ctrl + Shift + P)
2. Run "Extensions: Install from VSIX..."
3. Select the generated .vsix file

# Or using command line
code --install-extension tycode-*.vsix
```

## Development Mode

For development, the extension will automatically use the debug/release binary from the parent directory's target folder, so you don't need to copy binaries during development.

## Troubleshooting

### Binary Not Found
- Ensure the binary is in the correct platform-specific folder
- Check that the binary has execute permissions (chmod +x on Unix systems)
- Verify the extension can find the binary by checking the Developer Console in VSCode

### Cross-Compilation Issues
- Some platforms may require additional setup for cross-compilation
- Consider using GitHub Actions or other CI/CD to build on native platforms
- Docker can be used for Linux builds on other platforms

### Missing Dependencies
- On Linux, ensure you have required system libraries
- On Windows, you may need Visual C++ redistributables
- macOS binaries should be self-contained

## Notes

- The extension automatically detects the current platform and uses the appropriate binary
- Binaries are bundled within the VSIX package for easy distribution
- For security, consider code-signing your binaries before distribution
