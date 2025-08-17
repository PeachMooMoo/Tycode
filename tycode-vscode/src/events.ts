// Event type definitions and constants for TyCode VSCode extension

// Message types for conversation
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

// Response event from AI provider
export interface ResponseEvent {
    content: string;
    reasoning?: string;
    tool_calls?: any[];
    model?: string;
    is_complete?: boolean;
    context_info?: {
        tracked_files?: string[];
        context_size?: number;
    };
    token_usage?: {
        input_tokens?: number;
        output_tokens?: number;
        total_tokens?: number;
    };
}

// Tool execution result event
export interface ToolResultEvent {
    tool_name: string;
    success: boolean;
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

// Messages exchanged with subprocess
export interface SubprocessMessage {
    type: 'Ready' | 'Chat' | 'Cancel' | 'Response' | 'ToolResult' | 'Event' | 'Error' | 
          'ChangeProvider' | 'LoadSettings' | 'SettingsLoaded' | 'SaveSettings' | 
          'SettingsSaved' | 'ReloadSettings';
    
    // Chat message
    message?: string;
    
    // Response fields
    content?: string;
    reasoning?: string;
    tool_calls?: any[];
    model?: string;
    is_complete?: boolean;
    context_info?: any;
    token_usage?: any;
    
    // Tool result fields
    tool_name?: string;
    success?: boolean;
    result?: any;
    
    // Event fields
    event?: string;
    data?: any;
    
    // Error field
    error?: string;
    
    // Provider field
    provider?: string;
    
    // Settings fields
    settings?: Settings;
}

// Bridge events (subprocess communication)
export const BRIDGE_EVENTS = {
    MESSAGE: 'message',
    RESPONSE: 'response',
    TOOL_RESULT: 'toolResult',
    EVENT: 'event',
    ERROR: 'error',
    DISCONNECTED: 'disconnected',
    SETTINGS_RELOADED: 'settingsReloaded'
} as const;

// Conversation events
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
    CLEARED: 'cleared'
} as const;

// Conversation manager events
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