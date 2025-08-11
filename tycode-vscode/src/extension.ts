import * as vscode from 'vscode';
import { MainProvider } from './mainProvider';

let mainProvider: MainProvider;

export async function activate(context: vscode.ExtensionContext) {
    console.log('TyCode extension is activating...');

    // Create main provider
    mainProvider = new MainProvider(context);

    // Register webview provider
    context.subscriptions.push(
        vscode.window.registerWebviewViewProvider(
            'tycode.chatView',
            mainProvider,
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
            mainProvider.openChat();
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
            mainProvider.openChat();
            await mainProvider.sendMessageToActiveChat(
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
    if (mainProvider) {
        mainProvider.dispose();
    }
}
