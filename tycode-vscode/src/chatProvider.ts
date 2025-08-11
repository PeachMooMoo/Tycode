import * as vscode from 'vscode';
import * as path from 'path';
import { SubprocessBridge } from './subprocessBridge';

export class ChatProvider implements vscode.WebviewViewProvider {
    private _view?: vscode.WebviewView;
    private _extensionUri: vscode.Uri;
    private _bridge: SubprocessBridge;
    private _messageHistory: Array<{ role: string; content: string }> = [];

    constructor(
        private readonly _context: vscode.ExtensionContext,
        bridge: SubprocessBridge
    ) {
        this._extensionUri = _context.extensionUri;
        this._bridge = bridge;

        // Set up event listeners for subprocess responses
        this._bridge.on('response', (response: any) => {
            if (this._view) {
                // Display the main response with embedded reasoning and tool calls
                this._view.webview.postMessage({
                    type: 'displayMessage',
                    role: 'assistant',
                    content: response.content,
                    reasoning: response.reasoning,
                    toolCalls: response.tool_calls || [],
                    model: response.model,
                    isComplete: response.is_complete,
                    tokenUsage: response.token_usage
                });

                this._view.webview.postMessage({ type: 'hideTyping' });
                this._messageHistory.push({ role: 'assistant', content: response.content });
            }
        });

        this._bridge.on('event', (event: string, data: any) => {
            if (this._view) {
                if (event === 'system') {
                    this._view.webview.postMessage({
                        type: 'displayMessage',
                        role: 'system',
                        content: data.content
                    });
                }
                // Note: typing events are no longer sent, tool_calls are in Response
            }
        });

        this._bridge.on('error', (error: string) => {
            if (this._view) {
                this._view.webview.postMessage({
                    type: 'displayMessage',
                    role: 'error',
                    content: error
                });
                this._view.webview.postMessage({ type: 'hideTyping' });
            }
        });

        this._bridge.on('disconnected', () => {
            if (this._view) {
                this._view.webview.postMessage({
                    type: 'displayMessage',
                    role: 'error',
                    content: 'Connection to backend lost. Please restart the extension.'
                });
            }
        });
    }

    public resolveWebviewView(
        webviewView: vscode.WebviewView,
        context: vscode.WebviewViewResolveContext,
        _token: vscode.CancellationToken
    ) {
        this._view = webviewView;

        webviewView.webview.options = {
            enableScripts: true,
            localResourceRoots: [this._extensionUri]
        };

        webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);

        // Handle messages from the webview
        webviewView.webview.onDidReceiveMessage(async data => {
            switch (data.type) {
                case 'sendMessage':
                    await this.handleUserMessage(data.message);
                    break;
                case 'clear':
                    this._messageHistory = [];
                    break;
                case 'copyCode':
                    await vscode.env.clipboard.writeText(data.code);
                    vscode.window.showInformationMessage('Code copied to clipboard');
                    break;
                case 'insertCode':
                    await this.insertCodeInEditor(data.code);
                    break;
            }
        });
    }

    public async openChat() {
        if (!this._view) {
            await vscode.commands.executeCommand('tycode.chatView.focus');
        } else {
            this._view.show?.(true);
        }
    }

    public async sendMessage(message: string) {
        if (!this._view) {
            await this.openChat();
            // Wait a bit for the view to initialize
            await new Promise(resolve => setTimeout(resolve, 100));
        }

        if (this._view) {
            this._view.webview.postMessage({
                type: 'displayMessage',
                role: 'user',
                content: message
            });
            await this.handleUserMessage(message);
        }
    }

    private async handleUserMessage(message: string) {
        if (!this._view) return;

        // Add user message to history
        this._messageHistory.push({ role: 'user', content: message });

        // Show typing indicator
        this._view.webview.postMessage({ type: 'showTyping' });

        try {
            // Send message to the subprocess
            await this._bridge.sendMessage(message);
            // The response will be handled by the event listeners set up in the constructor
        } catch (error) {
            const errorMessage = error instanceof Error ? error.message : 'An unknown error occurred';
            this._view.webview.postMessage({
                type: 'displayMessage',
                role: 'error',
                content: errorMessage
            });
            this._view.webview.postMessage({ type: 'hideTyping' });
        }
    }

    private async insertCodeInEditor(code: string) {
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            vscode.window.showWarningMessage('No active editor');
            return;
        }

        const position = editor.selection.active;
        await editor.edit(editBuilder => {
            editBuilder.insert(position, code);
        });
    }

    private _getHtmlForWebview(webview: vscode.Webview) {
        const scriptUri = webview.asWebviewUri(
            vscode.Uri.joinPath(this._extensionUri, 'out', 'webview', 'chat.js')
        );
        const styleUri = webview.asWebviewUri(
            vscode.Uri.joinPath(this._extensionUri, 'out', 'webview', 'chat.css')
        );

        const nonce = this.getNonce();

        return `<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
                <link href="${styleUri}" rel="stylesheet">
                <title>TyCode Chat</title>
            </head>
            <body>
                <div class="chat-container">
                    <div class="chat-header">
                        <h3>TyCode Assistant</h3>
                        <button id="clear-chat" class="header-button" title="Clear chat">🗑️</button>
                    </div>
                    <div id="messages" class="messages"></div>
                    <div id="typing-indicator" class="typing-indicator" style="display: none;">
                        <span></span>
                        <span></span>
                        <span></span>
                    </div>
                    <div class="input-container">
                        <textarea 
                            id="message-input" 
                            class="message-input" 
                            placeholder="Ask me anything about your code..."
                            rows="3"
                        ></textarea>
                        <button id="send-button" class="send-button">Send</button>
                    </div>
                </div>
                <script nonce="${nonce}" src="${scriptUri}"></script>
            </body>
            </html>`;
    }

    private getNonce() {
        let text = '';
        const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
        for (let i = 0; i < 32; i++) {
            text += possible.charAt(Math.floor(Math.random() * possible.length));
        }
        return text;
    }
}
