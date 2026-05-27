use crate::pipeline::{AppEvent, PipelineHandle};
use crate::report::write_report;
use crate::session::Session;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Tabs};
use ratatui::{Frame, Terminal};
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Stream,
    Stats,
    Alerts,
}

impl Tab {
    fn titles() -> Vec<&'static str> {
        vec!["Stream", "Stats", "Alerts"]
    }

    fn index(self) -> usize {
        match self {
            Tab::Stream => 0,
            Tab::Stats => 1,
            Tab::Alerts => 2,
        }
    }
}

struct UiState {
    tab: Tab,
    scroll: usize,
    event_lines: Vec<String>,
    status_lines: Vec<String>,
    error_lines: Vec<String>,
    started: Instant,
}

impl UiState {
    fn new() -> Self {
        Self {
            tab: Tab::Stream,
            scroll: 0,
            event_lines: Vec::new(),
            status_lines: Vec::new(),
            error_lines: Vec::new(),
            started: Instant::now(),
        }
    }

    fn push_event(&mut self, line: String) {
        self.event_lines.push(line);
        truncate_front(&mut self.event_lines, 1000);
    }

    fn push_status(&mut self, line: String) {
        self.status_lines.push(line);
        truncate_front(&mut self.status_lines, 200);
    }

    fn push_error(&mut self, line: String) {
        self.error_lines.push(line);
        truncate_front(&mut self.error_lines, 200);
    }
}

pub async fn run_tui(pipeline: PipelineHandle, session: &mut Session) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_loop(&mut terminal, pipeline, session).await;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    pipeline: PipelineHandle,
    session: &mut Session,
) -> Result<()> {
    let mut state = UiState::new();
    let mut events = pipeline.channels.subscribe();
    let mut last_draw = Instant::now();

    loop {
        drain_events(&mut events, &mut state);
        if event::poll(Duration::from_millis(25))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('1') => state.tab = Tab::Stream,
                        KeyCode::Char('2') => state.tab = Tab::Stats,
                        KeyCode::Char('3') => state.tab = Tab::Alerts,
                        KeyCode::Char('r') => {
                            if let Some(path) = session.report_path.as_ref() {
                                match write_report(path, &pipeline.snapshot()) {
                                    Ok(_) => state.push_status(format!(
                                        "report written to {}",
                                        path.display()
                                    )),
                                    Err(error) => {
                                        state.push_error(format!("report failed: {error:#}"))
                                    }
                                }
                            }
                        }
                        KeyCode::Down => state.scroll = state.scroll.saturating_add(1),
                        KeyCode::Up => state.scroll = state.scroll.saturating_sub(1),
                        _ => {}
                    }
                }
            }
        }
        if last_draw.elapsed() >= Duration::from_millis(100) {
            terminal.draw(|frame| draw(frame, &state, &pipeline, session))?;
            last_draw = Instant::now();
        }
    }

    let final_stats = pipeline.stop().await;
    let snapshot = final_stats.snapshot();
    if let Some(path) = session.report_path.as_ref() {
        let _ = write_report(path, &snapshot);
    }
    session.set_stats(snapshot);
    Ok(())
}

fn drain_events(events: &mut tokio::sync::broadcast::Receiver<AppEvent>, state: &mut UiState) {
    loop {
        match events.try_recv() {
            Ok(AppEvent::Entry(entry)) => state.push_event(entry.compact()),
            Ok(AppEvent::Alert(alert)) => state.push_event(format!("ALERT {}", alert.compact())),
            Ok(AppEvent::Status(status)) => state.push_status(status),
            Ok(AppEvent::Error(error)) => state.push_error(error),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                state.push_error(format!("ui skipped {skipped} events"));
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
        }
    }
}

fn draw(frame: &mut Frame<'_>, state: &UiState, pipeline: &PipelineHandle, session: &Session) {
    let snapshot = pipeline.snapshot();
    let area = frame.size();
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(5),
        ])
        .split(area);
    let header = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(32), Constraint::Min(20)])
        .split(outer[0]);
    frame.render_widget(
        Paragraph::new(format!("logstream | {}", session.name))
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan)),
        header[0],
    );
    frame.render_widget(
        Tabs::new(Tab::titles())
            .select(state.tab.index())
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        header[1],
    );

    match state.tab {
        Tab::Stream => {
            let items = state
                .event_lines
                .iter()
                .rev()
                .skip(state.scroll)
                .take(outer[1].height.saturating_sub(2) as usize)
                .map(|line| ListItem::new(color_log_line(line)))
                .collect::<Vec<_>>();
            frame.render_widget(
                List::new(items).block(Block::default().title("Logs").borders(Borders::ALL)),
                outer[1],
            );
        }
        Tab::Stats => {
            let rows = vec![
                Row::new(vec![
                    "Lines".to_string(),
                    snapshot.total_lines_seen.to_string(),
                ]),
                Row::new(vec![
                    "Entries".to_string(),
                    snapshot.total_entries_kept.to_string(),
                ]),
                Row::new(vec![
                    "Parse errors".to_string(),
                    snapshot.parse_errors.to_string(),
                ]),
                Row::new(vec![
                    "Ignored".to_string(),
                    snapshot.ignored_entries.to_string(),
                ]),
            ];
            frame.render_widget(
                Table::new(rows, [Constraint::Length(18), Constraint::Min(10)])
                    .block(Block::default().title("Statistics").borders(Borders::ALL)),
                outer[1],
            );
        }
        Tab::Alerts => {
            let items = snapshot
                .alerts
                .iter()
                .rev()
                .skip(state.scroll)
                .take(outer[1].height.saturating_sub(2) as usize)
                .map(|alert| {
                    ListItem::new(Line::from(vec![Span::styled(
                        alert.compact(),
                        Style::default().fg(Color::Red),
                    )]))
                })
                .collect::<Vec<_>>();
            frame.render_widget(
                List::new(items).block(Block::default().title("Alerts").borders(Borders::ALL)),
                outer[1],
            );
        }
    }

    let footer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[2]);
    let status = state
        .status_lines
        .iter()
        .rev()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(format!(
            "{status}\nuptime={}s | q quit | r report | 1/2/3 tabs",
            state.started.elapsed().as_secs()
        ))
        .block(Block::default().title("Status").borders(Borders::ALL)),
        footer[0],
    );
    let errors = state
        .error_lines
        .iter()
        .rev()
        .take(4)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(errors)
            .style(Style::default().fg(Color::LightRed))
            .block(Block::default().title("Errors").borders(Borders::ALL)),
        footer[1],
    );
}

fn color_log_line(line: &str) -> Line<'_> {
    let color = if line.contains("ERROR") || line.contains("ALERT") {
        Color::Red
    } else if line.contains("WARN") {
        Color::Yellow
    } else if line.contains("INFO") {
        Color::Green
    } else {
        Color::White
    };
    Line::from(vec![Span::styled(
        line.to_string(),
        Style::default().fg(color),
    )])
}

fn truncate_front<T>(values: &mut Vec<T>, max_len: usize) {
    if values.len() > max_len {
        values.drain(0..values.len() - max_len);
    }
}
