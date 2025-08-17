import * as vscode from 'vscode';
import * as path from 'path';
import { SubprocessBridge } from './subprocessBridge';

interface DiffData {
    filePath: string;
    originalContent: string;
    newContent: string;
}

export class ChatProvider implements vscode.WebviewViewProvider {
    private _view?: vscode.WebviewView;
    private _extensionUri: vscode.Uri;
    private _bridge: SubprocessBridge;
    private _messageHistory: Array<{ role: string; content: string }> = [];
    private _diffDataStore: Map<string, DiffData> = new Map();

    constructor(
        private readonly _context: vscode.ExtensionContext,
        bridge: SubprocessBridge
    ) {
        this._extensionUri = _context.extensionUri;
        this._bridge = bridge;

        console.log('[ChatProvider] Constructor - setting up event listeners');
        console.log('[ChatProvider] Bridge listeners before:', this._bridge.eventNames());

        // Set up event listeners for subprocess responses
        this.setupBridgeListeners();

        console.log('[ChatProvider] Bridge listeners after:', this._bridge.eventNames());
        console.log('[ChatProvider] Bridge toolResult listener count:', this._bridge.listenerCount('toolResult'));
    }

    

    private async switchProvider(provider: string): Promise<void> {
        // Send cancel first to stop any ongoing processing
        await this._bridge.sendCancel();
        
        // Then send provider change message
        await this._bridge.changeProvider(provider);

        // Notify webview of the switch
        if (this._view) {
            this._view.webview.postMessage({
                type: 'providerSwitched',
                newProvider: provider
            });
        }
    }

    private setupBridgeListeners() {
        // Listen for settings reload events to update the provider dropdown
        this._bridge.on('settingsReloaded', (settings: any) => {
            if (this._view) {
                const providers = Object.keys(settings.providers || {});
                const selectedProvider = settings.active_provider || 'default';
                
                this._view.webview.postMessage({
                    type: 'providerConfig',
                    providers: providers,
                    selectedProvider: selectedProvider
                });
            }
        });

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

        this._bridge.on('toolResult', (result: any) => {
            console.log('[ChatProvider] Received toolResult event:', result);
            
            // Store diff data if available for file modification tools
            if (result.success && result.result) {
                const toolName = result.tool_name;
                if ((toolName === 'write_file' || toolName === 'replace_in_file' || toolName === 'apply_patch') &&
                    result.result.original_content !== undefined && 
                    result.result.new_content !== undefined) {
                    
                    const diffId = `diff-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
                    this._diffDataStore.set(diffId, {
                        filePath: result.result.path,
                        originalContent: result.result.original_content,
                        newContent: result.result.new_content
                    });
                    
                    // Add diffId to result for the webview
                    result.diffId = diffId;
                }
            }
            
            if (this._view) {
                const message = {
                    type: 'toolResult',
                    toolName: result.tool_name,
                    success: result.success,
                    result: result.result,
                    error: result.error,
                    diffId: result.diffId
                };
                console.log('[ChatProvider] Posting to webview:', message);
                this._view.webview.postMessage(message);
            } else {
                console.log('[ChatProvider] No view available to post toolResult');
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
                    this._view.webview.postMessage({ type: 'hideTyping' });
                } else if (event === 'retry_attempt') {
                    // Handle retry attempts
                    this._view.webview.postMessage({
                        type: 'retryAttempt',
                        attempt: data.attempt,
                        maxRetries: data.max_retries,
                        error: data.error,
                        backoffMs: data.backoff_ms
                    });
                }
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

        // Provider configuration will be loaded from subprocess when needed

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
                case 'viewDiff':
                    await this.showDiff(data.diffId);
                    break;
                case 'cancel':
                    await this._bridge.sendCancel();
                    break;
                case 'switchProvider':
                    await this.switchProvider(data.provider);
                    break;
                case 'getProviders':
                    // First reload settings from disk to ensure we have the latest
                    try {
                        await this._bridge.reloadSettings();
                        // Now get the fresh settings from cache
                        const settings = this._bridge.getSettings();
                        if (settings) {
                            const providers = Object.keys(settings.providers || {});
                            const selectedProvider = settings.active_provider || 'default';
                            
                            this._view?.webview.postMessage({
                                type: 'providerConfig',
                                providers: providers,
                                selectedProvider: selectedProvider
                            });
                        }
                    } catch (error) {
                        console.error('Failed to reload providers:', error);
                        // Fallback to cached settings
                        const settings = this._bridge.getSettings();
                        if (settings) {
                            const providers = Object.keys(settings.providers || {});
                            const selectedProvider = settings.active_provider || 'default';
                            
                            this._view?.webview.postMessage({
                                type: 'providerConfig',
                                providers: providers,
                                selectedProvider: selectedProvider
                            });
                        } else {
                            // No settings available yet
                            this._view?.webview.postMessage({
                                type: 'providerConfig',
                                providers: ['default'],
                                selectedProvider: 'default'
                            });
                        }
                    }
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
        console.log('[ChatProvider] Sending showTyping message');
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

    private async showDiff(diffId: string) {
        const diffData = this._diffDataStore.get(diffId);
        if (!diffData) {
            vscode.window.showWarningMessage('Diff data not found');
            return;
        }

        // Create URIs for the diff
        const originalUri = vscode.Uri.parse(`tycode-diff:before/${diffData.filePath}?${diffId}`);
        const modifiedUri = vscode.Uri.parse(`tycode-diff:after/${diffData.filePath}?${diffId}`);

        // Register a text document content provider for the diff
        const provider = new class implements vscode.TextDocumentContentProvider {
            constructor(private data: DiffData) {}
            
            provideTextDocumentContent(uri: vscode.Uri): string {
                if (uri.scheme === 'tycode-diff') {
                    if (uri.authority === 'before') {
                        return this.data.originalContent;
                    } else if (uri.authority === 'after') {
                        return this.data.newContent;
                    }
                }
                return '';
            }
        }(diffData);

        // Register the provider temporarily
        const disposable = vscode.workspace.registerTextDocumentContentProvider('tycode-diff', provider);

        // Open the diff editor
        const title = `Changes to ${path.basename(diffData.filePath)}`;
        await vscode.commands.executeCommand(
            'vscode.diff',
            originalUri,
            modifiedUri,
            title,
            { preview: true }
        );

        // Clean up after a delay (keep it alive for a while in case user switches tabs)
        setTimeout(() => {
            disposable.dispose();
        }, 300000); // 5 minutes
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
                        <button id="cancel-button" class="cancel-button" style="display: none;">Cancel</button>
                    </div>
                    <div class="provider-selector">
                        <label for="provider-select">Provider:</label>
                        <select id="provider-select">
                            <!-- Options will be populated dynamically -->
                        </select>
                        <button id="refresh-providers" class="refresh-button" title="Refresh providers">↻</button>
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
