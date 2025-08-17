import * as vscode from 'vscode';
import { MainProvider } from './mainProvider';
import { SettingsProvider } from './settingsProvider';
import { SubprocessBridge } from './subprocessBridge';

let mainProvider: MainProvider;
let settingsProvider: SettingsProvider;
let settingsBridge: SubprocessBridge;

export async function activate(context: vscode.ExtensionContext) {
    console.log('TyCode extension is activating...');

    // Create providers
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

    // Register settings command
    context.subscriptions.push(
        vscode.commands.registerCommand('tycode.openSettings', async () => {
            // Create a settings bridge on demand
            if (!settingsBridge) {
                const workspaceRoots = vscode.workspace.workspaceFolders?.map(f => f.uri.fsPath) || [];
                
                settingsBridge = new SubprocessBridge(context, workspaceRoots);
                await settingsBridge.initialize();
                settingsProvider = new SettingsProvider(context, settingsBridge);
            }
            settingsProvider.show();
        })
    );

    console.log('TyCode extension is now active!');
}

export function deactivate() {
    if (mainProvider) {
        mainProvider.dispose();
    }
    if (settingsProvider) {
        settingsProvider.dispose();
    }
    if (settingsBridge) {
        settingsBridge.dispose();
    }
}
