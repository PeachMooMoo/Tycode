# TyCode

TyCode is an intelligent operations toolkit that combines AWS service management with AI capabilities to enhance productivity in cloud operations tasks. Built in Rust, it provides AI-powered interactions through Amazon Bedrock with a focus on Aurora DSQL database operations, CloudWatch Logs analysis, and AWS infrastructure management.

## 🎯 What TyCode Does

**TyCode** is an AI-powered operational tool that serves as your intelligent assistant for AWS operations:

- **🤖 AI-Enhanced Operations**: Leverage Amazon Bedrock (Claude models) for intelligent analysis, troubleshooting, and operational guidance
- **🗄️ Database Management**: Specialized support for Aurora DSQL database operations with AI assistance
- **📊 Log Analysis**: Analyze CloudWatch logs using AI-powered insights and pattern recognition
- **☁️ Infrastructure Management**: Configure and manage AWS resources with intelligent recommendations
- **💬 Interactive Terminal**: Professional TUI for conversing with AI models and executing operations
- **🔧 Automation Ready**: Build AI-enhanced operational workflows and scripts

## 🏗️ Architecture

### Core Modules

- **`ai/`** - AI provider abstractions and Amazon Bedrock integration
- **`aws/`** - AWS service clients and utilities
- **`db/`** - Database operations with Aurora DSQL focus
- **`config/`** - Configuration management and AWS credential handling
- **`terminal/`** - Interactive terminal UI applications
- **`tools/`** - Utility functions and operational helpers

### AI Integration (`src/ai/`)

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

## 🖥️ TyCode Chat - Interactive AI Terminal

The flagship application is **TyCode Chat**, a sophisticated terminal user interface for AI-powered operations.

### Features

- 🎨 **Rich Terminal Interface**: Professional TUI with colors, borders, and organized layout
- 📜 **Scrollable History**: Full conversation history with navigation controls
- ⌨️ **Input History**: Navigate previous commands with arrow keys
- 🤖 **Model Selection**: Switch between Claude models dynamically
- 🧠 **Reasoning Support**: Enable/disable reasoning with configurable token budgets
- 🔧 **Command System**: Slash commands for application control
- 📊 **Status Display**: Real-time model, reasoning status, and shortcuts

### Usage

```bash
# Basic usage
cargo run --bin tycode-chat

# With custom configuration
cargo run --bin tycode-chat -- --model claude-sonnet-4 --temperature 0.7 --reasoning-budget 2000

# Different AWS profile/region
cargo run --bin tycode-chat -- --profile myprofile --region us-east-1
```

### Interactive Commands

- `/help` - Show help dialog
- `/settings` - Display current configuration  
- `/clear` - Clear conversation history
- `/model <n>` - Switch AI model
- `/reasoning [budget]` - Enable/disable reasoning mode

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

# Run the chat application
cargo run --bin tycode-chat

# Run tests
cargo test

# View documentation
cargo doc --open
```

### AWS Configuration

The application uses standard AWS credential resolution:
1. Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
2. AWS credentials file (`~/.aws/credentials`)
3. IAM roles (when running on EC2/ECS)
4. AWS CLI profiles (specify with `--profile` flag)

## 📦 Key Dependencies

### Core AWS Integration
- `aws-sdk-bedrockruntime` - Amazon Bedrock AI services
- `aws-sdk-cloudwatchlogs` - CloudWatch logs analysis
- `aws-sdk-dsql` - Aurora DSQL database operations
- `aws-sdk-sts` - AWS Security Token Service

### Terminal UI & User Experience
- `ratatui` - Modern terminal UI framework
- `crossterm` - Cross-platform terminal handling
- `tui-input` - Input widgets and controls
- `clap` - Command-line argument parsing

### Core Functionality
- `tokio` - Async runtime
- `sqlx` - Database connectivity
- `serde` + `serde_json` - Serialization
- `anyhow` - Error handling

## 🎯 Use Cases

### Database Operations
- **Query Assistance**: Get AI help with complex SQL queries
- **Schema Analysis**: Understand database structure and relationships
- **Performance Troubleshooting**: Analyze query performance with AI insights
- **Data Migration**: Plan and execute data migrations with AI guidance

### Log Analysis
- **Pattern Recognition**: Identify patterns and anomalies in CloudWatch logs
- **Error Investigation**: Get AI assistance in troubleshooting application errors
- **Performance Analysis**: Analyze application performance from log data
- **Alert Investigation**: Investigate and understand alert triggers

### Infrastructure Management
- **Configuration Review**: Get AI recommendations for AWS configurations
- **Cost Optimization**: Analyze and optimize AWS resource usage
- **Security Analysis**: Review security configurations and policies
- **Capacity Planning**: Plan resource scaling with AI insights

### Interactive Troubleshooting
- **Real-time Support**: Get immediate AI assistance during operations
- **Documentation Helper**: Ask questions about AWS services and best practices
- **Workflow Guidance**: Step-by-step guidance for complex operational tasks
- **Learning Assistant**: Learn AWS services and operations interactively

## 🛠️ Development & Extension

### Architecture Benefits

- **Modular Design**: Clean separation of concerns across modules
- **Provider Pattern**: Easy extension to other AI services beyond Bedrock
- **Async-First**: Built for high-performance concurrent operations
- **Type Safety**: Rust's type system prevents common operational errors

### Extension Points

- **New AI Providers**: Implement the `AiProvider` trait for other services
- **Custom Terminal Apps**: Extend the terminal module for specialized UIs
- **Database Integrations**: Add support for other database systems
- **AWS Service Extensions**: Integrate additional AWS services as needed

### Code Quality

- Follows Rust best practices with comprehensive error handling
- Uses `anyhow` for ergonomic error management
- Structured logging with `tracing`
- Modular architecture for maintainability and testing

## 🎪 Why TyCode?

TyCode bridges the gap between traditional operational tools and modern AI capabilities. Instead of switching between multiple tools, documentation, and support channels, operators can:

1. **Ask Questions**: Get instant answers about configurations, errors, and best practices
2. **Analyze Data**: Use AI to understand patterns in logs, metrics, and database queries  
3. **Get Guidance**: Receive step-by-step instructions for complex operational tasks
4. **Learn Interactively**: Understand AWS services and operations through conversation
5. **Automate Intelligently**: Build workflows that adapt and improve with AI insights

This makes TyCode particularly valuable for:
- **DevOps Engineers** managing complex AWS infrastructures
- **Database Administrators** working with Aurora DSQL and other databases
- **Site Reliability Engineers** troubleshooting production issues
- **Cloud Architects** designing and optimizing AWS solutions
- **Support Teams** providing faster, more accurate assistance

---

*TyCode: Where AWS operations meet artificial intelligence.*