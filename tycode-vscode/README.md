# TyCode VSCode Extension

AI-powered coding assistant for Visual Studio Code.

## Features

- **Chat Interface**: Interactive chat panel in the sidebar for conversing with the AI assistant
- **Code Selection**: Select code and ask questions about it via context menu
- **Code Insertion**: Insert AI-generated code directly into your editor
- **Syntax Highlighting**: Proper code formatting in chat responses
- **Copy/Insert Actions**: Quick actions to copy or insert code snippets from responses

## Development Setup

### Prerequisites

1. Node.js (v16 or higher)
2. Rust toolchain with wasm-pack
3. VSCode

### Install Dependencies

```bash
# Install Node dependencies
npm install

# Install wasm-pack (if not already installed)
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### Build the Extension

```bash
# Build WASM module from tycode-core
npm run build:wasm

# Build the extension
npm run build

# Or for development with watch mode
npm run watch
```

### Testing in VSCode

1. Open the `tycode-vscode` folder in VSCode
2. Press `F5` to launch a new VSCode window with the extension loaded
3. Open the TyCode sidebar panel or use `Ctrl+Shift+P` and run "TyCode: Open Chat"

## Configuration

Set your API key and preferences in VSCode settings:

- `tycode.apiKey`: Your API key for the AI service
- `tycode.model`: The AI model to use (default: claude-3-5-sonnet-20241022)
- `tycode.maxTokens`: Maximum tokens for AI responses (default: 4096)

## Building for Production

```bash
# Build and package the extension
npm run build
npm run package

# This creates a .vsix file that can be installed
```

## Installing the Extension

### From VSIX file:
1. Open VSCode
2. Go to Extensions view (`Ctrl+Shift+X`)
3. Click the "..." menu and select "Install from VSIX..."
4. Select the generated `.vsix` file

### From source:
1. Copy the built extension to your VSCode extensions folder:
   - Windows: `%USERPROFILE%\.vscode\extensions`
   - macOS/Linux: `~/.vscode/extensions`
2. Restart VSCode

## Architecture

The extension consists of three main components:

1. **TypeScript Extension Host**: Manages VSCode integration, commands, and UI
2. **WASM Module**: Core logic compiled from Rust for processing chat messages
3. **Webview UI**: HTML/CSS/JS chat interface

### Communication Flow:
```
User Input → Webview → Extension Host → WASM Module → AI Service
                ↑                             ↓
                └──────── Response ←──────────┘
```

## Troubleshooting

### WASM module not loading
- Ensure `tycode-core` is compiled with `wasm-pack build`
- Check that WASM files are in the `wasm/` directory
- Verify webpack is copying WASM files to output directory

### Chat not responding
- Check that API key is set in settings
- Verify network connection for AI service
- Check developer console for error messages

## Contributing

See the main project README for contribution guidelines.

## License

Same as the main TyCode project.