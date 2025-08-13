(function () {
    const vscode = acquireVsCodeApi();

    const messagesContainer = document.getElementById('messages');
    const messageInput = document.getElementById('message-input');
    const sendButton = document.getElementById('send-button');
    const clearButton = document.getElementById('clear-chat');
    const typingIndicator = document.getElementById('typing-indicator');

    // Send message when clicking send button
    sendButton.addEventListener('click', sendMessage);

    // Send message when pressing Enter (without Shift)
    messageInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            sendMessage();
        }
    });

    // Clear chat
    clearButton.addEventListener('click', () => {
        messagesContainer.innerHTML = '';
        vscode.postMessage({ type: 'clear' });
    });

    function sendMessage() {
        const message = messageInput.value.trim();
        if (!message) return;

        // Display user message
        displayMessage('user', message);

        // Clear input
        messageInput.value = '';
        messageInput.style.height = 'auto';

        // Send to extension
        vscode.postMessage({
            type: 'sendMessage',
            message: message
        });
    }

    // Keep track of current AI response group
    let currentResponseGroup = null;
    // Keep track of tool results for the current response
    let pendingToolResults = new Map();

    function displayMessage(role, content, details, model, isComplete, reasoning, toolCalls, tokenUsage) {
        // Special handling for AI response components
        if (role === 'assistant') {
            // Start a new response group
            currentResponseGroup = document.createElement('div');
            currentResponseGroup.className = 'ai-response-group';
            messagesContainer.appendChild(currentResponseGroup);

            // Create the message element
            const messageDiv = document.createElement('div');
            messageDiv.className = `message ${role}`;

            if (false) { // Remove standalone tool rendering
            } else if (role === 'assistant') {
                // Include model info if available
                const modelInfo = model ? `<div class="model-info">Model: ${model}</div>` : '';
                const completionInfo = isComplete !== undefined ?
                    `<div class="completion-info">${isComplete ? '✅ Complete' : '⏳ Pending AI response'}</div>` : '';

                // Build token usage info if available
                let tokenInfo = '';
                if (tokenUsage) {
                    tokenInfo = `<div class="token-info">📊 Tokens: ${tokenUsage.input_tokens} in, ${tokenUsage.output_tokens} out (${tokenUsage.total_tokens} total)</div>`;
                }

                // Build the reasoning section if present
                let reasoningSection = '';
                if (reasoning) {
                    const reasoningId = 'reasoning-' + Date.now();
                    const isLong = reasoning.length > 200;
                    const truncated = isLong ? reasoning.substring(0, 200) + '...' : reasoning;

                    reasoningSection = `
                        <div class="embedded-reasoning">
                            <div class="reasoning-header reasoning-header-clickable" data-reasoning-id="${reasoningId}">
                                💭 Reasoning 
                                <span class="reasoning-toggle" id="${reasoningId}-toggle">
                                    ${isLong ? '▶' : ''}
                                </span>
                            </div>
                            <div class="reasoning-content ${isLong ? 'collapsed' : ''}" id="${reasoningId}">
                                <div class="reasoning-truncated">${renderContent(truncated)}</div>
                                <div class="reasoning-full" style="display: none;">${renderContent(reasoning)}</div>
                            </div>
                        </div>
                    `;

                    // Store expansion state
                    window[`toggleReasoning_${reasoningId}`] = isLong;
                }

                // Build tool calls section if present
                let toolCallsSection = '';
                if (toolCalls && toolCalls.length > 0) {
                    const toolCallsHtml = toolCalls.map(toolCall => {
                        const toolId = `tool-${Date.now()}-${toolCall.name}`;
                        return `
                            <div class="tool-call-item" data-tool-name="${toolCall.name}" id="${toolId}">
                                <div class="tool-header">
                                    <span class="tool-status-icon">⏳</span>
                                    <span class="tool-name">${toolCall.name}</span>
                                    <span class="tool-status-text">Executing...</span>
                                </div>
                                ${toolCall.arguments ? `<div class="tool-details"><pre>${escapeHtml(JSON.stringify(toolCall.arguments, null, 2))}</pre></div>` : ''}
                                <div class="tool-result" style="display: none;"></div>
                            </div>
                        `;
                    }).join('');

                    toolCallsSection = `
                        <div class="embedded-tool-calls">
                            ${toolCallsHtml}
                        </div>
                    `;
                }

                messageDiv.innerHTML = `
                    ${modelInfo}
                    ${tokenInfo}
                    ${reasoningSection}
                    <div class="message-content">${renderContent(content)}</div>
                    ${toolCallsSection}
                    ${completionInfo}
                `;

                // Add click event listener for reasoning after adding to DOM
                if (reasoning) {
                    setTimeout(() => {
                        const header = messageDiv.querySelector('.reasoning-header-clickable');
                        if (header) {
                            header.addEventListener('click', function () {
                                const id = this.getAttribute('data-reasoning-id');
                                toggleReasoning(id);
                            });
                        }
                    }, 0);
                }

                // Add code action buttons
                addCodeActions(messageDiv);
            }

            // Add to group
            currentResponseGroup.appendChild(messageDiv);

            // Clear the group reference after assistant message (last in group)
            currentResponseGroup = null;
        } else {
            // Regular messages (user, system, error)
            const messageDiv = document.createElement('div');
            messageDiv.className = `message ${role}`;
            messageDiv.innerHTML = renderContent(content);

            // Add code action buttons for user messages too
            if (role === 'user') {
                addCodeActions(messageDiv);
            }

            messagesContainer.appendChild(messageDiv);

            // Clear any active response group when user sends a message
            if (role === 'user') {
                currentResponseGroup = null;
            }
        }

        messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }

    // Function to toggle reasoning expansion
    function toggleReasoning(reasoningId) {
        const content = document.getElementById(reasoningId);
        const toggle = document.getElementById(reasoningId + '-toggle');
        const truncated = content.querySelector('.reasoning-truncated');
        const full = content.querySelector('.reasoning-full');

        // Only toggle if there's a full version
        if (window[`toggleReasoning_${reasoningId}`]) {
            if (content.classList.contains('collapsed')) {
                content.classList.remove('collapsed');
                content.classList.add('expanded');
                truncated.style.display = 'none';
                full.style.display = 'block';
                toggle.textContent = '▼';
            } else {
                content.classList.remove('expanded');
                content.classList.add('collapsed');
                truncated.style.display = 'block';
                full.style.display = 'none';
                toggle.textContent = '▶';
            }
        }
    }

    function displayToolResult(toolName, success, result, error) {
        console.log('Tool result received:', { toolName, success, result, error });
        
        // Find the most recent tool call item with this name
        const toolItems = document.querySelectorAll(`.tool-call-item[data-tool-name="${toolName}"]`);
        if (toolItems.length === 0) {
            console.warn('No tool item found for:', toolName);
            return;
        }
        
        // Get the last one (most recent)
        const toolItem = toolItems[toolItems.length - 1];
        
        // Update status icon and text
        const statusIcon = toolItem.querySelector('.tool-status-icon');
        const statusText = toolItem.querySelector('.tool-status-text');
        const resultDiv = toolItem.querySelector('.tool-result');
        
        if (success) {
            statusIcon.textContent = '✅';
            statusText.textContent = 'Success';
            toolItem.classList.add('tool-success');
        } else {
            statusIcon.textContent = '❌';
            statusText.textContent = 'Failed';
            toolItem.classList.add('tool-error');
        }
        
        // Display result if available
        if (result || error) {
            resultDiv.style.display = 'block';
            
            // Format the result based on tool type
            let resultContent = '';
            if (error) {
                resultContent = `<div class="tool-error-message">${escapeHtml(error)}</div>`;
            } else if (result) {
                // Special formatting for different tool types
                if (toolName === 'write_file' || toolName === 'replace_in_file') {
                    // File modification tools
                    if (result.path) {
                        resultContent = `<div class="tool-success-message">✓ Modified: ${escapeHtml(result.path)}</div>`;
                        if (result.changes_applied !== undefined) {
                            resultContent += `<div class="tool-detail">Changes applied: ${result.changes_applied}</div>`;
                        }
                    } else {
                        resultContent = `<div class="tool-success-message">✓ File operation completed</div>`;
                    }
                } else if (toolName === 'delete_file') {
                    if (result.path) {
                        resultContent = `<div class="tool-success-message">✓ Deleted: ${escapeHtml(result.path)}</div>`;
                    }
                } else if (toolName === 'read_file') {
                    if (result.content) {
                        const lines = result.content.split('\n').length;
                        resultContent = `<div class="tool-success-message">✓ Read ${lines} lines</div>`;
                    }
                } else if (toolName === 'list_files') {
                    if (result.files && Array.isArray(result.files)) {
                        resultContent = `<div class="tool-success-message">✓ Found ${result.files.length} files</div>`;
                    }
                } else if (toolName === 'execute_command') {
                    if (result.exit_code !== undefined) {
                        const exitStatus = result.exit_code === 0 ? '✓' : '⚠';
                        resultContent = `<div class="tool-success-message">${exitStatus} Exit code: ${result.exit_code}</div>`;
                        if (result.stdout) {
                            resultContent += `<details><summary>Output</summary><pre>${escapeHtml(result.stdout)}</pre></details>`;
                        }
                        if (result.stderr) {
                            resultContent += `<details><summary>Errors</summary><pre>${escapeHtml(result.stderr)}</pre></details>`;
                        }
                    }
                } else {
                    // Generic result display
                    resultContent = `<pre>${escapeHtml(JSON.stringify(result, null, 2))}</pre>`;
                }
            }
            
            resultDiv.innerHTML = resultContent;
        }
        
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }

    function renderContent(content) {
        // Escape HTML first
        let rendered = escapeHtml(content);

        // Render code blocks with syntax highlighting hint
        rendered = rendered.replace(/```(\w+)?\n([\s\S]*?)```/g, (match, lang, code) => {
            return `<div class="code-block-container">
                <pre><code class="language-${lang || 'plaintext'}">${escapeHtml(code.trim())}</code></pre>
            </div>`;
        });

        // Render inline code
        rendered = rendered.replace(/`([^`]+)`/g, '<code>$1</code>');

        // Render markdown headers (h1-h6)
        // Process headers from h6 to h1 to avoid conflicts
        rendered = rendered.replace(/^######\s+(.+)$/gm, '<h6>$1</h6>');
        rendered = rendered.replace(/^#####\s+(.+)$/gm, '<h5>$1</h5>');
        rendered = rendered.replace(/^####\s+(.+)$/gm, '<h4>$1</h4>');
        rendered = rendered.replace(/^###\s+(.+)$/gm, '<h3>$1</h3>');
        rendered = rendered.replace(/^##\s+(.+)$/gm, '<h2>$1</h2>');
        rendered = rendered.replace(/^#\s+(.+)$/gm, '<h1>$1</h1>');

        // Render links
        rendered = rendered.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank">$1</a>');

        // Render bold
        rendered = rendered.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

        // Render italic
        rendered = rendered.replace(/\*([^*]+)\*/g, '<em>$1</em>');

        // Preserve line breaks
        rendered = rendered.replace(/\n/g, '<br>');

        // Clean up excessive spacing
        // Remove <br> tags that were inserted inside headers
        rendered = rendered.replace(/(<h[1-6]>.*?)<br>(.*?<\/h[1-6]>)/g, '$1 $2');
        // Remove blank lines after headers
        rendered = rendered.replace(/(<\/h[1-6]>)<br>/g, '$1');
        // Reduce multiple consecutive line breaks to just one
        rendered = rendered.replace(/(<br>){2,}/g, '<br>');
        // Remove line break before headers (tighter spacing)
        rendered = rendered.replace(/<br>(<h[1-6]>)/g, '$1');

        return rendered;
    }

    function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    function addCodeActions(messageDiv) {
        const codeBlocks = messageDiv.querySelectorAll('.code-block-container');

        codeBlocks.forEach(block => {
            const actionsDiv = document.createElement('div');
            actionsDiv.className = 'code-actions';

            // Copy button
            const copyButton = document.createElement('button');
            copyButton.className = 'code-action-button';
            copyButton.textContent = 'Copy';
            copyButton.onclick = () => {
                const code = block.querySelector('code').textContent;
                vscode.postMessage({
                    type: 'copyCode',
                    code: code
                });
            };

            // Insert button
            const insertButton = document.createElement('button');
            insertButton.className = 'code-action-button';
            insertButton.textContent = 'Insert';
            insertButton.onclick = () => {
                const code = block.querySelector('code').textContent;
                vscode.postMessage({
                    type: 'insertCode',
                    code: code
                });
            };

            actionsDiv.appendChild(copyButton);
            actionsDiv.appendChild(insertButton);
            block.appendChild(actionsDiv);
        });
    }

    // Handle messages from extension
    window.addEventListener('message', event => {
        const message = event.data;

        switch (message.type) {
            case 'displayMessage':
                displayMessage(
                    message.role,
                    message.content,
                    message.details,
                    message.model,
                    message.isComplete,
                    message.reasoning,
                    message.toolCalls,
                    message.tokenUsage
                );
                break;
            case 'toolResult':
                displayToolResult(
                    message.toolName,
                    message.success,
                    message.result,
                    message.error
                );
                break;
            case 'showTyping':
                typingIndicator.style.display = 'flex';
                messagesContainer.scrollTop = messagesContainer.scrollHeight;
                break;
            case 'hideTyping':
                typingIndicator.style.display = 'none';
                break;
        }
    });

    // Auto-resize textarea
    messageInput.addEventListener('input', () => {
        messageInput.style.height = 'auto';
        messageInput.style.height = messageInput.scrollHeight + 'px';
    });

    // Focus input on load
    messageInput.focus();
})();
