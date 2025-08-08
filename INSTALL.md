# Tycode Installation Guide

This project includes automated installation scripts to make the binaries available system-wide.

## Quick Setup

```bash
# Make scripts executable and run setup
chmod +x setup-auto-install.sh
./setup-auto-install.sh
```

## Installation Methods

### Method 1: Direct Installation
```bash
./install.sh
# or
cargo install  # (using cargo alias)
```

This will:
- Build the project in release mode
- Install binaries to `~/.local/bin` (or `/usr/local/bin` if run as root)
- Create convenient symlinks:
  - `tycode` → `tycode-chat`
  - `tycli` → `tycode-cli`
- Add the install directory to PATH if needed

### Method 2: Auto-Install with Cargo Run
Use the provided cargo aliases to automatically install after building:

```bash
# Run and auto-install
cargo run-install -- [args]

# Build release and auto-install
cargo build-install
```

### Method 3: Shell Alias for Auto-Installation
Add this to your `~/.bashrc` or `~/.zshrc`:

```bash
alias cargo-run='bash /path/to/project/cargo-run-with-install.sh run'
```

Then every `cargo-run` will automatically update the installed binaries.

## Installed Binaries

After installation, you'll have:
- `tycode-chat` - Terminal chat interface (also available as `tycode`)
- `tycode-cli` - Command-line interface (also available as `tycli`)

## Uninstallation

To remove the installed binaries:
```bash
rm -f ~/.local/bin/tycode* ~/.local/bin/tycli
# or if installed system-wide:
sudo rm -f /usr/local/bin/tycode* /usr/local/bin/tycli
```

## Notes

- The installation script automatically detects if you're running as root
- User installations go to `~/.local/bin`
- System-wide installations (with sudo) go to `/usr/local/bin`
- The script will offer to add the install directory to your PATH if needed