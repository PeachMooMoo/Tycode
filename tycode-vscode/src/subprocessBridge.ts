import * as vscode from 'vscode';
import * as path from 'path';
import { spawn, ChildProcess } from 'child_process';
import { EventEmitter } from 'events';

interface SubprocessMessage {
    type: string;
    [key: string]: any;
}

export class SubprocessBridge extends EventEmitter {
    private child: ChildProcess | null = null;
    private buffer: string = '';
    private workspaceRoots?: string[];
    private settings?: any;

    constructor(private context: vscode.ExtensionContext, workspaceRoots?: string[]) {
        super();
        this.workspaceRoots = workspaceRoots;
    }

    getSettings(): any {
        return this.settings;
    }

    async initialize(): Promise<void> {
        if (this.child) {
            return;
        }

        // Get the path to tycode binary
        const cliPath = this.getBinaryPath();

        // Check if the binary exists
        const fs = require('fs');
        if (!fs.existsSync(cliPath)) {
            throw new Error(`tycode binary not found at: ${cliPath}`);
        }

        // Get all workspace folders
        const workspaceFolders = vscode.workspace.workspaceFolders;
        let workspaceRoots: string[] = [];
        let cwd: string;

        if (workspaceFolders && workspaceFolders.length > 0) {
            // Collect all workspace roots
            workspaceRoots = workspaceFolders.map(folder => folder.uri.fsPath);
            // Use first workspace folder as working directory
            cwd = workspaceFolders[0].uri.fsPath;
        } else {
            // No workspace folders, use current working directory
            cwd = process.cwd();
            workspaceRoots = [cwd];
        }

        console.log('Spawning subprocess:', cliPath);
        console.log('Working directory:', cwd);
        console.log('Workspace roots:', workspaceRoots);

        // Build command arguments
        const args = ['--subprocess'];
        
        // Add workspace roots if we have multiple
        if (workspaceRoots.length > 0) {
            args.push('--workspace-roots', workspaceRoots.join(','));
        }

        // Spawn the subprocess with the workspace folders
        this.child = spawn(cliPath, args, {
            stdio: ['pipe', 'pipe', 'pipe'],
            cwd: cwd,
            env: process.env
        });

        // Handle stdout (messages from CLI)
        this.child.stdout?.on('data', (data: Buffer) => {
            this.buffer += data.toString();

            // Process complete lines
            let lines = this.buffer.split('\n');
            this.buffer = lines.pop() || '';

            for (const line of lines) {
                if (line.trim()) {
                    try {
                        const message: SubprocessMessage = JSON.parse(line);
                        this.handleMessage(message);
                    } catch (e) {
                        console.error('Failed to parse message from subprocess:', line, e);
                    }
                }
            }
        });

        // Handle stderr (debug output)
        this.child.stderr?.on('data', (data: Buffer) => {
            console.error('Subprocess stderr:', data.toString());
        });

        // Handle subprocess exit
        this.child.on('exit', (code) => {
            console.log('Subprocess exited with code:', code);
            this.child = null;
            this.emit('disconnected');
        });

        // Wait for ready signal
        return new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Subprocess initialization timeout'));
            }, 5000);

            const readyHandler = (message: SubprocessMessage) => {
                if (message.type === 'Ready') {
                    clearTimeout(timeout);
                    this.off('message', readyHandler);
                    this.settings = message.settings;
                    console.log('Subprocess bridge initialized successfully with settings:', this.settings);
                    resolve();
                }
            };

            this.on('message', readyHandler);
        });
    }

    async sendMessage(message: string): Promise<void> {
        if (!this.child || !this.child.stdin) {
            throw new Error('Subprocess not initialized');
        }

        const msg: SubprocessMessage = {
            type: 'Chat',
            message
        };

        this.child.stdin.write(JSON.stringify(msg) + '\n');
    }

    async sendCancel(): Promise<void> {
        if (!this.child || !this.child.stdin) {
            throw new Error('Subprocess not initialized');
        }

        const msg: SubprocessMessage = {
            type: 'Cancel'
        };

        this.child.stdin.write(JSON.stringify(msg) + '\n');
    }

    async changeProvider(provider: string): Promise<void> {
        if (!this.child || !this.child.stdin) {
            throw new Error('Subprocess not initialized');
        }

        const msg: SubprocessMessage = {
            type: 'ChangeProvider',
            provider
        };

        this.child.stdin.write(JSON.stringify(msg) + '\n');
    }

    async loadSettings(): Promise<any> {
        if (!this.child || !this.child.stdin) {
            throw new Error('Subprocess not initialized');
        }

        return new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Settings load timeout'));
            }, 5000);

            const settingsHandler = (message: SubprocessMessage) => {
                if (message.type === 'SettingsLoaded') {
                    clearTimeout(timeout);
                    this.off('message', settingsHandler);
                    this.settings = message.settings; // Update cached settings
                    resolve(message.settings);
                }
            };

            this.on('message', settingsHandler);

            const msg: SubprocessMessage = {
                type: 'LoadSettings'
            };

            this.child!.stdin!.write(JSON.stringify(msg) + '\n');
        });
    }

    async reloadSettings(): Promise<void> {
        if (!this.child || !this.child.stdin) {
            throw new Error('Subprocess not initialized');
        }

        // First, send reload message to chat actor to reload settings from disk
        const msg: SubprocessMessage = {
            type: 'ReloadSettings'
        };
        this.child.stdin.write(JSON.stringify(msg) + '\n');

        // Wait a bit for the reload to complete
        await new Promise(resolve => setTimeout(resolve, 100));

        // Now load the fresh settings and update cache
        const settings = await this.loadSettings();
        this.settings = settings;
        
        // Emit an event so all ChatProviders can update their UI
        this.emit('settingsReloaded', settings);
    }

    async saveSettings(settings: any): Promise<void> {
        if (!this.child || !this.child.stdin) {
            throw new Error('Subprocess not initialized');
        }

        return new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error('Settings save timeout'));
            }, 5000);

            const saveHandler = (message: SubprocessMessage) => {
                if (message.type === 'SettingsSaved') {
                    clearTimeout(timeout);
                    this.off('message', saveHandler);
                    if (message.success) {
                        resolve();
                    } else {
                        reject(new Error(message.error || 'Failed to save settings'));
                    }
                }
            };

            this.on('message', saveHandler);

            const msg: SubprocessMessage = {
                type: 'SaveSettings',
                settings
            };

            this.child!.stdin!.write(JSON.stringify(msg) + '\n');
        });
    }

    private handleMessage(message: SubprocessMessage) {
        console.log('[SubprocessBridge] Received message:', message.type, message);
        this.emit('message', message);

        // Emit specific events based on message type
        switch (message.type) {
            case 'Response':
                // New format includes more data
                this.emit('response', {
                    content: message.content,
                    reasoning: message.reasoning,
                    tool_calls: message.tool_calls || [],
                    model: message.model,
                    is_complete: message.is_complete,
                    context_info: message.context_info,
                    token_usage: message.token_usage
                });
                break;
            case 'ToolResult':
                console.log('[SubprocessBridge] Emitting toolResult event:', message);
                this.emit('toolResult', {
                    tool_name: message.tool_name,
                    success: message.success,
                    result: message.result,
                    error: message.error
                });
                break;
            case 'Event':
                this.emit('event', message.event, message.data);
                break;
            case 'Error':
                this.emit('error', message.error);
                break;
        }
    }

    private getBinaryPath(): string {
        const fs = require('fs');

        // Determine platform-specific binary name
        const platform = process.platform;
        const arch = process.arch;
        const binaryName = platform === 'win32' ? 'tycode.exe' : 'tycode';

        // Check development paths first (prioritize fresh builds during development)
        const devPaths = [
            path.join(this.context.extensionPath, '..', 'target', 'release', binaryName),
            path.join(this.context.extensionPath, '..', 'target', 'debug', binaryName)
        ];

        for (const devPath of devPaths) {
            if (fs.existsSync(devPath)) {
                console.log('Using development binary:', devPath);
                return devPath;
            }
        }

        // Fall back to bundled binary (production)
        let platformDir: string;
        if (platform === 'darwin') {
            platformDir = arch === 'arm64' ? 'darwin-arm64' : 'darwin-x64';
        } else if (platform === 'linux') {
            platformDir = 'linux-x64';
        } else if (platform === 'win32') {
            platformDir = 'win32-x64';
        } else {
            throw new Error(`Unsupported platform: ${platform}`);
        }

        let binaryPath = path.join(this.context.extensionPath, 'binaries', platformDir, binaryName);
        if (fs.existsSync(binaryPath)) {
            console.log('Using bundled binary:', binaryPath);
            return binaryPath;
        }

        throw new Error(`tycode binary not found. Searched development paths: ${devPaths.join(', ')} and bundled path: ${binaryPath}`);
    }

    dispose(): void {
        if (this.child) {
            this.child.kill();
            this.child = null;
        }
        this.removeAllListeners();
    }
}
