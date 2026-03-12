//! Dashboard UI for the basis monitor.

use std::io;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use anyhow::{Result, Context};
use tracing::{info, error};

use crate::config::Config;
use crate::queue::manager::{QueueManager, QueueStats};

/// Dashboard UI for displaying system status and metrics.
pub struct Dashboard {
    /// Application configuration
    config: Config,
    /// Shared stats (when using new_with_shared_stats)
    shared_stats: Option<Arc<Mutex<QueueStats>>>,
    /// Queue manager for statistics (when using new)
    queue_manager: Option<QueueManager>,
    /// Shutdown signal (set when user presses 'q')
    shutdown: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// Terminal instance
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    /// Running flag
    running: bool,
    /// Last update time
    last_update: Instant,
    /// Performance data storage
    performance_history: Vec<f64>,
}

impl Dashboard {
    /// Create a new dashboard with shared stats (for use with async processor).
    pub fn new_with_shared_stats(
        config: Config,
        shared_stats: Arc<Mutex<QueueStats>>,
        shutdown: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Self> {
        enable_raw_mode().context("Failed to enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to enter alternate screen")?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).context("Failed to create terminal")?;

        Ok(Self {
            config,
            shared_stats: Some(shared_stats),
            queue_manager: None,
            shutdown: Some(shutdown),
            terminal,
            running: true,
            last_update: Instant::now(),
            performance_history: Vec::new(),
        })
    }

    /// Create a new dashboard with exclusive QueueManager.
    pub fn new(config: Config, queue_manager: QueueManager) -> Result<Self> {
        // Setup terminal
        enable_raw_mode().context("Failed to enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to enter alternate screen")?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).context("Failed to create terminal")?;

        Ok(Self {
            config,
            shared_stats: None,
            queue_manager: Some(queue_manager),
            shutdown: None,
            terminal,
            running: true,
            last_update: Instant::now(),
            performance_history: Vec::new(),
        })
    }

    /// Run the dashboard main loop.
    pub fn run(&mut self) -> Result<()> {
        info!("Starting dashboard UI");

        while self.running {
            // Check for user input
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            self.running = false;
                            if let Some(ref s) = self.shutdown {
                                s.store(true, Ordering::SeqCst);
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Update UI frequently for responsive display (max 4 FPS)
            let refresh_secs = self.config.ui_refresh_interval.max(1);
            if self.last_update.elapsed() >= Duration::from_secs(refresh_secs) {
                self.update()?;
                self.last_update = Instant::now();
            }
        }

        self.cleanup()?;
        info!("Dashboard stopped");
        Ok(())
    }

    /// Update the dashboard display.
    fn update(&mut self) -> Result<()> {
        // Get current statistics from shared stats or queue manager
        let stats = if let Some(ref shared) = self.shared_stats {
            shared.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?.clone()
        } else if let Some(ref qm) = self.queue_manager {
            qm.get_stats()
        } else {
            return Ok(());
        };

        // Draw UI - clone to avoid borrow conflict in closure
        let config = self.config.clone();
        self.terminal.draw(|frame| {
            Self::draw_ui_impl(frame, &stats, &config);
        })?;

        Ok(())
    }

    /// Draw the entire UI (static to avoid borrow in terminal.draw closure).
    fn draw_ui_impl(frame: &mut Frame, stats: &QueueStats, config: &Config) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),  // Header
                Constraint::Min(10),    // Main content
                Constraint::Length(3),  // Footer
            ])
            .split(frame.size());

        Self::draw_header_impl(frame, chunks[0], stats);
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(chunks[1]);
        Self::draw_left_panel_impl(frame, main_chunks[0], stats);
        Self::draw_right_panel_impl(frame, main_chunks[1], stats);
        Self::draw_footer_impl(frame, chunks[2], config);
    }

    fn draw_header_impl(frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let total_secs = stats.run_time as u64;
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        let uptime = format!("{:02}:{:02}:{:02}", hours, mins, secs);
        let header = Paragraph::new(format!(
            "🚀 Binance Basis Monitor - Rust Implementation     ⏱️ Uptime: {}",
            uptime
        ))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Cyan))
                    .title("System Status"),
            );
        frame.render_widget(header, area);
    }

    fn draw_left_panel_impl(frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(5)])
            .split(area);
        Self::draw_metrics_panel_impl(frame, left_chunks[0], stats);
        Self::draw_symbols_panel_impl(frame, left_chunks[1], stats);
    }

    fn draw_metrics_panel_impl(frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let rows = vec![
            Row::new(vec![Cell::from("Queue Size"), Cell::from(format!("{}/{}", stats.queue_size, stats.max_size))]),
            Row::new(vec![Cell::from("Receive Rate"), Cell::from(format!("{:.0} msg/s", stats.receive_rate))]),
            Row::new(vec![Cell::from("Process Rate"), Cell::from(format!("{:.0} msg/s", stats.process_rate))]),
            Row::new(vec![Cell::from("Backlog"), Cell::from(format!("{}", stats.backlog))]),
            Row::new(vec![Cell::from("P99 Latency"), Cell::from(format!("{:.1} ms", stats.latency_p99 * 1000.0))]),
        ];
        let widths = [Constraint::Percentage(50), Constraint::Percentage(50)];
        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Blue))
                    .title("Performance Metrics"),
            );
        frame.render_widget(table, area);
    }

    fn draw_symbols_panel_impl(frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let mut rows = Vec::new();
        let mut symbol_stats: Vec<(&String, &crate::queue::manager::SymbolStats)> =
            stats.symbol_stats.iter().collect();
        symbol_stats.sort_by(|a, b| b.1.received.cmp(&a.1.received));
        for (symbol, stats) in symbol_stats.iter().take(10) {
            let rate = if stats.received > 0 {
                stats.processed as f64 / stats.received as f64
            } else {
                0.0
            };
            rows.push(Row::new(vec![
                Cell::from(symbol.as_str()),
                Cell::from(format!("{}", stats.received)),
                Cell::from(format!("{}", stats.processed)),
                Cell::from(format!("{:.1}%", rate * 100.0)),
            ]));
        }
        if rows.is_empty() {
            rows.push(Row::new(vec![
                Cell::from("No symbol data yet"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ]));
        }
        let widths = [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ];
        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Green))
                    .title(format!("Symbols ({} total)", stats.symbol_stats.len())),
            );
        frame.render_widget(table, area);
    }

    fn draw_right_panel_impl(frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(6)])
            .split(area);
        Self::draw_performance_trends_impl(frame, right_chunks[0], stats);
        Self::draw_alerts_impl(frame, right_chunks[1], stats);
    }

    fn draw_performance_trends_impl(frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let latency_ms = stats.latency_p99 * 1000.0;
        let e2e_ms = stats.latency_e2e_p99 * 1000.0;
        let text = format!(
            "📊 Performance Trends\n\n\
             📥 Receive Rate: {:.0} msg/s\n\
             ⚙️ Process Rate: {:.0} msg/s\n\
             ⏱️ P99 Queue: {:.1} ms\n\
             ⏱️ P99 E2E: {:.1} ms\n\n\
             Run Time: {:.0}s",
            stats.receive_rate,
            stats.process_rate,
            latency_ms,
            e2e_ms,
            stats.run_time
        );
        let para = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Magenta))
                    .title("Performance Trends"),
            );
        frame.render_widget(para, area);
    }

    fn draw_alerts_impl(frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let mut alerts: Vec<String> = Vec::new();
        let queue_usage = if stats.max_size > 0 {
            stats.queue_size as f64 / stats.max_size as f64
        } else {
            0.0
        };
        if queue_usage > 0.9 {
            alerts.push("❌ Queue critical".to_string());
        } else if queue_usage > 0.7 {
            alerts.push("⚠️ Queue high".to_string());
        }
        if stats.receive_rate > 0.0 {
            let ratio = stats.process_rate / stats.receive_rate;
            if ratio < 0.8 {
                alerts.push("⚠️ Processing lag".to_string());
            } else if ratio < 0.95 {
                alerts.push("ℹ️ Processing slow".to_string());
            }
        }
        let latency_ms = stats.latency_p99 * 1000.0;
        if latency_ms > 500.0 {
            alerts.push(format!("❌ High latency: {:.0}ms", latency_ms));
        } else if latency_ms > 100.0 {
            alerts.push(format!("⚠️ Elevated latency: {:.0}ms", latency_ms));
        }
        if stats.dropped_count > 0 {
            alerts.push(format!("⚠️ Messages dropped: {}", stats.dropped_count));
        }
        if alerts.is_empty() {
            alerts.push("✅ All systems normal".to_string());
        }
        let text = alerts.join("\n");
        let border_color = if text.contains("❌") {
            Color::Red
        } else if text.contains("⚠️") {
            Color::Yellow
        } else {
            Color::Green
        };
        let para = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(border_color))
                    .title("Alerts"),
            );
        frame.render_widget(para, area);
    }

    fn draw_footer_impl(frame: &mut Frame, area: Rect, config: &Config) {
        let test_mode = if config.performance_test_mode { " | 🔄 TEST MODE" } else { "" };
        let text = format!(
            "Monitoring {} symbols{} | Window: {}s | EMA: {}",
            config.symbols.len(),
            test_mode,
            config.window_interval,
            config.ema_window
        );
        let para = Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(para, area);
    }

    /// Draw the entire UI (legacy - uses self).
    fn draw_ui(&self, frame: &mut Frame, stats: &QueueStats) {
        Self::draw_ui_impl(frame, stats, &self.config);
    }

    /// Draw header panel.
    fn draw_header(&self, frame: &mut Frame, area: Rect) {
        let header = Paragraph::new("🚀 Binance Basis Monitor - Rust Implementation")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Cyan))
                    .title("System Status"),
            );

        frame.render_widget(header, area);
    }

    /// Draw left panel (metrics and symbols).
    fn draw_left_panel(&self, frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10),  // Metrics
                Constraint::Min(5),      // Symbols
            ])
            .split(area);

        self.draw_metrics_panel(frame, left_chunks[0], stats);
        self.draw_symbols_panel(frame, left_chunks[1], stats);
    }

    /// Draw metrics panel.
    fn draw_metrics_panel(&self, frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let rows = vec![
            Row::new(vec![Cell::from("Queue Size"), Cell::from(format!("{}/{}", stats.queue_size, stats.max_size))]),
            Row::new(vec![Cell::from("Receive Rate"), Cell::from(format!("{:.0} msg/s", stats.receive_rate))]),
            Row::new(vec![Cell::from("Process Rate"), Cell::from(format!("{:.0} msg/s", stats.process_rate))]),
            Row::new(vec![Cell::from("Backlog"), Cell::from(format!("{}", stats.backlog))]),
            Row::new(vec![Cell::from("P99 Latency (Queue)"), Cell::from(format!("{:.1} ms", stats.latency_p99 * 1000.0))]),
            Row::new(vec![Cell::from("P99 E2E Latency"), Cell::from(format!("{:.1} ms", stats.latency_e2e_p99 * 1000.0))]),
        ];

        let widths = [Constraint::Percentage(50), Constraint::Percentage(50)];
        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Blue))
                    .title("Performance Metrics"),
            );

        frame.render_widget(table, area);
    }

    /// Draw symbols panel.
    fn draw_symbols_panel(&self, frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let mut rows = Vec::new();

        // Add top symbols by activity
        let mut symbol_stats: Vec<(&String, &crate::queue::manager::SymbolStats)> =
            stats.symbol_stats.iter().collect();
        symbol_stats.sort_by(|a, b| b.1.received.cmp(&a.1.received));

        for (symbol, stats) in symbol_stats.iter().take(10) {
            let rate = if stats.received > 0 {
                stats.processed as f64 / stats.received as f64
            } else {
                0.0
            };

            rows.push(Row::new(vec![
                Cell::from(symbol.as_str()),
                Cell::from(format!("{}", stats.received)),
                Cell::from(format!("{}", stats.processed)),
                Cell::from(format!("{:.1}%", rate * 100.0)),
            ]));
        }

        if rows.is_empty() {
            rows.push(Row::new(vec![
                Cell::from("No symbol data yet"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ]));
        }

        let widths = [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ];
        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Green))
                    .title(format!("Symbols ({} total)", stats.symbol_stats.len())),
            );

        frame.render_widget(table, area);
    }

    /// Draw right panel (performance and alerts).
    fn draw_right_panel(&self, frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),      // Performance
                Constraint::Length(6),   // Alerts
            ])
            .split(area);

        self.draw_performance_panel(frame, right_chunks[0], stats);
        self.draw_alerts_panel(frame, right_chunks[1], stats);
    }

    /// Draw performance panel.
    fn draw_performance_panel(&self, frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let text = format!(
            "📊 Performance Trends\n\n\
             📥 Receive Rate: {:.0} msg/s\n\
             ⚙️ Process Rate: {:.0} msg/s\n\
             ⏱️ P99 Queue: {:.1} ms\n\
             ⏱️ P99 E2E: {:.1} ms\n\n\
             Run Time: {:.0} s",
            stats.receive_rate,
            stats.process_rate,
            stats.latency_p99 * 1000.0,
            stats.latency_e2e_p99 * 1000.0,
            stats.run_time
        );

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Magenta))
                    .title("Performance Trends"),
            );

        frame.render_widget(paragraph, area);
    }

    /// Draw alerts panel.
    fn draw_alerts_panel(&self, frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let mut alerts = Vec::new();

        // Check for alerts
        let queue_usage = stats.queue_size as f64 / stats.max_size as f64;
        if queue_usage > 0.9 {
            alerts.push("❌ Queue critical");
        } else if queue_usage > 0.7 {
            alerts.push("⚠️ Queue high");
        }

        if stats.receive_rate > 0.0 {
            let processing_ratio = stats.process_rate / stats.receive_rate;
            if processing_ratio < 0.8 {
                alerts.push("⚠️ Processing lag");
            } else if processing_ratio < 0.95 {
                alerts.push("ℹ️ Processing slow");
            }
        }

        if stats.latency_p99 * 1000.0 > 500.0 {
            alerts.push("❌ High latency");
        } else if stats.latency_p99 * 1000.0 > 100.0 {
            alerts.push("⚠️ Elevated latency");
        }

        if stats.dropped_count > 0 {
            alerts.push("⚠️ Messages dropped");
        }

        if alerts.is_empty() {
            alerts.push("✅ All systems normal");
        }

        let text = alerts.join("\n");
        let border_color = if alerts.iter().any(|a| a.contains("❌")) {
            Color::Red
        } else if alerts.iter().any(|a| a.contains("⚠️")) {
            Color::Yellow
        } else {
            Color::Green
        };

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(border_color))
                    .title("Alerts"),
            );

        frame.render_widget(paragraph, area);
    }

    /// Draw footer panel.
    fn draw_footer(&self, frame: &mut Frame, area: Rect, stats: &QueueStats) {
        let test_mode = if self.config.performance_test_mode {
            "🔄 TEST MODE"
        } else {
            ""
        };

        let text = format!(
            "Monitoring {} symbols {} | Window: {}s | EMA: {}",
            self.config.symbols.len(),
            test_mode,
            self.config.window_interval,
            self.config.ema_window
        );

        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(paragraph, area);
    }

    /// Clean up terminal settings.
    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .context("Failed to leave alternate screen")?;
        self.terminal.show_cursor().context("Failed to show cursor")?;
        Ok(())
    }
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        // Ensure terminal is cleaned up
        let _ = self.cleanup();
    }
}