import { EventEmitter } from 'events';
import { Conversation } from './conversation';
import * as vscode from 'vscode';

export class ConversationManager extends EventEmitter {
    private conversations: Map<string, Conversation> = new Map();
    private activeConversationId: string | null = null;
    private availableProviders: { [name: string]: any } = {};
    private defaultProvider: string | null = null;

    constructor(private context: vscode.ExtensionContext) {
        super();
        // Provider settings will be loaded from subprocess when needed
        this.availableProviders = {};
        this.defaultProvider = null;  // Don't hardcode - get from settings
    }

    getAvailableProviders(): string[] {
        return Object.keys(this.availableProviders);
    }

    getDefaultProvider(): string | null {
        return this.defaultProvider;
    }

    async createConversation(title?: string, selectedProvider?: string): Promise<Conversation> {
        const id = this.generateId();
        // Don't default to anything - let the subprocess use its settings
        const conversation = new Conversation(this.context, id, title, selectedProvider);
        
        await conversation.initialize();
        
        this.conversations.set(id, conversation);
        this.activeConversationId = id;

        // Forward conversation events
        conversation.on('response', (message) => {
            this.emit('conversationUpdate', id, 'response', message);
        });

        conversation.on('userMessage', (message) => {
            this.emit('conversationUpdate', id, 'userMessage', message);
        });

        conversation.on('system', (message) => {
            this.emit('conversationUpdate', id, 'system', message);
        });

        conversation.on('error', (message) => {
            this.emit('conversationUpdate', id, 'error', message);
        });

        conversation.on('toolResult', (result) => {
            this.emit('conversationUpdate', id, 'toolResult', result);
        });

        conversation.on('titleChanged', (newTitle) => {
            this.emit('conversationTitleChanged', id, newTitle);
        });

        conversation.on('providerChanged', (provider) => {
            this.emit('conversationProviderChanged', id, provider);
        });

        conversation.on('providerSwitched', (oldProvider, newProvider) => {
            this.emit('conversationProviderSwitched', id, oldProvider, newProvider);
        });

        conversation.on('disconnected', () => {
            this.emit('conversationDisconnected', id);
        });

        this.emit('conversationCreated', conversation);
        
        return conversation;
    }

    getConversation(id: string): Conversation | undefined {
        return this.conversations.get(id);
    }

    getActiveConversation(): Conversation | undefined {
        return this.activeConversationId ? this.conversations.get(this.activeConversationId) : undefined;
    }

    setActiveConversation(id: string): boolean {
        if (this.conversations.has(id)) {
            this.activeConversationId = id;
            this.emit('activeConversationChanged', id);
            return true;
        }
        return false;
    }

    getAllConversations(): Conversation[] {
        return Array.from(this.conversations.values());
    }

    closeConversation(id: string): boolean {
        const conversation = this.conversations.get(id);
        if (conversation) {
            conversation.dispose();
            this.conversations.delete(id);

            // If this was the active conversation, clear it or switch to another
            if (this.activeConversationId === id) {
                const remaining = Array.from(this.conversations.keys());
                this.activeConversationId = remaining.length > 0 ? remaining[remaining.length - 1] : null;
                if (this.activeConversationId) {
                    this.emit('activeConversationChanged', this.activeConversationId);
                }
            }

            this.emit('conversationClosed', id);
            return true;
        }
        return false;
    }

    closeAllConversations(): void {
        for (const conversation of this.conversations.values()) {
            conversation.dispose();
        }
        this.conversations.clear();
        this.activeConversationId = null;
        this.emit('allConversationsClosed');
    }

    private generateId(): string {
        return Date.now().toString(36) + Math.random().toString(36).substr(2);
    }

    

    dispose(): void {
        this.closeAllConversations();
        this.removeAllListeners();
    }
}