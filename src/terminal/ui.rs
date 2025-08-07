use crate::terminal::{events::ChatMessage, splash::TYCODE_ASCII};
use ratatui::prelude::StatefulWidget;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
    Frame,
};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use tui_scrollview::{ScrollView, ScrollViewState, ScrollbarVisibility};
use tui_textarea::TextArea;

use crate::ai::types::{Model, ModelTunings};

pub struct UI {
    pub status_message: Option<(String, Instant)>,
}

impl UI {
    pub fn new() -> Self {
        Self {
            status_message: None,
        }
    }

    pub fn set_status(&mut self, message: String) {
        self.status_message = Some((message, Instant::now()));
    }

    pub fn render_all(
        &self,
        f: &mut Frame,
        messages: &VecDeque<ChatMessage>,
        input: &TextArea,
        is_typing: bool,
        model: &Model,
        tunings: &ModelTunings,
        show_help: bool,
        show_settings: bool,
        system_prompt: &str,
        file_modification_api: &crate::chat::state::FileModificationApi,
        scroll_state: &mut ScrollViewState,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // Chat area
                Constraint::Length(3), // Input area
                Constraint::Length(1), // Status area
            ])
            .split(f.area());

        self.render_messages(f, chunks[0], messages, scroll_state);
        self.render_input(f, chunks[1], input, is_typing);
        self.render_status(f, chunks[2], model, tunings, is_typing);

        if show_help {
            self.render_help(f);
        }

        if show_settings {
            self.render_settings(f, model, tunings, system_prompt, file_modification_api);
        }
    }

    pub fn render_messages(
        &self,
        f: &mut Frame,
        area: Rect,
        messages: &VecDeque<ChatMessage>,
        scroll_state: &mut ScrollViewState,
    ) {
        let block = Block::default()
            .title(" Chat ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        let inner = block.inner(area);

        // Calculate timestamp width to account for it in wrapping
        let timestamp_width = "[00:00:00] ".len(); // Typical timestamp format

        // Calculate total content height needed
        // Account for scrollbar + borders + timestamp + extra padding to prevent text cutoff
        let wrap_width = (inner.width as usize).saturating_sub(timestamp_width + 6);
        let mut total_height = 0u16;

        for msg in messages.iter() {
            let mut msg_height = 0u16;

            // Count lines for main content
            if !msg.content.trim().is_empty() {
                let content = format!("{}: {}", msg.sender.prefix(), msg.content);
                let wrapped_content = textwrap::fill(&content, wrap_width);
                msg_height += wrapped_content.lines().count() as u16;
            }

            // Count lines for reasoning
            if let Some(reasoning) = &msg.reasoning {
                msg_height += 2; // Empty line + "Reasoning:" line
                let reasoning_text = textwrap::fill(&reasoning.text, wrap_width.saturating_sub(4));
                msg_height += reasoning_text.lines().count() as u16;
            }

            // Count lines for tool calls
            if !msg.tool_calls.is_empty() {
                msg_height += 2; // Empty line + "Tool Usage:" line
                msg_height += msg.tool_calls.len() as u16;
            }

            // Add spacing between messages
            msg_height += 1;
            total_height += msg_height;
        }

        // Create ScrollView with calculated content size (no horizontal scrolling)
        let content_size = Size::new(inner.width, total_height.max(inner.height));
        let mut scroll_view = ScrollView::new(content_size)
            .horizontal_scrollbar_visibility(ScrollbarVisibility::Never);

        // Render messages into the scroll view buffer
        self.render_messages_into_scrollview(&mut scroll_view, messages, wrap_width);

        // Render the scroll view
        scroll_view.render(inner, f.buffer_mut(), scroll_state);

        // Render the block border
        f.render_widget(block, area);
    }

    fn render_messages_into_scrollview(
        &self,
        scroll_view: &mut ScrollView,
        messages: &VecDeque<ChatMessage>,
        wrap_width: usize,
    ) {
        let mut y_offset = 0u16;
        let buf = scroll_view.buf_mut();

        for msg in messages.iter() {
            // Render main message content
            if !msg.content.trim().is_empty() {
                let timestamp_str = format!(
                    "[{}] ",
                    chrono::DateTime::<chrono::Local>::from(
                        std::time::SystemTime::now() - msg.timestamp.elapsed()
                    )
                    .format("%H:%M:%S")
                );

                let content = format!("{}: {}", msg.sender.prefix(), msg.content);
                let wrapped_content = textwrap::fill(&content, wrap_width);

                for (i, line) in wrapped_content.lines().enumerate() {
                    let line_content = if i == 0 {
                        Line::from(vec![
                            Span::styled(
                                timestamp_str.clone(),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(line.to_string(), Style::default().fg(msg.sender.color())),
                        ])
                    } else {
                        let indent = " ".repeat(timestamp_str.len());
                        Line::from(vec![
                            Span::styled(indent, Style::default().fg(Color::DarkGray)),
                            Span::styled(line.to_string(), Style::default().fg(msg.sender.color())),
                        ])
                    };

                    let paragraph = Paragraph::new(vec![line_content]);
                    // Use full available width for rendering (timestamp + content)
                    let render_width =
                        (wrap_width + timestamp_str.len()).min(buf.area.width as usize);
                    let rect = Rect::new(0, y_offset, render_width as u16, 1);
                    paragraph.render(rect, buf);
                    y_offset += 1;
                }
            }

            // Render reasoning if present
            if let Some(reasoning) = &msg.reasoning {
                y_offset += 1; // Empty line

                let reasoning_header = Line::from(vec![
                    Span::styled("  🧠 ", Style::default().fg(Color::Magenta)),
                    Span::styled(
                        "Reasoning:",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);

                let paragraph = Paragraph::new(vec![reasoning_header]);
                let rect = Rect::new(0, y_offset, wrap_width as u16, 1);
                paragraph.render(rect, buf);
                y_offset += 1;

                let reasoning_text = textwrap::fill(&reasoning.text, wrap_width.saturating_sub(4));
                for line in reasoning_text.lines() {
                    let line_content = Line::from(vec![
                        Span::styled("     ", Style::default()),
                        Span::styled(line.to_string(), Style::default().fg(Color::Magenta)),
                    ]);

                    let paragraph = Paragraph::new(vec![line_content]);
                    let rect = Rect::new(0, y_offset, wrap_width as u16, 1);
                    paragraph.render(rect, buf);
                    y_offset += 1;
                }
            }

            // Render tool calls if present
            if !msg.tool_calls.is_empty() {
                y_offset += 1; // Empty line

                let tool_header = Line::from(vec![
                    Span::styled("  🔧 ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        "Tool Usage:",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);

                let paragraph = Paragraph::new(vec![tool_header]);
                let rect = Rect::new(0, y_offset, wrap_width as u16, 1);
                paragraph.render(rect, buf);
                y_offset += 1;

                for tool_call in &msg.tool_calls {
                    let tool_line = Line::from(vec![
                        Span::styled("     • ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            tool_call.name.clone(),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" with args: ", Style::default().fg(Color::Cyan)),
                        Span::styled(
                            tool_call.arguments.to_string(),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]);

                    let paragraph = Paragraph::new(vec![tool_line]);
                    let rect = Rect::new(0, y_offset, wrap_width as u16, 1);
                    paragraph.render(rect, buf);
                    y_offset += 1;
                }
            }

            // Add spacing between messages
            y_offset += 1;
        }
    }

    pub fn render_input(&self, f: &mut Frame, area: Rect, input: &TextArea, is_typing: bool) {
        let title = if is_typing {
            " Input (AI is typing...) "
        } else {
            " Input (Enter to send) "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(if is_typing {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            });

        // Clone the textarea and set the block
        let mut textarea = input.clone();
        textarea.set_block(block);

        // Render the textarea widget
        f.render_widget(&textarea, area);
    }

    pub fn render_status(
        &self,
        f: &mut Frame,
        area: Rect,
        model: &Model,
        tunings: &ModelTunings,
        is_typing: bool,
    ) {
        let mut status_text = format!(" {} | ", model.name());

        // Add tuning info if set
        if let Some(temp) = tunings.temperature {
            status_text.push_str(&format!("T:{:.2} | ", temp));
        }
        if let Some(max) = tunings.max_tokens {
            status_text.push_str(&format!("Max:{} | ", max));
        }
        if let Some(budget) = tunings.reasoning_budget {
            status_text.push_str(&format!("Reasoning:{} | ", budget));
        }

        if is_typing {
            status_text.push_str("🤖 AI is thinking... | ");
        }

        status_text
            .push_str("Shift+↑/↓: Scroll | Page Up/Down: Fast scroll | F1: Help | F2: Settings");

        if let Some((ref message, timestamp)) = self.status_message {
            if timestamp.elapsed() < Duration::from_secs(3) {
                status_text = format!(" {} ", message);
            }
            // Line::from("  Ctrl+V/Cmd+V   - Paste from clipboard");
        }

        let style = if is_typing {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        };

        let paragraph = Paragraph::new(status_text).style(style);

        f.render_widget(paragraph, area);
    }

    pub fn render_help(&self, f: &mut Frame) {
        let area = centered_rect(80, 70, f.area());

        f.render_widget(Clear, area);

        let help_text = vec![
            Line::from("📚 TyCode Chat Help"),
            Line::from(""),
            Line::from("🔧 Commands:"),
            Line::from("  /help          - Toggle this help"),
            Line::from("  /settings      - Show settings"),
            Line::from("  /clear         - Clear conversation"),
            Line::from("  /model <name>  - Change AI model"),
            Line::from("  /reasoning [budget] - Enable/disable reasoning"),
            Line::from("  /fileapi <type> - Set file modification API"),
            Line::from("                   (patch|findreplace)"),
            Line::from(""),
            Line::from("🚀 CLI Parameters:"),
            Line::from("  -t, --temperature  - Set response temperature (0.0-1.0)"),
            Line::from("  --max-tokens       - Set max output tokens"),
            Line::from("  --top-p           - Set top-p sampling (0.0-1.0)"),
            Line::from("  --reasoning-budget - Set reasoning token budget"),
            Line::from(""),
            Line::from("⌨️  Keyboard Shortcuts:"),
            Line::from("  Enter          - Send message"),
            Line::from("  Enter          - New line"),
            Line::from("  ↑/↓            - Navigate input history"),
            Line::from("  Shift+↑/↓      - Scroll messages"),
            Line::from("  Page Up/Down   - Fast scroll"),
            Line::from("  Home/End       - Scroll to top/bottom"),
            Line::from("  F1             - Toggle help"),
            Line::from("  F2             - Toggle settings"),
            Line::from("  ESC            - Close dialogs"),
            Line::from(""),
            Line::from("✨ Features:"),
            Line::from("  • Rich text rendering"),
            Line::from("  • Input history with ↑/↓"),
            Line::from("  • Scrollable message history"),
            Line::from("  • Real-time typing indicators"),
            Line::from("  • Tool usage and reasoning display"),
            Line::from("  • Configurable model parameters"),
        ];

        let paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title(" Help (Press ESC to close) ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);

        f.render_widget(paragraph, area);
    }

    pub fn render_settings(
        &self,
        f: &mut Frame,
        model: &Model,
        tunings: &ModelTunings,
        system_prompt: &str,
        file_modification_api: &crate::chat::state::FileModificationApi,
    ) {
        let area = centered_rect(60, 50, f.area());

        f.render_widget(Clear, area);

        let settings_text = vec![
            Line::from("⚙️  Current Settings"),
            Line::from(""),
            Line::from(format!("Model: {}", model.name())),
            Line::from(format!(
                "Max Tokens: {}",
                tunings
                    .max_tokens
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "Default".to_string())
            )),
            Line::from(format!(
                "Temperature: {}",
                tunings
                    .temperature
                    .map(|t| format!("{:.2}", t))
                    .unwrap_or_else(|| "Default".to_string())
            )),
            Line::from(format!(
                "Top-P: {}",
                tunings
                    .top_p
                    .map(|p| format!("{:.2}", p))
                    .unwrap_or_else(|| "Default".to_string())
            )),
            Line::from(format!(
                "Reasoning: {}",
                if let Some(budget) = tunings.reasoning_budget {
                    format!("Enabled ({} tokens)", budget)
                } else {
                    "Disabled".to_string()
                }
            )),
            Line::from(format!(
                "File Modification API: {:?}",
                file_modification_api
            )),
            Line::from(""),
            Line::from("System Prompt:"),
            Line::from(system_prompt),
        ];

        let paragraph = Paragraph::new(settings_text)
            .block(
                Block::default()
                    .title(" Settings (Press ESC to close) ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left);

        f.render_widget(paragraph, area);
    }

    pub fn render_splash(&self, f: &mut Frame) {
        let area = centered_rect(60, 70, f.area());

        f.render_widget(Clear, area);

        let mut splash_lines = vec![Line::from("")];

        // Add TYCODE ASCII art lines in yellow
        for line in TYCODE_ASCII.lines() {
            splash_lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Yellow),
            )));
        }

        splash_lines.push(Line::from(""));
        splash_lines.push(Line::from(Span::styled(
            "Press any key to start chatting...",
            Style::default().fg(Color::Green),
        )));

        let paragraph = Paragraph::new(splash_lines)
            .block(
                Block::default()
                    .title(" Welcome to TyCode ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }
}

// Helper function for centering rectangles
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
