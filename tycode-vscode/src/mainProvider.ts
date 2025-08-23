import * as vscode from 'vscode';
import { ConversationManager } from './conversationManager';
import { Conversation } from './conversation';
import * as path from 'path';
import { 
    ConversationMessage, 
    ToolResultEvent, 
    MANAGER_EVENTS 
} from './events';

// Import build info - will be generated at build time
let buildInfo = { buildTime: 'dev', timestamp: new Date().toISOString() };
try {
    const buildModule = require('./build-info');
    buildInfo = buildModule.buildInfo;
} catch (e) {
    // Build info not available in dev mode
}

interface DiffData {
    filePath: string;
    originalContent: string;
    newContent: string;
}

export class MainProvider implements vscode.WebviewViewProvider {
    private _view?: vscode.WebviewView;
    private conversationManager: ConversationManager;
    private _diffDataStore: Map<string, DiffData> = new Map();

    constructor(
        private readonly context: vscode.ExtensionContext
    ) {
        this.conversationManager = new ConversationManager(context);
        this.setupConversationListeners();
    }

    private setupConversationListeners(): void {
        this.conversationManager.on(MANAGER_EVENTS.CONVERSATION_CREATED, (conversation: Conversation) => {
            this.sendToWebview({
                type: 'conversationCreated',
                id: conversation.id,
                title: conversation.title
            });
            
            });

        this.conversationManager.on(MANAGER_EVENTS.CONVERSATION_UPDATE, (id: string, updateType: string, data: any) => {
            // IMPORTANT: Special handling for toolResult events!
            // Tool results need special processing to extract diff data for file
            // modifications. This is different from other message types which are
            // passed through directly. The data parameter here is the raw 
            // ToolResultEvent from the subprocess, NOT a ConversationMessage.
            if (updateType === 'toolResult') {
                console.log('[MainProvider] Processing toolResult:', id, data);
                
                // Cast to proper type - this is a ToolResultEvent, not a ConversationMessage
                const toolResult = data as ToolResultEvent;
                
                // Check if this is a file modification with diff data
                let diffId: string | undefined;
                if (toolResult.success && toolResult.result) {
                    const toolName = toolResult.tool_name;
                    if ((toolName === 'write_file' || toolName === 'replace_in_file' || toolName === 'apply_patch') &&
                        toolResult.result.original_content !== undefined &&
                        toolResult.result.new_content !== undefined) {
                        
                        // Generate unique ID for this diff and store it
                        diffId = `diff-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
                        console.log('[MainProvider] Storing diff with ID:', diffId);
                        this._diffDataStore.set(diffId, {
                            filePath: toolResult.result.path,
                            originalContent: toolResult.result.original_content,
                            newContent: toolResult.result.new_content
                        });
                    }
                }
                
                // Send tool result to webview with optional diffId
                this.sendToWebview({
                    type: 'toolResult',
                    conversationId: id,
                    toolName: toolResult.tool_name,
                    success: toolResult.success,
                    result: toolResult.result,
                    error: toolResult.error,
                    diffId: diffId
                });
                return;
            }
            
            // Handle typing status events
            if (updateType === 'typing_status') {
                console.log('[MainProvider] Received typing_status:', id, data);
                
                // Check if this is actually a retry attempt event
                if (data.event_type === 'retry_attempt') {
                    console.log('[MainProvider] Processing retry_attempt:', data);
                    this.sendToWebview({
                        type: 'retryAttempt',
                        conversationId: id,
                        attempt: data.attempt,
                        maxRetries: data.max_retries,
                        error: data.error,
                        backoffMs: data.backoff_ms
                    });
                    return;
                }
                
                // Standard typing status
                this.sendToWebview({
                    type: 'showTyping',
                    conversationId: id,
                    show: data.is_typing
                });
                return;
            }
            
            // Standard message handling for non-toolResult events
            const message = data as ConversationMessage;
            
            this.sendToWebview({
                type: 'conversationMessage',
                conversationId: id,
                messageType: updateType,
                message
            });
        });

        this.conversationManager.on(MANAGER_EVENTS.CONVERSATION_TITLE_CHANGED, (id: string, title: string) => {
            this.sendToWebview({
                type: 'conversationTitleChanged',
                id,
                title
            });
        });

        this.conversationManager.on(MANAGER_EVENTS.ACTIVE_CONVERSATION_CHANGED, (id: string) => {
            this.sendToWebview({
                type: 'activeConversationChanged',
                id
            });
        });

        this.conversationManager.on(MANAGER_EVENTS.CONVERSATION_CLOSED, (id: string) => {
            this.sendToWebview({
                type: 'conversationClosed',
                id
            });
        });

        this.conversationManager.on(MANAGER_EVENTS.CONVERSATION_DISCONNECTED, (id: string) => {
            this.sendToWebview({
                type: 'conversationDisconnected',
                id
            });
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
            localResourceRoots: [this.context.extensionUri]
        };

        webviewView.webview.html = this.getHtmlForWebview(webviewView.webview);

        // Handle messages from the webview
        webviewView.webview.onDidReceiveMessage(async data => {
            console.log('[MainProvider] Received message from webview:', data);
            switch (data.type) {
                case 'newChat':
                    await this.handleNewChat();
                    break;
                case 'openSettings':
                    await this.handleOpenSettings();
                    break;
                case 'sendMessage':
                    await this.handleSendMessage(data.conversationId, data.message);
                    break;
                case 'switchTab':
                    this.handleSwitchTab(data.conversationId);
                    break;
                case 'closeTab':
                    this.handleCloseTab(data.conversationId);
                    break;
                case 'renameTab':
                    this.handleRenameTab(data.conversationId, data.title);
                    break;
                case 'clearChat':
                    this.handleClearChat(data.conversationId);
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
                    await this.handleCancel(data.conversationId);
                    break;
                case 'switchProvider':
                    await this.handleSwitchProvider(data.conversationId, data.provider);
                    break;
                case 'getProviders':
                    // Just get cached providers, no reload
                    this.handleGetCachedProviders(data.conversationId);
                    break;
                case 'refreshProviders':
                    // Force reload from disk
                    await this.handleRefreshProviders(data.conversationId);
                    break;
            }
        });

        // Send initial state
        this.sendInitialState();
    }

    private async sendInitialState(): Promise<void> {
        const conversations = this.conversationManager.getAllConversations();
        const activeConversation = this.conversationManager.getActiveConversation();

        this.sendToWebview({
            type: 'initialState',
            conversations: conversations.map(c => ({
                id: c.id,
                title: c.title,
                messages: c.messages,
                selectedProvider: c.selectedProvider
            })),
            activeConversationId: activeConversation?.id || null
        });

        // On initial load, just get cached settings without reloading from disk
        for (const c of conversations) {
            if (c.bridge) {
                const settings = c.bridge.getSettings();
                if (settings) {
                    const providers = Object.keys(settings.providers || {});
                    const selectedProvider = settings.active_provider;
                    
                    this.sendToWebview({
                        type: 'providerConfig',
                        conversationId: c.id,
                        providers,
                        selectedProvider
                    });
                }
            }
        }
    }

    private async handleNewChat(): Promise<void> {
        try {
            const conversation = await this.conversationManager.createConversation();
            this.sendToWebview({
                type: 'showTyping',
                conversationId: conversation.id,
                show: false
            });
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to create new chat: ${error}`);
        }
    }

    private async handleOpenSettings(): Promise<void> {
        await vscode.commands.executeCommand('tycode.openSettings');
    }

    private async handleSendMessage(conversationId: string, message: string): Promise<void> {
        const conversation = this.conversationManager.getConversation(conversationId);
        if (!conversation) {
            vscode.window.showErrorMessage('Conversation not found');
            return;
        }

        try {
            await conversation.sendMessage(message);
        } catch (error) {
            vscode.window.showErrorMessage(`Failed to send message: ${error}`);
        }
    }

    private handleSwitchTab(conversationId: string): void {
        this.conversationManager.setActiveConversation(conversationId);
    }

    private handleCloseTab(conversationId: string): void {
        this.conversationManager.closeConversation(conversationId);
    }

    private handleRenameTab(conversationId: string, title: string): void {
        const conversation = this.conversationManager.getConversation(conversationId);
        if (conversation) {
            conversation.title = title;
        }
    }

    private handleClearChat(conversationId: string): void {
        const conversation = this.conversationManager.getConversation(conversationId);
        if (conversation) {
            conversation.clearMessages();
            this.sendToWebview({
                type: 'conversationCleared',
                conversationId
            });
        }
    }

    private async handleCancel(conversationId: string): Promise<void> {
        const conversation = this.conversationManager.getConversation(conversationId);
        if (!conversation) {
            return;
        }

        try {
            await conversation.sendCancel();
            // Hide typing indicator immediately when cancel is successful
            this.sendToWebview({
                type: 'showTyping',
                conversationId,
                show: false
            });
        } catch (error) {
            console.error('[MainProvider] Failed to cancel:', error);
        }
    }

    private async handleSwitchProvider(conversationId: string, provider: string): Promise<void> {
        const conversation = this.conversationManager.getConversation(conversationId);
        if (!conversation) {
            return;
        }

        try {
            await conversation.switchProvider(provider);
            
            this.sendToWebview({
                type: 'providerSwitched',
                conversationId,
                newProvider: provider
            });
        } catch (error) {
            console.error('[MainProvider] Failed to switch provider:', error);
            vscode.window.showErrorMessage(`Failed to switch provider: ${error}`);
        }
    }

    private handleGetCachedProviders(conversationId: string): void {
        const conversation = this.conversationManager.getConversation(conversationId);
        
        // Just get cached settings, no reload
        if (conversation && conversation.bridge) {
            const settings = conversation.bridge.getSettings();
            if (settings) {
                const providers = Object.keys(settings.providers || {});
                const selectedProvider = settings.active_provider;
                
                this.sendToWebview({
                    type: 'providerConfig',
                    conversationId,
                    providers,
                    selectedProvider
                });
                return;
            }
        }
        
        // No settings available yet - send empty response
        this.sendToWebview({
            type: 'providerConfig',
            conversationId,
            providers: [],
            selectedProvider: null
        });
    }

    private async handleRefreshProviders(conversationId: string): Promise<void> {
        const conversation = this.conversationManager.getConversation(conversationId);
        
        if (conversation && conversation.bridge) {
            try {
                // Force reload settings from disk to get the latest
                await conversation.bridge.reloadSettings();
                
                // Now get the fresh settings from cache
                const settings = conversation.bridge.getSettings();
                if (settings) {
                    const providers = Object.keys(settings.providers || {});
                    const selectedProvider = settings.active_provider;
                    
                    this.sendToWebview({
                        type: 'providerConfig',
                        conversationId,
                        providers,
                        selectedProvider
                    });
                    return;
                }
            } catch (error) {
                console.error('[MainProvider] Failed to reload providers:', error);
            }
        }
        
        // No settings available yet or error occurred - send empty response
        this.sendToWebview({
            type: 'providerConfig',
            conversationId,
            providers: [],
            selectedProvider: null
        });
    }

    private async insertCodeInEditor(code: string): Promise<void> {
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

    private async showDiff(diffId: string): Promise<void> {
        console.log('[MainProvider] showDiff called with diffId:', diffId);
        const diffData = this._diffDataStore.get(diffId);
        if (!diffData) {
            console.error('[MainProvider] Diff data not found for diffId:', diffId);
            vscode.window.showWarningMessage('Diff data not found');
            return;
        }
        
        console.log('[MainProvider] Diff data found:', {
            filePath: diffData.filePath,
            originalLength: diffData.originalContent?.length,
            newLength: diffData.newContent?.length
        });

        // Create URIs for the diff - use :// to properly set authority
        const originalUri = vscode.Uri.parse(`tycode-diff://before/${diffData.filePath}?${diffId}`);
        const modifiedUri = vscode.Uri.parse(`tycode-diff://after/${diffData.filePath}?${diffId}`);

        // Register a text document content provider for the diff
        const provider = new class implements vscode.TextDocumentContentProvider {
            constructor(private data: DiffData) {}
            
            provideTextDocumentContent(uri: vscode.Uri): string {
                console.log('[MainProvider] provideTextDocumentContent called');
                console.log('[MainProvider] URI scheme:', uri.scheme);
                console.log('[MainProvider] URI authority:', uri.authority);
                console.log('[MainProvider] URI path:', uri.path);
                console.log('[MainProvider] Full URI:', uri.toString());
                
                if (uri.scheme === 'tycode-diff') {
                    if (uri.authority === 'before') {
                        console.log('[MainProvider] Returning original content, length:', this.data.originalContent?.length);
                        return this.data.originalContent;
                    } else if (uri.authority === 'after') {
                        console.log('[MainProvider] Returning new content, length:', this.data.newContent?.length);
                        return this.data.newContent;
                    }
                }
                console.log('[MainProvider] No content match for URI');
                return '';
            }
        }(diffData);

        // Register the provider temporarily
        const disposable = vscode.workspace.registerTextDocumentContentProvider('tycode-diff', provider);

        try {
            // Open the diff editor
            const title = `Changes to ${path.basename(diffData.filePath)}`;
            console.log('[MainProvider] Opening diff editor with title:', title);
            await vscode.commands.executeCommand(
                'vscode.diff',
                originalUri,
                modifiedUri,
                title,
                { preview: true }
            );
        } catch (error) {
            console.error('[MainProvider] Error opening diff editor:', error);
            vscode.window.showErrorMessage('Failed to open diff: ' + (error as Error).message);
        }

        // Clean up after a delay (keep it alive for a while in case user switches tabs)
        setTimeout(() => {
            console.log('[MainProvider] Disposing diff provider for:', diffId);
            disposable.dispose();
        }, 300000); // 5 minutes
    }

    private sendToWebview(message: any): void {
        if (this._view) {
            this._view.webview.postMessage(message);
        }
    }

    public async openChat(): Promise<void> {
        if (!this._view) {
            await vscode.commands.executeCommand('tycode.chatView.focus');
        } else {
            this._view.show?.(true);
        }
    }

    public async sendMessageToActiveChat(message: string): Promise<void> {
        let conversation = this.conversationManager.getActiveConversation();
        if (!conversation) {
            // Create a new chat if none exists (without a default title)
            try {
                conversation = await this.conversationManager.createConversation();
                this.sendToWebview({
                    type: 'showTyping',
                    conversationId: conversation.id,
                    show: false
                });
            } catch (error) {
                vscode.window.showErrorMessage(`Failed to create new chat: ${error}`);
                return;
            }
        }
        
        if (conversation) {
            await this.handleSendMessage(conversation.id, message);
        }
    }

    private getHtmlForWebview(webview: vscode.Webview): string {
        const scriptUri = webview.asWebviewUri(
            vscode.Uri.joinPath(this.context.extensionUri, 'out', 'webview', 'main.js')
        );
        const styleUri = webview.asWebviewUri(
            vscode.Uri.joinPath(this.context.extensionUri, 'out', 'webview', 'main.css')
        );

        const nonce = this.getNonce();

        return `<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <meta name="viewport" content="width=device-width, initial-scale=1.0">
                <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}';">
                <link href="${styleUri}" rel="stylesheet">
                <title>TyCode</title>
            </head>
            <body>
                <div class="main-container">
                    <!-- Tab bar (hidden when no conversations) -->
                    <div id="tab-bar" class="tab-bar" style="display: none;">
                        <div id="tabs" class="tabs"></div>
                        <button id="new-tab-button" class="new-tab-button" title="New Chat">+</button>
                    </div>

                    <!-- Welcome screen (shown when no conversations) -->
                    <div id="welcome-screen" class="welcome-screen">
                        <div class="welcome-content">
                            <div class="tiger-emoji">🐯</div>
                            <h1 class="welcome-title">TyCode</h1>
                            <div class="welcome-buttons">
                                <button id="welcome-new-chat" class="welcome-button primary">New Chat</button>
                                <button id="welcome-settings" class="welcome-button">Settings</button>
                            </div>
                            <div class="build-info">Build ${buildInfo.buildTime}</div>
                        </div>
                    </div>

                    <!-- Conversations container (hidden when no conversations) -->
                    <div id="conversations-container" class="conversations-container" style="display: none;">
                        <!-- Conversation views will be dynamically added here -->
                    </div>
                </div>
                <!-- Provider selector template (will be cloned for each conversation) -->
                <template id="provider-selector-template">
                    <div class="provider-selector">
                        <label for="provider-select">Provider:</label>
                        <select class="provider-select">
                            <!-- Options will be populated dynamically -->
                        </select>
                        <button class="refresh-providers" title="Refresh providers">↻</button>
                    </div>
                </template>
                <script nonce="${nonce}" src="${scriptUri}"></script>
            </body>
            </html>`;
    }

    private getNonce(): string {
        let text = '';
        const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
        for (let i = 0; i < 32; i++) {
            text += possible.charAt(Math.floor(Math.random() * possible.length));
        }
        return text;
    }

    public dispose(): void {
        this.conversationManager.dispose();
    }
}