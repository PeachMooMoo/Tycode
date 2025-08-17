import { EventEmitter } from 'events';
import { SubprocessBridge } from './subprocessBridge';
import * as vscode from 'vscode';

export interface ConversationMessage {
    role: string;
    content: string;
    reasoning?: string;
    toolCalls?: any[];
    model?: string;
    isComplete?: boolean;
    tokenUsage?: any;
}

export class Conversation extends EventEmitter {
    public bridge: SubprocessBridge;
    private _id: string;
    private _title: string;
    private _messages: ConversationMessage[] = [];
    private _isActive: boolean = false;
    private _isManuallyNamed: boolean = false;
    private _hasFirstMessage: boolean = false;
    private _selectedProvider: string | undefined;

    constructor(
        private context: vscode.ExtensionContext,
        id: string,
        title?: string,
        selectedProvider?: string
    ) {
        super();
        this._id = id;
        this._title = title || 'New Chat';
        this._isManuallyNamed = !!title;
        this._selectedProvider = selectedProvider;
        this.bridge = new SubprocessBridge(context);
    }

    get id(): string {
        return this._id;
    }

    get title(): string {
        return this._title;
    }

    set title(value: string) {
        this._title = value;
        this._isManuallyNamed = true;  // Mark as manually named when user sets title
        this.emit('titleChanged', value);
    }

    get messages(): ConversationMessage[] {
        return this._messages;
    }

    get isActive(): boolean {
        return this._isActive;
    }

    get selectedProvider(): string | undefined {
        return this._selectedProvider;
    }

    set selectedProvider(provider: string | undefined) {
        if (this._selectedProvider !== provider) {
            this._selectedProvider = provider;
            this.emit('providerChanged', provider);
        }
    }

    async initialize(): Promise<void> {
        await this.bridge.initialize();
        this._isActive = true;

        // Don't send provider change on init - let subprocess use its settings
        
        // Set up event listeners
        this.bridge.on('response', (response: any) => {
            const message: ConversationMessage = {
                role: 'assistant',
                content: response.content,
                reasoning: response.reasoning,
                toolCalls: response.tool_calls || [],
                model: response.model,
                isComplete: response.is_complete,
                tokenUsage: response.token_usage
            };
            this._messages.push(message);
            this.emit('response', message);
        });

        this.bridge.on('event', (event: string, data: any) => {
            if (event === 'system') {
                const message: ConversationMessage = {
                    role: 'system',
                    content: data.content
                };
                this._messages.push(message);
                this.emit('system', message);
            }
        });

        this.bridge.on('error', (error: string) => {
            const message: ConversationMessage = {
                role: 'error',
                content: error
            };
            this._messages.push(message);
            this.emit('error', message);
        });

        this.bridge.on('toolResult', (result: any) => {
            console.log('[Conversation] Received toolResult:', result);
            const message: ConversationMessage = {
                role: 'tool-result',
                content: JSON.stringify(result),
                toolName: result.tool_name,
                success: result.success,
                result: result.result,
                error: result.error
            } as any;
            this._messages.push(message);
            this.emit('toolResult', result);
        });

        this.bridge.on('disconnected', () => {
            this._isActive = false;
            this.emit('disconnected');
        });
    }

    async sendMessage(content: string): Promise<void> {
        if (!this._isActive) {
            throw new Error('Conversation is not active');
        }

        // Add user message to history
        const userMessage: ConversationMessage = {
            role: 'user',
            content
        };
        this._messages.push(userMessage);
        this.emit('userMessage', userMessage);

        // Auto-generate title from first message if not manually named
        if (!this._hasFirstMessage && !this._isManuallyNamed) {
            this._hasFirstMessage = true;
            const generatedTitle = this.generateTitleFromMessage(content);
            if (generatedTitle && generatedTitle !== this._title) {
                this._title = generatedTitle;
                // Don't mark as manually named since this is auto-generated
                this.emit('titleChanged', generatedTitle);
            }
        }

        // Send to subprocess with selected provider
        await this.bridge.sendMessage(content);
    }

    async sendCancel(): Promise<void> {
        if (!this._isActive) {
            throw new Error('Conversation is not active');
        }
        
        // Send cancel to subprocess
        await this.bridge.sendCancel();
    }

    private generateTitleFromMessage(message: string): string {
        // Remove leading/trailing whitespace
        let title = message.trim();
        
        // Remove code blocks for cleaner titles
        title = title.replace(/```[\s\S]*?```/g, '[code]');
        title = title.replace(/`[^`]+`/g, '...');
        
        // Remove URLs
        title = title.replace(/https?:\/\/[^\s]+/g, '[link]');
        
        // Remove excessive whitespace
        title = title.replace(/\s+/g, ' ');
        
        // Take first line/sentence
        const firstLine = title.split('\n')[0];
        const firstSentence = firstLine.split(/[.!?]/)[0];
        
        // Use whichever is shorter but meaningful
        title = firstSentence.length > 10 ? firstSentence : firstLine;
        
        // Truncate if too long (keep it concise)
        const maxLength = 40;
        if (title.length > maxLength) {
            title = title.substring(0, maxLength - 3) + '...';
        }
        
        // Fallback if message is too short or empty after processing
        if (title.length < 3) {
            // Try to extract something meaningful from original
            const words = message.trim().split(/\s+/).slice(0, 5).join(' ');
            title = words.length > 3 ? words : 'New Chat';
        }
        
        return title;
    }

    clearMessages(): void {
        this._messages = [];
        this.emit('cleared');
    }

    async switchProvider(provider: string): Promise<void> {
        if (this._selectedProvider === provider) {
            return; // No change needed
        }

        // Store old provider
        const oldProvider = this._selectedProvider;
        this._selectedProvider = provider;

        // Send cancel first to stop any ongoing processing
        await this.bridge.sendCancel();
        
        // Then send provider change message to the subprocess
        await this.bridge.changeProvider(provider);

        // Emit event
        this.emit('providerSwitched', oldProvider, provider);

        
    }

    dispose(): void {
        this._isActive = false;
        this.bridge.dispose();
        this.removeAllListeners();
    }
}