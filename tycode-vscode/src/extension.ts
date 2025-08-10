import * as vscode from 'vscode';
import { ChatProvider } from './chatProvider';
import { SubprocessBridge } from './subprocessBridge';

let chatProvider: ChatProvider;
let bridge: SubprocessBridge;

export async function activate(context: vscode.ExtensionContext) {
    console.log('TyCode extension is activating...');

    // Initialize subprocess bridge
    bridge = new SubprocessBridge(context);
    await bridge.initialize();

    // Create chat provider
    chatProvider = new ChatProvider(context, bridge);

    // Register webview provider
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(
            'tycode.chatView',
            chatProvider,
            {
                webviewOptions: {
                    retainContextWhenHidden: true
                }
            }
        )
    );

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('tycode.openChat', () => {
            chatProvider.openChat();
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('tycode.askAboutSelection', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }

            const selection = editor.document.getText(editor.selection);
            if (!selection) {
                vscode.window.showWarningMessage('No text selected');
                return;
            }

            // Open chat and send the selection
            chatProvider.openChat();
            await chatProvider.sendMessage(
                `Can you explain this code?\n\n\`\`\`${editor.document.languageId}\n${selection}\n\`\`\``
            );
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('tycode.applyChanges', async (changes: string) => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showWarningMessage('No active editor');
                return;
            }

            await editor.edit(editBuilder => {
                const document = editor.document;
                const fullRange = new vscode.Range(
                    document.positionAt(0),
                    document.positionAt(document.getText().length)
                );
                editBuilder.replace(fullRange, changes);
            });
        })
    );

    console.log('TyCode extension is now active!');
}

export function deactivate() {
    if (bridge) {
        bridge.dispose();
    }
}
