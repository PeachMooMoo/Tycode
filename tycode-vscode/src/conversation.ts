import { EventEmitter } from 'events';
import { ChatActorClient } from '../lib/client';
import * as vscode from 'vscode';
import { 
    ConversationMessage, 
    ChatEvent,
    CONVERSATION_EVENTS 
} from './events';

export class Conversation extends EventEmitter {
    public client: ChatActorClient;
    private _id: string;
    private _title: string;
    private _messages: ConversationMessage[] = [];
    private _isActive: boolean = false;
    private _isManuallyNamed: boolean = false;
    private _hasFirstMessage: boolean = false;
    private _selectedProvider: string | undefined;
    private eventConsumer: Promise<void> | null = null;
    private shouldStop: boolean = false;

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
        
        // Get workspace roots for the client
        const workspaceFolders = vscode.workspace.workspaceFolders;
        const workspaceRoots = workspaceFolders ? workspaceFolders.map(f => f.uri.fsPath) : [];
        
        console.log('[Conversation] Creating ChatActorClient with workspaceRoots:', workspaceRoots);
        console.log('[Conversation] Using default settings path (~/.tycode/settings.toml)');
        
        // Use default settings path (~/.tycode/settings.toml)
        this.client = new ChatActorClient(workspaceRoots);
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
        this.emit(CONVERSATION_EVENTS.TITLE_CHANGED, value);
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
            this.emit(CONVERSATION_EVENTS.PROVIDER_CHANGED, provider);
        }
    }

    async initialize(): Promise<void> {
        this._isActive = true;

        // Start consuming events from the client
        this.startEventConsumption();
    }

    private startEventConsumption(): void {
        this.shouldStop = false;
        this.eventConsumer = this.consumeEvents();
    }

    private async consumeEvents(): Promise<void> {
        try {
            for await (const event of this.client.events()) {
                if (this.shouldStop) {
                    break;
                }
                
                console.log('[Conversation] Received event:', event);
                await this.handleEvent(event);
            }
        } catch (error) {
            console.error('[Conversation] Event consumption error:', error);
            if (!this.shouldStop) {
                const message: ConversationMessage = {
                    role: 'error',
                    content: `Event consumption error: ${error}`
                };
                this._messages.push(message);
                this.emit(CONVERSATION_EVENTS.ERROR, message);
            }
        }
    }

    private async handleEvent(event: ChatEvent): Promise<void> {
        // Handle string events first
        if (event === 'ConversationCleared') {
            this._messages = [];
            this.emit(CONVERSATION_EVENTS.CLEARED);
            return;
        }
        
        // Handle object events
        if (typeof event === 'object' && event !== null) {
            if ('MessageAdded' in event) {
                const responseMessage: ConversationMessage = {
                    role: 'assistant',
                    content: event.MessageAdded.content,
                    reasoning: event.MessageAdded.reasoning?.text,
                    toolCalls: event.MessageAdded.tool_calls || [],
                    model: event.MessageAdded.model_info?.model,
                    isComplete: true, // MessageAdded events are complete
                    tokenUsage: event.MessageAdded.token_usage
                };
                this._messages.push(responseMessage);
                this.emit(CONVERSATION_EVENTS.RESPONSE, responseMessage);
            } else if ('Settings' in event) {
                // Handle settings events - could emit a settings event if needed
                console.log('[Conversation] Settings updated:', event.Settings);
            } else if ('TypingStatusChanged' in event) {
                this.emit(CONVERSATION_EVENTS.TYPING_STATUS, {
                    is_typing: event.TypingStatusChanged
                });
            } else if ('ToolExecutionCompleted' in event) {
                const toolMessage: ConversationMessage = {
                    role: 'tool-result',
                    content: JSON.stringify(event.ToolExecutionCompleted),
                    toolName: event.ToolExecutionCompleted.tool_name,
                    success: event.ToolExecutionCompleted.success,
                    result: event.ToolExecutionCompleted.result,
                    error: event.ToolExecutionCompleted.error
                };
                this._messages.push(toolMessage);
                this.emit(CONVERSATION_EVENTS.TOOL_RESULT, {
                    tool_name: event.ToolExecutionCompleted.tool_name,
                    success: event.ToolExecutionCompleted.success,
                    result: event.ToolExecutionCompleted.result,
                    error: event.ToolExecutionCompleted.error
                });
            } else if ('OperationCancelled' in event) {
                const systemMessage: ConversationMessage = {
                    role: 'system',
                    content: `Operation cancelled: ${event.OperationCancelled.message}`
                };
                this._messages.push(systemMessage);
                this.emit(CONVERSATION_EVENTS.SYSTEM, systemMessage);
            } else if ('RetryAttempt' in event) {
                this.emit(CONVERSATION_EVENTS.RETRY_ATTEMPT, {
                    attempt: event.RetryAttempt.attempt,
                    max_retries: event.RetryAttempt.max_retries,
                    error: event.RetryAttempt.error,
                    backoff_ms: event.RetryAttempt.backoff_ms
                });
            } else if ('Error' in event) {
                const errorMessage: ConversationMessage = {
                    role: 'error',
                    content: event.Error
                };
                this._messages.push(errorMessage);
                this.emit(CONVERSATION_EVENTS.ERROR, errorMessage);
            } else {
                console.warn('[Conversation] Unknown object event:', event);
            }
        } else {
            console.warn('[Conversation] Unknown event type:', event);
        }
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
        this.emit(CONVERSATION_EVENTS.USER_MESSAGE, userMessage);

        // Auto-generate title from first message if not manually named
        if (!this._hasFirstMessage && !this._isManuallyNamed) {
            this._hasFirstMessage = true;
            const generatedTitle = this.generateTitleFromMessage(content);
            if (generatedTitle && generatedTitle !== this._title) {
                this._title = generatedTitle;
                // Don't mark as manually named since this is auto-generated
                this.emit(CONVERSATION_EVENTS.TITLE_CHANGED, generatedTitle);
            }
        }

        // Send to subprocess with selected provider
        await this.client.sendMessage(content);
    }

    async sendCancel(): Promise<void> {
        if (!this._isActive) {
            throw new Error('Conversation is not active');
        }
        
        // Send cancel message to subprocess
        await this.client.cancel();
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
        this.emit(CONVERSATION_EVENTS.CLEARED);
    }

    async switchProvider(provider: string): Promise<void> {
        if (this._selectedProvider === provider) {
            return; // No change needed
        }

        // Store old provider
        const oldProvider = this._selectedProvider;
        this._selectedProvider = provider;

        // Send cancel first to stop any ongoing processing
        await this.client.cancel();
        
        // Then send provider change message to the subprocess
        await this.client.changeProvider(provider);

        // Emit event
        this.emit(CONVERSATION_EVENTS.PROVIDER_SWITCHED, oldProvider, provider);
    }

    dispose(): void {
        this._isActive = false;
        this.shouldStop = true;
        
        // Stop the event consumer
        if (this.eventConsumer) {
            this.eventConsumer.catch(() => {}); // Ignore errors during shutdown
        }
        
        // Close the client
        this.client.close();
        this.removeAllListeners();
    }
}