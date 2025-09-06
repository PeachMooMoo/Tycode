// Exact port of tycode-core/src/chat/events.rs, actor.rs, ai/types.rs, ai/model.rs

export type ChatEvent =
  | { MessageAdded: ChatMessage }
  | { Settings: any }
  | { TypingStatusChanged: boolean }
  | 'ConversationCleared'
  | {
      ToolExecutionCompleted: {
        tool_name: string,
        success: boolean,
        result?: any,
        ui_data?: any,
        error?: string
      }
    }
  | { OperationCancelled: { message: string } }
  | {
      RetryAttempt: {
        attempt: number,
        max_retries: number,
        error: string,
        backoff_ms: number
      }
    }
  | { Error: string }

export interface ChatMessage {
  timestamp: number;
  sender: MessageSender;
  content: string;
  reasoning?: ReasoningData;
  tool_calls: ToolUseData[];
  model_info?: ModelInfo;
  context_info?: ContextInfo;
  token_usage?: TokenUsage;
}

export interface ContextInfo {
  directory_list_bytes: number;
  files: FileInfo[];
}

export interface FileInfo {
  path: string;
  bytes: number;
}

export type Model = 'claude-opus-4-1' | 'claude-opus-4' | 'claude-sonnet-4' | 'claude-sonnet-3-7' | 'gpt-oss-120b' | 'grok-code-fast-1' | 'None';

export interface ModelInfo {
  model: Model;
}

export type MessageSender = 'User' | 'System' | 'Error' | { Assistant: { agent: string } }

export interface ReasoningData {
  text: string;
  signature?: string;
  blob?: number[]; // byte array
}

export interface ToolUseData {
  id: string;
  name: string;
  arguments: any;
}

export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
}

// Exact port from tycode-core/src/chat/actor.rs
export type ChatActorMessage =
  | { UserInput: string }
  | { ChangeProvider: string }
  | 'GetSettings'
  | { SaveSettings: { settings: any } }