// Re-export types from tycode-client-typescript
export type {
    ChatEvent,
    ChatMessage,
    ChatActorMessage,
    ContextInfo,
    FileInfo,
    Model,
    ModelInfo,
    MessageSender,
    ReasoningData,
    ToolUseData,
    TokenUsage
} from '../lib/types';

// Message types for conversation (mapped from ChatMessage)
export interface ConversationMessage {
    role: 'user' | 'assistant' | 'system' | 'error' | 'tool-result';
    content: string;
    reasoning?: string;
    toolCalls?: any[];
    model?: string;
    isComplete?: boolean;
    tokenUsage?: {
        input_tokens?: number;
        output_tokens?: number;
        total_tokens?: number;
    };
    toolName?: string;
    success?: boolean;
    result?: any;
    error?: string;
}

// Settings structure
export interface Settings {
    providers?: {
        [key: string]: any;
    };
    active_provider?: string;
    [key: string]: any;
}

// Event constants for conversation management
export const CONVERSATION_EVENTS = {
    RESPONSE: 'response',
    USER_MESSAGE: 'userMessage',
    SYSTEM: 'system',
    ERROR: 'error',
    TOOL_RESULT: 'toolResult',
    TITLE_CHANGED: 'titleChanged',
    PROVIDER_CHANGED: 'providerChanged',
    PROVIDER_SWITCHED: 'providerSwitched',
    DISCONNECTED: 'disconnected',
    TYPING_STATUS: 'typingStatus',
    RETRY_ATTEMPT: 'retryAttempt',
    CLEARED: 'cleared'
} as const;

// Event constants for conversation manager
export const MANAGER_EVENTS = {
    CONVERSATION_CREATED: 'conversationCreated',
    CONVERSATION_UPDATE: 'conversationUpdate',
    CONVERSATION_TITLE_CHANGED: 'conversationTitleChanged',
    CONVERSATION_PROVIDER_CHANGED: 'conversationProviderChanged',
    CONVERSATION_PROVIDER_SWITCHED: 'conversationProviderSwitched',
    CONVERSATION_DISCONNECTED: 'conversationDisconnected',
    ACTIVE_CONVERSATION_CHANGED: 'activeConversationChanged',
    CONVERSATION_CLOSED: 'conversationClosed',
    ALL_CONVERSATIONS_CLOSED: 'allConversationsClosed'
} as const;