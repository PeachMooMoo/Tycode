import * as vscode from 'vscode';
import { SubprocessBridge } from './subprocessBridge';

export class SettingsProvider {
    private panel: vscode.WebviewPanel | undefined;

    constructor(
        private context: vscode.ExtensionContext,
        private bridge: SubprocessBridge
    ) {}

    public async show() {
        if (this.panel) {
            this.panel.reveal();
            return;
        }

        this.panel = vscode.window.createWebviewPanel(
            'tycodeSettings',
            'TyCode Settings',
            vscode.ViewColumn.One,
            {
                enableScripts: true,
                retainContextWhenHidden: true
            }
        );

        this.panel.webview.html = this.getWebviewContent();

        // Load current settings and send to webview
        const settings = await this.loadSettings();
        this.panel.webview.postMessage({
            type: 'loadSettings',
            settings: settings
        });

        // Handle messages from the webview
        this.panel.webview.onDidReceiveMessage(
            async message => {
                switch (message.type) {
                    case 'saveSettings':
                        await this.saveSettings(message.settings);
                        vscode.window.showInformationMessage('Settings saved successfully');
                        break;
                    case 'getSettings':
                        const currentSettings = await this.loadSettings();
                        this.panel?.webview.postMessage({
                            type: 'loadSettings',
                            settings: currentSettings
                        });
                        break;
                    case 'error':
                        vscode.window.showErrorMessage(message.message);
                        break;
                }
            },
            undefined,
            this.context.subscriptions
        );

        this.panel.onDidDispose(() => {
            this.panel = undefined;
        });
    }

    private async loadSettings(): Promise<any> {
        try {
            return await this.bridge.loadSettings();
        } catch (error) {
            console.error('Failed to load settings:', error);
            vscode.window.showErrorMessage(`Failed to load settings: ${error}`);
            return {
                active_provider: 'default',
                providers: {
                    default: {
                        type: 'bedrock',
                        profile: 'default',
                        region: 'us-west-2'
                    }
                }
            };
        }
    }

    private async saveSettings(settings: any): Promise<void> {
        try {
            await this.bridge.saveSettings(settings);
            // Reload settings in the subprocess to apply changes
            await this.bridge.reloadSettings();
        } catch (error) {
            console.error('Failed to save settings:', error);
            vscode.window.showErrorMessage(`Failed to save settings: ${error}`);
            throw error;
        }
    }

    private getWebviewContent(): string {
        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>TyCode Settings</title>
    <style>
        body {
            font-family: var(--vscode-font-family);
            color: var(--vscode-foreground);
            background-color: var(--vscode-editor-background);
            padding: 20px;
            max-width: 800px;
            margin: 0 auto;
        }
        
        h1 {
            font-size: 24px;
            margin-bottom: 20px;
            border-bottom: 1px solid var(--vscode-panel-border);
            padding-bottom: 10px;
        }
        
        .section {
            margin-bottom: 30px;
        }
        
        .section-title {
            font-size: 18px;
            margin-bottom: 15px;
            font-weight: 600;
        }
        
        .provider-list {
            border: 1px solid var(--vscode-panel-border);
            border-radius: 4px;
            padding: 15px;
            margin-bottom: 20px;
        }
        
        .provider-item {
            padding: 10px;
            margin-bottom: 10px;
            border: 1px solid var(--vscode-panel-border);
            border-radius: 4px;
            background-color: var(--vscode-editor-inactiveSelectionBackground);
        }
        
        .provider-item.active {
            background-color: var(--vscode-editor-selectionBackground);
            border-color: var(--vscode-focusBorder);
        }
        
        .provider-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 10px;
        }
        
        .provider-name {
            font-weight: 600;
            display: flex;
            align-items: center;
            gap: 10px;
        }
        
        .provider-details {
            margin-left: 25px;
            color: var(--vscode-descriptionForeground);
        }
        
        .provider-actions {
            display: flex;
            gap: 10px;
        }
        
        button {
            background-color: var(--vscode-button-background);
            color: var(--vscode-button-foreground);
            border: none;
            padding: 6px 14px;
            border-radius: 2px;
            cursor: pointer;
            font-size: 13px;
        }
        
        button:hover {
            background-color: var(--vscode-button-hoverBackground);
        }
        
        button.danger {
            background-color: #f14c4c;
            color: white;
        }
        
        button.danger:hover {
            background-color: #d73333;
        }
        
        button.primary {
            background-color: var(--vscode-button-background);
        }
        
        .add-provider-btn {
            margin-bottom: 20px;
        }
        
        .form-group {
            margin-bottom: 15px;
        }
        
        label {
            display: block;
            margin-bottom: 5px;
            font-weight: 500;
        }
        
        input[type="text"], select {
            width: 100%;
            padding: 6px 8px;
            background-color: var(--vscode-input-background);
            color: var(--vscode-input-foreground);
            border: 1px solid var(--vscode-input-border);
            border-radius: 2px;
        }
        
        input[type="radio"] {
            margin-right: 5px;
        }
        
        .help-text {
            font-size: 12px;
            color: var(--vscode-descriptionForeground);
            margin-top: 4px;
        }
        
        .modal {
            display: none;
            position: fixed;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background-color: rgba(0, 0, 0, 0.5);
            z-index: 1000;
        }
        
        .modal-content {
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            background-color: var(--vscode-editor-background);
            border: 1px solid var(--vscode-panel-border);
            border-radius: 4px;
            padding: 20px;
            min-width: 400px;
            max-width: 500px;
        }
        
        .modal-header {
            font-size: 18px;
            margin-bottom: 20px;
            font-weight: 600;
        }
        
        .modal-footer {
            display: flex;
            justify-content: flex-end;
            gap: 10px;
            margin-top: 20px;
        }
        
        .actions {
            display: flex;
            justify-content: flex-end;
            gap: 10px;
            margin-top: 30px;
            padding-top: 20px;
            border-top: 1px solid var(--vscode-panel-border);
        }
    </style>
</head>
<body>
    <h1>TyCode Settings</h1>
    
    <div class="section">
        <div class="section-title">Provider Configurations</div>
        <div class="provider-list" id="providerList">
            <!-- Providers will be dynamically added here -->
        </div>
        <button class="add-provider-btn" onclick="showAddProviderModal()">+ Add Provider</button>
    </div>
    
    <div class="actions">
        <button class="primary" onclick="saveSettings()">Save Settings</button>
    </div>
    
    <!-- Add/Edit Provider Modal -->
    <div id="providerModal" class="modal">
        <div class="modal-content">
            <div class="modal-header" id="modalTitle">Add Provider</div>
            <div class="form-group">
                <label for="providerName">Name</label>
                <input type="text" id="providerName" placeholder="e.g., personal, work, dev">
                <div class="help-text">A unique name for this provider configuration</div>
            </div>
            <div class="form-group">
                <label for="providerType">Type</label>
                <select id="providerType">
                    <option value="bedrock">AWS Bedrock</option>
                </select>
            </div>
            <div id="providerFields">
                <!-- Dynamic fields based on provider type -->
            </div>
            <div class="modal-footer">
                <button onclick="closeModal()">Cancel</button>
                <button class="primary" onclick="saveProvider()">Save</button>
            </div>
        </div>
    </div>
    
    <!-- Delete Confirmation Modal -->
    <div id="deleteConfirmModal" class="modal">
        <div class="modal-content" style="max-width: 400px;">
            <div class="modal-header">Confirm Delete</div>
            <div style="margin: 20px 0;">
                Are you sure you want to delete the provider "<span id="deleteProviderName"></span>"?
            </div>
            <div class="modal-footer">
                <button onclick="cancelDelete()">Cancel</button>
                <button class="danger" onclick="confirmDelete()">Delete</button>
            </div>
        </div>
    </div>
    
    <script>
        const vscode = acquireVsCodeApi();
        let settings = {
            active_provider: 'default',
            providers: {}
        };
        let editingProvider = null;
        let deletingProvider = null;
        
        // Listen for messages from extension
        window.addEventListener('message', event => {
            const message = event.data;
            switch (message.type) {
                case 'loadSettings':
                    settings = message.settings;
                    renderProviders();
                    break;
            }
        });
        
        function renderProviders() {
            const list = document.getElementById('providerList');
            list.innerHTML = '';
            
            if (!settings.providers || Object.keys(settings.providers).length === 0) {
                list.innerHTML = '<div style="color: var(--vscode-descriptionForeground);">No providers configured</div>';
                return;
            }
            
            for (const [name, config] of Object.entries(settings.providers)) {
                const isActive = name === settings.active_provider;
                const item = document.createElement('div');
                item.className = 'provider-item' + (isActive ? ' active' : '');
                
                let providerInfo = '';
                if (config.type === 'bedrock') {
                    providerInfo = \`Profile: \${config.profile}, Region: \${config.region || 'us-west-2'}\`;
                }
                
                item.innerHTML = \`
                    <div class="provider-header">
                        <div class="provider-name">
                            <input type="radio" name="activeProvider" value="\${name}" 
                                \${isActive ? 'checked' : ''} 
                                onchange="setActiveProvider('\${name}')">
                            <span>\${name} (AWS Bedrock)</span>
                        </div>
                        <div class="provider-actions">
                            <button onclick="editProvider('\${name}')">Edit</button>
                            <button class="danger" onclick="deleteProvider('\${name}')">Delete</button>
                        </div>
                    </div>
                    <div class="provider-details">\${providerInfo}</div>
                \`;
                
                list.appendChild(item);
            }
        }
        
        function setActiveProvider(name) {
            settings.active_provider = name;
        }
        
        function showAddProviderModal() {
            editingProvider = null;
            document.getElementById('modalTitle').textContent = 'Add Provider';
            document.getElementById('providerName').value = '';
            document.getElementById('providerName').disabled = false;
            document.getElementById('providerType').value = 'bedrock';
            updateProviderFields('bedrock');
            document.getElementById('providerModal').style.display = 'block';
        }
        
        function editProvider(name) {
            editingProvider = name;
            const config = settings.providers[name];
            document.getElementById('modalTitle').textContent = 'Edit Provider';
            document.getElementById('providerName').value = name;
            document.getElementById('providerName').disabled = true;
            document.getElementById('providerType').value = config.type;
            updateProviderFields(config.type, config);
            document.getElementById('providerModal').style.display = 'block';
        }
        
        function updateProviderFields(type, config = {}) {
            const fieldsDiv = document.getElementById('providerFields');
            
            if (type === 'bedrock') {
                fieldsDiv.innerHTML = \`
                    <div class="form-group">
                        <label for="awsProfile">AWS Profile</label>
                        <input type="text" id="awsProfile" value="\${config.profile || 'default'}" placeholder="default">
                        <div class="help-text">AWS profile name from ~/.aws/credentials</div>
                    </div>
                    <div class="form-group">
                        <label for="awsRegion">AWS Region</label>
                        <input type="text" id="awsRegion" value="\${config.region || 'us-west-2'}" placeholder="us-west-2">
                        <div class="help-text">AWS region (e.g., us-west-2, us-east-1)</div>
                    </div>
                \`;
            }
        }
        
        function closeModal() {
            document.getElementById('providerModal').style.display = 'none';
        }
        
        function saveProvider() {
            const name = document.getElementById('providerName').value.trim();
            const type = document.getElementById('providerType').value;
            
            if (!name) {
                vscode.postMessage({
                    type: 'error',
                    message: 'Provider name is required'
                });
                return;
            }
            
            if (!editingProvider && settings.providers[name]) {
                vscode.postMessage({
                    type: 'error',
                    message: 'Provider with this name already exists'
                });
                return;
            }
            
            let config = { type };
            
            if (type === 'bedrock') {
                const profile = document.getElementById('awsProfile').value.trim() || 'default';
                const region = document.getElementById('awsRegion').value.trim() || 'us-west-2';
                config.profile = profile;
                config.region = region;
            }
            
            settings.providers[name] = config;
            
            // If this is the first provider, make it active
            if (Object.keys(settings.providers).length === 1) {
                settings.active_provider = name;
            }
            
            closeModal();
            renderProviders();
        }
        
        function deleteProvider(name) {
            if (name === settings.active_provider) {
                vscode.postMessage({
                    type: 'error',
                    message: 'Cannot delete the active provider'
                });
                return;
            }
            
            // Show confirmation modal
            deletingProvider = name;
            document.getElementById('deleteProviderName').textContent = name;
            document.getElementById('deleteConfirmModal').style.display = 'block';
        }
        
        function confirmDelete() {
            if (deletingProvider) {
                delete settings.providers[deletingProvider];
                renderProviders();
                deletingProvider = null;
            }
            document.getElementById('deleteConfirmModal').style.display = 'none';
        }
        
        function cancelDelete() {
            deletingProvider = null;
            document.getElementById('deleteConfirmModal').style.display = 'none';
        }
        
        function saveSettings() {
            vscode.postMessage({
                type: 'saveSettings',
                settings: settings
            });
        }
        
        // Initial load
        vscode.postMessage({ type: 'getSettings' });
    </script>
</body>
</html>`;
    }

    public dispose() {
        if (this.panel) {
            this.panel.dispose();
        }
    }
}