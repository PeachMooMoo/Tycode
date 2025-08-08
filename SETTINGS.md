# TyCode Settings Configuration

TyCode now supports persistent configuration through a settings file located at `~/.tycode/settings.toml`. This allows you to customize models, AWS credentials, and agent-specific settings without needing to specify command-line arguments every time.

## Settings File Location

The settings file is automatically created at `~/.tycode/settings.toml` on first run. You can edit this file directly to customize your configuration.

## Configuration Structure

### Global Settings

```toml
[global]
default_model = "claude-sonnet-4"        # Default AI model to use
file_modification_api = "FindReplace"    # File modification method: "Patch" or "FindReplace"
trace = false                            # Enable trace logging
```

### AWS Configuration

```toml
[aws]
profile = "cline"        # AWS profile name
region = "us-west-2"     # AWS region

# Optional: Specific AWS credentials for different services
[aws.credentials.dsql]
account_id = "603014702751"
role = "Bedrock-Access"
region = "us-west-2"
```

### Agent-Specific Settings

You can override settings for specific agents:

```toml
[agents.software_engineer]
model = "claude-opus-4-1"    # Use Opus for complex engineering tasks
temperature = 0.7
max_tokens = 4096
reasoning_budget = 2000       # Enable reasoning for this agent
tools = ["read_file", "write_file", "list_files", "modify_file"]

[agents.design]
model = "claude-sonnet-4"     # Use Sonnet for design work
temperature = 0.5
max_tokens = 4096
tools = ["read_file", "list_files", "search_files"]
```

## Model Selection Priority

When determining which model to use, TyCode follows this priority:

1. **User-configured model** (from settings file for the specific agent)
2. **Agent's preferred model** (hardcoded agent preference)
3. **Global default model** (from settings or command line)

The CLI will show which model is being used and its source:

```
[AI] (using claude-opus-4-1 (configured)) Here's the response...
[AI] (using claude-sonnet-4 (agent default)) Here's the design...
[AI] (using claude-haiku-3-5 (global default)) Quick response...
```

## CLI Commands

### View Current Settings

Use the `/settings` command in the CLI to view your current configuration:

```
/settings
```

This will display:
- Global settings (default model, file API, etc.)
- AWS configuration
- Agent-specific overrides
- Settings file location

### Command Line Arguments

Command-line arguments always override settings from the configuration file:

```bash
# Override the model for this session
tycode-cli --model claude-haiku-3-5

# Use a different AWS profile
tycode-cli --profile production --region eu-west-1

# Disable settings file completely
tycode-cli --no-settings
```

## Available Models

The following models are available:

- `claude-opus-4-1` - Most capable, best for complex tasks
- `claude-opus-4` - Previous Opus version
- `claude-sonnet-4` - Balanced performance and cost
- `claude-sonnet-3-7` - Updated Sonnet 3.5
- `claude-sonnet-3-5-v2` - Sonnet 3.5 v2
- `claude-sonnet-3-5` - Original Sonnet 3.5
- `claude-haiku-3-5` - Fast and efficient
- `claude-haiku-3` - Previous Haiku version

## Example Settings File

Here's a complete example settings file:

```toml
version = "1.0"

[global]
default_model = "claude-sonnet-4"
file_modification_api = "FindReplace"
trace = false

[aws]
profile = "cline"
region = "us-west-2"

[aws.credentials.dsql]
account_id = "603014702751"
role = "Bedrock-Access"
region = "us-west-2"

# Software Engineer Agent - optimized for code generation
[agents.software_engineer]
model = "claude-opus-4-1"
temperature = 0.7
max_tokens = 4096
reasoning_budget = 2000
tools = ["read_file", "write_file", "list_files", "modify_file"]

# Design Agent - optimized for architecture and design
[agents.design]
model = "claude-sonnet-4"
temperature = 0.5
max_tokens = 4096
tools = ["read_file", "list_files", "search_files"]
```

## Troubleshooting

If settings aren't loading:

1. Check that `~/.tycode/settings.toml` exists
2. Verify the TOML syntax is valid
3. Run with `--no-settings` to bypass settings
4. Check for error messages on startup

The settings file will be automatically created with defaults if it doesn't exist on first run.