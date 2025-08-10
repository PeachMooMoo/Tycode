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

    function displayMessage(role, content, details, model, isComplete, reasoning, toolCalls) {
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
                    `<div class="completion-info">${isComplete ? '✅ Complete' : '⏳ Pending tool execution'}</div>` : '';

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
                    const toolCallsHtml = toolCalls.map(toolCall => `
                        <div class="tool-call-item">
                            <div class="tool-header">🔧 ${toolCall.name}</div>
                            ${toolCall.arguments ? `<div class="tool-details"><pre>${escapeHtml(JSON.stringify(toolCall.arguments, null, 2))}</pre></div>` : ''}
                        </div>
                    `).join('');

                    toolCallsSection = `
                        <div class="embedded-tool-calls">
                            ${toolCallsHtml}
                        </div>
                    `;
                }

                messageDiv.innerHTML = `
                    ${modelInfo}
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

        // Render links
        rendered = rendered.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank">$1</a>');

        // Render bold
        rendered = rendered.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

        // Render italic
        rendered = rendered.replace(/\*([^*]+)\*/g, '<em>$1</em>');

        // Preserve line breaks
        rendered = rendered.replace(/\n/g, '<br>');

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
                    message.toolCalls
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
