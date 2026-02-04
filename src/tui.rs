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
use std::{io, time::Duration};
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
}

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
            status_message: "按 SPACE 选择/取消选择, ENTER 扫描, C 清理, Q 退出".to_string(),
            is_scanning: false,
            is_cleaning: false,
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(item) = self.cleanup_items.get_mut(self.selected_index) {
            item.enabled = !item.enabled;
            debug!("Toggled selection for item {}: {}", self.selected_index, item.name);
        }
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
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
                        if !app.is_scanning && !app.is_cleaning {
                            app.toggle_selection();
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if !app.is_scanning && !app.is_cleaning {
                            app.previous();
                            list_state.select(Some(app.selected_index));
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !app.is_scanning && !app.is_cleaning {
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
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        // Reset
                        *app = App::new();
                        list_state.select(Some(0));
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame<'_>, app: &mut App, list_state: &mut ListState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(5),  // Header
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Status bar
        ])
        .split(f.size());

    // Header
    let header = Block::default()
        .borders(Borders::ALL)
        .title(" Clean-RS 系统清理工具 ")
        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    
    let header_text = if app.state == AppState::CleaningDone {
        let total_size = app.get_total_size(true);
        let total_files = app.get_total_files(true);
        vec![
            Line::from(vec![
                Span::styled("清理完成! ", Style::default().fg(Color::Green)),
                Span::styled(format!("共释放 {:.2} MB ", total_size), 
                           Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} 个文件)", total_files), 
                           Style::default().fg(Color::Gray)),
            ])
        ]
    } else if app.state == AppState::ScanningDone {
        let total_size = app.get_total_size(false);
        let total_files = app.get_total_files(false);
        vec![
            Line::from(vec![
                Span::styled("扫描完成! ", Style::default().fg(Color::Cyan)),
                Span::styled(format!("可清理 {:.2} MB ", total_size), 
                           Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("({} 个文件)", total_files), 
                           Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("按 [C] 开始清理, [R] 重置", 
                           Style::default().fg(Color::Yellow)),
            ])
        ]
    } else if app.is_scanning {
        vec![
            Line::from(vec![
                Span::styled("正在扫描...", Style::default().fg(Color::Yellow)),
            ])
        ]
    } else if app.is_cleaning {
        vec![
            Line::from(vec![
                Span::styled("正在清理...", Style::default().fg(Color::Yellow)),
            ])
        ]
    } else {
        vec![
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
            let status_icon = if item.enabled { "[✓]" } else { "[ ]" };
            let style = if item.enabled {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Gray)
            };
            
            let result_info = if let (AppState::ScanningDone, Some(true)) = 
                (&app.state, app.scan_results.get(i).map(|r| r.is_some())) {
                let result = app.scan_results[i].as_ref().unwrap();
                if result.has_data {
                    format!(" - {:.2} MB, {} 文件", result.size_mb(), result.files)
                } else {
                    " - 无数据".to_string()
                }
            } else if let (AppState::CleaningDone, Some(true)) = 
                (&app.state, app.clean_results.get(i).map(|r| r.is_some())) {
                let result = app.clean_results[i].as_ref().unwrap();
                if result.has_data {
                    format!(" - ✓ 已清理 {:.2} MB", result.size_mb())
                } else {
                    " - 无数据".to_string()
                }
            } else {
                "".to_string()
            };
            
            let content = format!("{} {}{}{} {}", 
                status_icon,
                &item.name,
                if item.enabled { "" } else { " (禁用)" },
                result_info,
                if result_info.is_empty() { &item.description } else { "" }
            );
            
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" 清理项目 (SPACE 选择, ENTER 扫描) "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
    
    f.render_stateful_widget(list, chunks[1], list_state);

    // Status bar
    let status_bar = Block::default()
        .borders(Borders::ALL);
    
    let status_text = Paragraph::new(app.status_message.as_str())
        .block(status_bar)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(status_text, chunks[2]);

    // Progress indicator (if scanning or cleaning)
    if app.is_scanning || app.is_cleaning {
        let progress_block = Block::default()
            .title(if app.is_scanning { " 扫描进度 " } else { " 清理进度 " });
        
        let progress = Gauge::default()
            .block(progress_block)
            .gauge_style(Style::default().fg(Color::LightCyan).bg(Color::Black))
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