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

    constructor(private context: vscode.ExtensionContext) {
        super();
    }

    async initialize(): Promise<void> {
        if (this.child) {
            return;
        }

        // Get the path to tycode binary
        const cliPath = this.getBinaryPath();

        // Get AWS profile from settings
        const config = vscode.workspace.getConfiguration('tycode');
        const awsProfile = config.get<string>('awsProfile') || 'cline';

        // Check if the binary exists
        const fs = require('fs');
        if (!fs.existsSync(cliPath)) {
            throw new Error(`tycode binary not found at: ${cliPath}`);
        }

        // Get the current workspace folder
        const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
        const cwd = workspaceFolder ? workspaceFolder.uri.fsPath : process.cwd();

        console.log('Spawning subprocess:', cliPath);
        console.log('Working directory:', cwd);

        // Spawn the subprocess with the workspace folder as working directory
        this.child = spawn(cliPath, ['--subprocess', '--profile', awsProfile], {
            stdio: ['pipe', 'pipe', 'pipe'],
            cwd: cwd
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
                    console.log('Subprocess bridge initialized successfully');
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

    private handleMessage(message: SubprocessMessage) {
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

        // Map platform/arch to directory names
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

        // Try bundled binary first (production)
        let binaryPath = path.join(this.context.extensionPath, 'binaries', platformDir, binaryName);
        if (fs.existsSync(binaryPath)) {
            console.log('Using bundled binary:', binaryPath);
            return binaryPath;
        }

        // Fall back to development paths
        const devPaths = [
            path.join(this.context.extensionPath, '..', 'target', 'debug', binaryName),
            path.join(this.context.extensionPath, '..', 'target', 'release', binaryName)
        ];

        for (const devPath of devPaths) {
            if (fs.existsSync(devPath)) {
                console.log('Using development binary:', devPath);
                return devPath;
            }
        }

        throw new Error(`tycode binary not found. Searched: ${binaryPath}, ${devPaths.join(', ')}`);
    }

    dispose(): void {
        if (this.child) {
            this.child.kill();
            this.child = null;
        }
        this.removeAllListeners();
    }
}
