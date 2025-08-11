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
    private bridge: SubprocessBridge;
    private _id: string;
    private _title: string;
    private _messages: ConversationMessage[] = [];
    private _isActive: boolean = false;

    constructor(
        private context: vscode.ExtensionContext,
        id: string,
        title?: string
    ) {
        super();
        this._id = id;
        this._title = title || `Chat ${id.slice(0, 8)}`;
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
        this.emit('titleChanged', value);
    }

    get messages(): ConversationMessage[] {
        return this._messages;
    }

    get isActive(): boolean {
        return this._isActive;
    }

    async initialize(): Promise<void> {
        await this.bridge.initialize();
        this._isActive = true;

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

        // Send to subprocess
        await this.bridge.sendMessage(content);
    }

    clearMessages(): void {
        this._messages = [];
        this.emit('cleared');
    }

    dispose(): void {
        this._isActive = false;
        this.bridge.dispose();
        this.removeAllListeners();
    }
}