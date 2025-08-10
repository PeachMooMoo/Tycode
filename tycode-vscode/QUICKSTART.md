# Quick Start Guide

## Fix "Cannot find module" Error

Run these commands from the `tycode-vscode` directory:

```bash
# Make scripts executable
chmod +x quick-build.sh build-wasm.sh dev.sh

# Run quick build
./quick-build.sh
```

## Alternative: Manual Build

If the script doesn't work, run these commands manually:

```bash
# 1. Install dependencies
npm install

# 2. Create output directories
mkdir -p out/webview
mkdir -p wasm

# 3. Compile TypeScript
npx tsc

# 4. Copy webview files
cp src/webview/*.html out/webview/
cp src/webview/*.css out/webview/
cp src/webview/*.js out/webview/
```

## Testing the Extension

1. After building, press `F5` in VSCode (with the tycode-vscode folder open)
2. A new VSCode window will open with the extension loaded
3. Click the TyCode icon in the activity bar (left sidebar)
4. Or use Command Palette (`Cmd+Shift+P` on Mac, `Ctrl+Shift+P` on Windows/Linux) and search for "TyCode: Open Chat"

## Troubleshooting

### Error: Cannot find module 'typescript'
```bash
npm install --save-dev typescript
```

### Error: Cannot find module 'vscode'
```bash
npm install --save-dev @types/vscode
```

### Error: tsc command not found
```bash
npx tsc
# or install globally
npm install -g typescript
```

## Setting Up API Key

1. Open VSCode Settings (`Cmd+,` on Mac, `Ctrl+,` on Windows/Linux)
2. Search for "tycode"
3. Set your API key in the `tycode.apiKey` field