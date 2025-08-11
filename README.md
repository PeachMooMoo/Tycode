# TyCode

TyCode is an AI-powered development assistant that integrates with Amazon Bedrock's Claude models to provide intelligent code generation, refactoring, and development guidance. Built in Rust, it offers both a native CLI and VSCode extension for seamless AI-assisted programming with full project context awareness.

## 🎯 What TyCode Does

**TyCode** is an AI pair programmer that helps you write better code faster:

- **🤖 Intelligent Code Generation**: Generate functions, modules, or entire features based on natural language descriptions
- **📝 Code Refactoring**: Improve existing code structure, performance, and readability with AI-powered suggestions
- **📁 File Management**: Read, write, modify, and track files with context-aware operations
- **🔧 Command Execution**: Run cargo commands and development tools directly through the assistant
- **💬 Interactive Development**: Chat with AI about your code, get explanations, and debug issues together
- **🎯 Structured Workflow**: Follows a disciplined approach: understand → plan → implement → review

## 🏗️ Architecture

### Core Modules

- **`agents/`** - Specialized AI agents (Software Engineer, Design) with different capabilities and tool access
- **`ai/`** - AI provider abstractions and Amazon Bedrock integration with Claude models
- **`chat/`** - Event-driven chat actor system for managing conversations and state
- **`settings/`** - Configuration management and persistent settings
- **`tools/`** - Development tools for file operations, command execution, and context management

### AI Integration (`tycode-core/src/ai/`)

The AI module provides a clean abstraction over Amazon Bedrock services with unified content handling:

```rust
// Unified conversation API
pub struct ConversationRequest {
    pub messages: Vec<Message>,
    pub model: Model,
    pub system_prompt: String,
    pub tunings: ModelTunings,
    pub stop_sequences: Vec<String>,
    pub tools: Vec<ToolDefinition>,
}

#[async_trait]
pub trait AiProvider: Clone + Send + Sync {
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse>;
}
```

**Key Features:**
- Multiple Claude model support (Sonnet, Haiku, Opus)
- AI reasoning modes with configurable token budgets
- Tool usage and function calling capabilities
- System prompts and conversation memory
- Structured development workflow enforcement

## 🖥️ TyCode CLI - Interactive AI Terminal

The flagship application is **TyCode CLI**, a sophisticated terminal interface for AI-powered development.

### Features

- 🎨 **Rich Terminal Interface**: Professional TUI with colors, borders, and organized layout
- 📜 **Scrollable History**: Full conversation history with navigation controls
- ⌨️ **Input History**: Navigate previous commands with arrow keys
- 🤖 **Model Selection**: Switch between Claude models dynamically
- 🧠 **Reasoning Support**: Enable/disable reasoning with configurable token budgets
- 🔧 **Command System**: Slash commands for application control
- 📊 **Status Display**: Real-time model, reasoning status, and shortcuts
- 📁 **File Tracking**: Track specific files for context awareness

### Usage

```bash
# Basic usage
cargo run --bin tycode

# With custom configuration
cargo run --bin tycode -- --model claude-sonnet-4 --temperature 0.7 --reasoning-budget 2000

# Different AWS profile/region
cargo run --bin tycode -- --profile myprofile --region us-east-1

# Subprocess mode for VSCode extension
cargo run --bin tycode -- --subprocess
```

### Interactive Commands

- `/help` - Show help dialog
- `/settings` - Display current configuration and settings file
- `/clear` - Clear conversation history
- `/model <name>` - Switch AI model
- `/reasoning [budget]` - Enable/disable reasoning mode
- `/fileapi <patch|findreplace>` - Set file modification API
- `/trace <on|off>` - Enable/disable trace logging

### Keyboard Shortcuts

- **Enter**: Send message
- **↑/↓**: Navigate input history
- **Ctrl+↑/↓**: Scroll message history
- **Page Up/Down**: Fast scroll through conversation
- **F1**: Toggle help dialog
- **F2**: Toggle settings dialog
- **ESC**: Close dialogs or scroll to bottom
- **Ctrl+C**: Exit application

## 🚀 Getting Started

### Prerequisites

- Rust 1.70+
- AWS credentials configured (via AWS CLI, environment variables, or IAM roles)
- Access to Amazon Bedrock with Claude models

### Installation & Setup

```bash
# Clone and build
git clone <repository>
cd tycode
cargo build

# Run the CLI (recommended)
cargo run --bin tycode

# Install the VSCode extension
cd tycode-vscode
npm install
npm run package
# Install the generated .vsix file in VSCode

# Run tests
cargo test

# View documentation
cargo doc --open
```

### Configuration

TyCode supports persistent configuration through `~/.tycode/settings.toml`:

```toml
[global]
default_model = "claude-sonnet-4"
file_modification_api = "FindReplace"

[aws]
profile = "cline"
region = "us-west-2"

[agents.software_engineer]
model = "claude-opus-4-1"
temperature = 0.7
max_tokens = 4096
```

The settings file is automatically created on first run. Use `/settings` command to view current configuration. See [SETTINGS.md](SETTINGS.md) for detailed configuration options.

### AWS Configuration

The application uses standard AWS credential resolution:
1. Settings file (`~/.tycode/settings.toml`)
2. Command-line arguments (`--profile`, `--region`)
3. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
4. AWS credentials file (`~/.aws/credentials`)
5. IAM roles (when running on EC2/ECS)

## 📦 Key Dependencies

### Core AWS Integration
- `aws-sdk-bedrockruntime` - Amazon Bedrock AI services
- `aws-config` - AWS configuration and authentication
- `aws-smithy-types` - AWS SDK type definitions

### Terminal UI & User Experience (CLI only)
- `colored` - Terminal output coloring
- `rustyline` - Readline implementation for input handling
- `indicatif` - Progress indicators
- `clap` - Command-line argument parsing

### Core Functionality
- `tokio` - Async runtime
- `serde` + `serde_json` - Serialization
- `anyhow` - Error handling
- `similar` - Diff generation for file modifications
- `regex` - Pattern matching for file operations
- `walkdir` - Recursive file traversal

## 🎯 Use Cases

### Code Development
- **Feature Implementation**: Describe what you want to build and let TyCode generate the code
- **Code Review**: Get AI-powered feedback on code quality, bugs, and improvements
- **Refactoring**: Transform legacy code into modern, maintainable structures
- **Test Generation**: Automatically create comprehensive test suites
- **Documentation**: Generate or improve code documentation and comments

### Debugging & Problem Solving
- **Bug Investigation**: Get help understanding and fixing complex bugs
- **Performance Analysis**: Identify and resolve performance bottlenecks
- **Error Resolution**: Understand error messages and get fix suggestions
- **Code Explanation**: Have AI explain complex code sections in plain language

### Learning & Best Practices
- **Pattern Guidance**: Learn design patterns and architectural best practices
- **Language Features**: Understand language-specific features and idioms
- **Library Usage**: Get examples and explanations for using external libraries
- **Code Style**: Ensure consistent code style and conventions

### Project Management
- **File Organization**: Restructure and organize project files effectively
- **Dependency Management**: Analyze and optimize project dependencies
- **Migration Assistance**: Help with framework or library migrations
- **Codebase Analysis**: Understand large codebases quickly

## 🛠️ Development & Extension

### Architecture Benefits

- **Modular Design**: Clean separation between core, CLI, and VSCode extension
- **Agent System**: Specialized agents for different types of tasks
- **Tool Abstraction**: Extensible tool system for new capabilities
- **Provider Pattern**: Easy extension to other AI services beyond Bedrock
- **Event-Driven**: Async event-based chat system for scalability
- **Type Safety**: Rust's type system prevents common errors

### Extension Points

- **New Agents**: Create specialized agents for specific domains
- **Custom Tools**: Add new tools for specific development workflows
- **AI Providers**: Implement the `AiProvider` trait for other services
- **File Strategies**: Add new file modification strategies beyond patch/find-replace

### Code Quality

- Follows Rust best practices with comprehensive error handling
- Uses `anyhow` for ergonomic error management
- Structured logging with `tracing`
- Modular architecture for maintainability and testing
- Strict style guide enforcement (YAGNI, shallow nesting, immediate error surfacing)

## 🎪 Why TyCode?

TyCode transforms how developers interact with AI assistance by providing a context-aware, structured approach to AI pair programming:

1. **Context Awareness**: Tracks and understands your project files, maintaining awareness of your codebase
2. **Structured Workflow**: Follows a disciplined development process ensuring quality and completeness
3. **Multiple Interfaces**: Use it in your terminal or directly in VSCode - your choice
4. **Tool Integration**: Executes commands and manipulates files directly, not just suggesting changes
5. **Configurable AI**: Choose between different Claude models based on task complexity
6. **Learning Partner**: Not just generating code, but explaining and teaching as it goes

This makes TyCode particularly valuable for:
- **Solo Developers** wanting an intelligent pair programmer
- **Teams** looking to accelerate development and maintain consistency
- **Learners** seeking to understand code and best practices
- **Senior Engineers** automating routine tasks and focusing on architecture
- **Code Reviewers** getting AI-assisted analysis and suggestions
- **Project Maintainers** managing refactoring and technical debt

The key differentiator is that TyCode doesn't just suggest code - it understands your project context, plans its approach, implements changes directly, and reviews its own work, all while following software engineering best practices.

---

*TyCode: Your AI pair programmer that actually writes code.*