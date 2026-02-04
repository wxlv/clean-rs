use crate::cleanup_items::{CleanupItem, CleanupResult, get_all_cleanup_items};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, time::{Duration, Instant}};
use tracing::{debug, info};

/// Application state for the TUI
pub struct App {
    pub cleanup_items: Vec<CleanupItem>,
    pub scan_results: Vec<Option<CleanupResult>>,
    pub clean_results: Vec<Option<CleanupResult>>,
    pub selected_index: usize,
    pub state: AppState,
    pub status_message: String,
    pub is_scanning: bool,
    pub is_cleaning: bool,
    /// Track the last key event time to prevent auto-repeat issues
    pub last_key_event_time: Option<Instant>,
}

/// Cooldown duration between key events (150ms) to prevent auto-repeat
const KEY_COOLDOWN_MS: u64 = 150;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Initial,
    Scanning,
    ScanningDone,
    Cleaning,
    CleaningDone,
}

impl App {
    pub fn new() -> Self {
        let cleanup_items = get_all_cleanup_items();
        let scan_results = vec![None; cleanup_items.len()];
        let clean_results = vec![None; cleanup_items.len()];
        
        Self {
            cleanup_items,
            scan_results,
            clean_results,
            selected_index: 0,
            state: AppState::Initial,
            status_message: "SPACE 选择 | A 全选 | D 取消 | I 反选 | ENTER 扫描 | C 清理 | Q 退出".to_string(),
            is_scanning: false,
            is_cleaning: false,
            last_key_event_time: None,
        }
    }

    /// Check if a key event should be processed based on cooldown
    /// Returns true if the key event should be processed
    pub fn should_process_key(&mut self) -> bool {
        let now = Instant::now();
        match self.last_key_event_time {
            Some(last_time) => {
                if now.duration_since(last_time).as_millis() >= KEY_COOLDOWN_MS as u128 {
                    self.last_key_event_time = Some(now);
                    true
                } else {
                    false
                }
            }
            None => {
                self.last_key_event_time = Some(now);
                true
            }
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(item) = self.cleanup_items.get_mut(self.selected_index) {
            item.enabled = !item.enabled;
            debug!("Toggled selection for item {}: {}", self.selected_index, item.name);
        }
    }

    pub fn select_all(&mut self) {
        for item in &mut self.cleanup_items {
            item.enabled = true;
        }
        info!("Selected all items");
    }

    pub fn deselect_all(&mut self) {
        for item in &mut self.cleanup_items {
            item.enabled = false;
        }
        info!("Deselected all items");
    }

    pub fn invert_selection(&mut self) {
        for item in &mut self.cleanup_items {
            item.enabled = !item.enabled;
        }
        info!("Inverted selection");
    }

    pub fn next(&mut self) {
        if self.cleanup_items.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.cleanup_items.len();
    }

    pub fn previous(&mut self) {
        if self.cleanup_items.is_empty() {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.cleanup_items.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    pub async fn scan_all(&mut self) {
        self.state = AppState::Scanning;
        self.is_scanning = true;
        self.status_message = "正在扫描...".to_string();
        
        for (i, item) in self.cleanup_items.iter().enumerate() {
            if item.enabled {
                let result = item.scan();
                self.scan_results[i] = Some(result);
                debug!("Scanned item {}: {:?}", i, self.scan_results[i]);
            }
        }
        
        self.state = AppState::ScanningDone;
        self.is_scanning = false;
        self.status_message = "扫描完成! 按 C 执行清理, 或按 Q 退出".to_string();
        info!("Scanning complete");
    }

    pub async fn clean_selected(&mut self) {
        self.state = AppState::Cleaning;
        self.is_cleaning = true;
        self.status_message = "正在清理...".to_string();
        
        for (i, item) in self.cleanup_items.iter().enumerate() {
            if item.enabled {
                let result = item.clean();
                self.clean_results[i] = Some(result);
                debug!("Cleaned item {}: {:?}", i, self.clean_results[i]);
            }
        }
        
        self.state = AppState::CleaningDone;
        self.is_cleaning = false;
        let total_size: u64 = self.clean_results.iter()
            .filter_map(|r| r.as_ref())
            .map(|r| r.size_bytes)
            .sum();
        self.status_message = format!("清理完成! 共释放 {:.2} MB", total_size as f64 / (1024.0 * 1024.0));
        info!("Cleaning complete: {:.2} MB freed", total_size as f64 / (1024.0 * 1024.0));
    }

    pub fn get_total_size(&self, use_clean_results: bool) -> f64 {
        let results = if use_clean_results {
            &self.clean_results
        } else {
            &self.scan_results
        };
        
        results.iter()
            .filter_map(|r| r.as_ref())
            .map(|r| r.size_bytes as f64)
            .sum::<f64>() / (1024.0 * 1024.0)
    }

    pub fn get_total_files(&self, use_clean_results: bool) -> u64 {
        let results = if use_clean_results {
            &self.clean_results
        } else {
            &self.scan_results
        };
        
        results.iter()
            .filter_map(|r| r.as_ref())
            .map(|r| r.files)
            .sum()
    }
}

/// Run the TUI application
pub fn run_tui() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    
    const MIN_WIDTH: u16 = 80;
    const MIN_HEIGHT: u16 = 24;
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Set terminal size to reasonable minimum if it's too small
    let size = terminal.size()?;
    if size.width < MIN_WIDTH || size.height < MIN_HEIGHT {
        let new_width = size.width.max(MIN_WIDTH);
        let new_height = size.height.max(MIN_HEIGHT);
        terminal.resize(Rect {
            x: 0,
            y: 0,
            width: new_width,
            height: new_height,
        })?;
    }

    let mut app = App::new();
    let mut list_state = ListState::default();
    list_state.select(Some(0));

    let res = run_app(&mut terminal, &mut app, &mut list_state);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    list_state: &mut ListState,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app, list_state))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                    KeyCode::Char(' ') => {
                        // Apply cooldown for Space key to prevent rapid toggling
                        if !app.is_scanning && !app.is_cleaning && app.should_process_key() {
                            app.toggle_selection();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        // Apply cooldown for navigation to prevent skipping
                        if !app.is_scanning && !app.is_cleaning && app.should_process_key() {
                            app.previous();
                            list_state.select(Some(app.selected_index));
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        // Apply cooldown for navigation to prevent skipping
                        if !app.is_scanning && !app.is_cleaning && app.should_process_key() {
                            app.next();
                            list_state.select(Some(app.selected_index));
                        }
                    }
                    KeyCode::Enter => {
                        if app.state == AppState::Initial || app.state == AppState::ScanningDone {
                            tokio::runtime::Runtime::new()
                                .unwrap()
                                .block_on(app.scan_all());
                        }
                    }
                    KeyCode::Char('c') | KeyCode::Char('C') => {
                        if app.state == AppState::ScanningDone {
                            tokio::runtime::Runtime::new()
                                .unwrap()
                                .block_on(app.clean_selected());
                            
                            // After cleaning, reset to initial state
                            *app = App::new();
                            app.status_message = "清理完成！已重置到初始状态，可选择其他项目或按 Q 退出".to_string();
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        // Reset
                        *app = App::new();
                        list_state.select(Some(0));
                    }
                    // Batch selection shortcuts
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        if !app.is_scanning && !app.is_cleaning {
                            app.select_all();
                        }
                    }
                    KeyCode::Char('d') | KeyCode::Char('D') => {
                        if !app.is_scanning && !app.is_cleaning {
                            app.deselect_all();
                        }
                    }
                    KeyCode::Char('i') | KeyCode::Char('I') => {
                        if !app.is_scanning && !app.is_cleaning {
                            app.invert_selection();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame<'_>, app: &mut App, list_state: &mut ListState) {
    // Modern color scheme inspired by CCleaner/BleachBit
    let header_color = Color::Rgb(0, 120, 215);  // Windows blue
    let success_color = Color::Rgb(16, 185, 129); // Green
    let warning_color = Color::Rgb(245, 158, 11); // Amber/Yellow
    let accent_color = Color::Rgb(99, 102, 241);  // Indigo
    let bg_color = Color::Rgb(30, 41, 59);        // Dark slate
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(6),  // Header (increased)
            Constraint::Min(12),    // Main content
            Constraint::Length(4),  // Status bar (increased)
        ])
        .split(f.size());

    // Clear background with modern dark theme
    let bg_block = Block::default()
        .style(Style::default().bg(bg_color));
    f.render_widget(bg_block, f.size());

    // Header with modern styling
    let header = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(header_color))
        .title(" 🧹 Clean-RS 系统清理工具 v0.3 ")
        .title_style(Style::default().fg(header_color).add_modifier(Modifier::BOLD));
    
    let header_text = if app.state == AppState::CleaningDone {
        let total_size = app.get_total_size(true);
        let total_files = app.get_total_files(true);
        vec![
            Line::from(vec![
                Span::styled("✓ 清理完成! ", Style::default().fg(success_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("共释放 {:.2} MB ", total_size), 
                           Style::default().fg(warning_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} 个文件)", total_files), 
                           Style::default().fg(Color::Rgb(148, 163, 184))),
            ]),
            Line::from(vec![
                Span::styled("系统已优化，可以选择其他项目继续清理", 
                           Style::default().fg(Color::Rgb(148, 163, 184))),
            ])
        ]
    } else if app.state == AppState::ScanningDone {
        let total_size = app.get_total_size(false);
        let total_files = app.get_total_files(false);
        vec![
            Line::from(vec![
                Span::styled("✓ 扫描完成! ", Style::default().fg(accent_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("可清理 {:.2} MB ", total_size), 
                           Style::default().fg(warning_color).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} 个文件)", total_files), 
                           Style::default().fg(Color::Rgb(148, 163, 184))),
            ]),
            Line::from(vec![
                Span::styled("按 [C] 开始清理, [R] 重置, [Q] 退出", 
                           Style::default().fg(warning_color)),
            ])
        ]
    } else if app.is_scanning {
        vec![
            Line::from(vec![
                Span::styled("⏳ 正在扫描系统...", Style::default().fg(warning_color)),
            ]),
            Line::from(vec![
                Span::styled("请稍候...", Style::default().fg(Color::Rgb(148, 163, 184))),
            ])
        ]
    } else if app.is_cleaning {
        vec![
            Line::from(vec![
                Span::styled("🧹 正在清理垃圾文件...", Style::default().fg(warning_color)),
            ]),
            Line::from(vec![
                Span::styled("请稍候...", Style::default().fg(Color::Rgb(148, 163, 184))),
            ])
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("👋 欢迎使用 Clean-RS!", Style::default().fg(accent_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("选择要清理的项目，然后按 [ENTER] 扫描", 
                           Style::default().fg(Color::White)),
            ])
        ]
    };

    let header_paragraph = Paragraph::new(header_text)
        .block(header)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);
    f.render_widget(header_paragraph, chunks[0]);

    // Main content - list of cleanup items
    let items: Vec<ListItem> = app.cleanup_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let status_icon = if item.enabled { "✓" } else { "○" };
            let style = if item.enabled {
                Style::default()
                    .fg(Color::Rgb(34, 197, 94))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(148, 163, 184))
            };
            
            let result_info = if let (AppState::ScanningDone, Some(true)) = 
                (&app.state, app.scan_results.get(i).map(|r| r.is_some())) {
                let result = app.scan_results[i].as_ref().unwrap();
                if result.has_data {
                    format!(" → {:.2} MB, {} 文件", result.size_mb(), result.files)
                } else {
                    " → (无数据)".to_string()
                }
            } else if let (AppState::CleaningDone, Some(true)) = 
                (&app.state, app.clean_results.get(i).map(|r| r.is_some())) {
                let result = app.clean_results[i].as_ref().unwrap();
                if result.has_data {
                    format!(" → ✓ 已清理",)
                } else {
                    " → (无数据)".to_string()
                }
            } else {
                "".to_string()
            };
            
            let icon_style = if item.enabled {
                Style::default().fg(Color::Rgb(34, 197, 94))
            } else {
                Style::default().fg(Color::Rgb(148, 163, 184))
            };
            
            let content = Line::from(vec![
                Span::styled(format!("[{}] ", status_icon), icon_style),
                Span::styled(&item.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(result_info, Style::default().fg(warning_color)),
            ]);
            
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(header_color))
            .title(" 📋 清理项目列表 "))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(51, 65, 85))
                .add_modifier(Modifier::BOLD),
        );
    
    f.render_stateful_widget(list, chunks[1], list_state);

    // Status bar
    let status_bar = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(header_color));
    
    let status_line = Line::from(vec![
        Span::styled("💡 ", Style::default().fg(accent_color)),
        Span::styled(&app.status_message, Style::default().fg(Color::White)),
    ]);
    
    let status_text = Paragraph::new(status_line)
        .block(status_bar)
        .alignment(Alignment::Center)
        .style(Style::default().bg(Color::Rgb(30, 41, 59)));
    f.render_widget(status_text, chunks[2]);

    // Progress indicator (if scanning or cleaning)
    if app.is_scanning || app.is_cleaning {
        let progress_title = if app.is_scanning { " ⏳ 扫描中 " } else { " 🧹 清理中 " };
        let progress_block = Block::default()
            .title(progress_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent_color));
        
        // Animated gradient color for progress
        let progress_color = if app.is_scanning {
            Color::Rgb(99, 102, 241)  // Indigo
        } else {
            Color::Rgb(34, 197, 94)   // Green
        };
        
        let progress = Gauge::default()
            .block(progress_block)
            .gauge_style(Style::default().fg(progress_color).bg(Color::Rgb(30, 41, 59)))
            .ratio(1.0); // Full progress
        
        let popup_area = Rect {
            x: f.size().width / 4,
            y: f.size().height / 2 - 2,
            width: f.size().width / 2,
            height: 5,
        };
        f.render_widget(Clear, popup_area);
        f.render_widget(progress, popup_area);
    }
}