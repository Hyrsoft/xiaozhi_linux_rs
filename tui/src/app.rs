use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc;

use crate::face::{FaceAnimator, FaceState};

/// Commands that the core controller can send to the TUI.
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Update the face animation state.
    SetState(TuiState),
    /// Set the subtitle text (TTS text displayed below the face).
    SetSubtitle(String),
    /// Shut down the TUI gracefully.
    Quit,
}

/// Mapped system states for the TUI (avoids coupling to SystemState directly).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiState {
    Idle,
    Listening,
    Speaking,
    Thinking,
    NetworkError,
}

impl From<TuiState> for FaceState {
    fn from(s: TuiState) -> Self {
        match s {
            TuiState::Idle => FaceState::Idle,
            TuiState::Listening => FaceState::Listening,
            TuiState::Speaking => FaceState::Speaking,
            TuiState::Thinking => FaceState::Thinking,
            TuiState::NetworkError => FaceState::Idle,
        }
    }
}

/// The main TUI application.
pub struct TuiApp {
    rx: mpsc::Receiver<TuiCommand>,
    face: FaceAnimator,
    subtitle: String,
    current_state: TuiState,
}

impl TuiApp {
    /// Create a new TUI application.
    /// Returns `(TuiApp, Sender)` — the sender is used by the controller to push commands.
    pub fn new() -> (Self, mpsc::Sender<TuiCommand>) {
        let (tx, rx) = mpsc::channel(256);
        let app = Self {
            rx,
            face: FaceAnimator::new(),
            subtitle: String::new(),
            current_state: TuiState::Idle,
        };
        (app, tx)
    }

    /// Run the TUI event loop. This takes over the terminal.
    pub async fn run(mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        io::stdout().execute(EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let tick_rate = Duration::from_millis(67); // ~15 FPS

        loop {
            // Process all pending commands
            loop {
                match self.rx.try_recv() {
                    Ok(cmd) => match cmd {
                        TuiCommand::SetState(state) => {
                            self.current_state = state;
                            self.face.set_state(state.into());
                        }
                        TuiCommand::SetSubtitle(text) => {
                            self.subtitle = text;
                        }
                        TuiCommand::Quit => {
                            Self::restore_terminal(&mut terminal)?;
                            return Ok(());
                        }
                    },
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        Self::restore_terminal(&mut terminal)?;
                        return Ok(());
                    }
                }
            }

            // Handle terminal input events (non-blocking)
            if event::poll(Duration::from_millis(0))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => {
                                Self::restore_terminal(&mut terminal)?;
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Advance animation
            self.face.tick();

            // Render
            self.draw(&mut terminal)?;

            // Wait for next tick
            tokio::time::sleep(tick_rate).await;
        }
    }

    /// Draw the TUI layout: face animation + subtitle only.
    fn draw(&self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
        terminal.draw(|frame| {
            let size = frame.area();

            // Full area for face + subtitle
            let face_block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" 小智 — {} ", self.state_label()))
                .style(Style::default().fg(Color::Rgb(100, 140, 255)));

            let inner = face_block.inner(size);
            frame.render_widget(face_block, size);

            // Split: face area + subtitle (2 lines at bottom)
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(2),
                ])
                .split(inner);

            let face_area = chunks[0];
            let subtitle_area = chunks[1];

            // Face widget (self-centers within the area)
            frame.render_widget(self.face.widget(), face_area);

            // Subtitle
            let subtitle_text = if self.subtitle.is_empty() {
                vec![Line::from(Span::styled(
                    "···",
                    Style::default().fg(Color::DarkGray),
                ))]
            } else {
                vec![Line::from(Span::styled(
                    self.subtitle.clone(),
                    Style::default()
                        .fg(Color::Rgb(100, 200, 255))
                        .add_modifier(Modifier::BOLD),
                ))]
            };
            let subtitle_widget = Paragraph::new(subtitle_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            frame.render_widget(subtitle_widget, subtitle_area);
        })?;

        Ok(())
    }

    fn state_label(&self) -> &'static str {
        match self.current_state {
            TuiState::Idle => "待机",
            TuiState::Listening => "聆听中",
            TuiState::Speaking => "说话中",
            TuiState::Thinking => "思考中",
            TuiState::NetworkError => "网络错误",
        }
    }

    fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
        disable_raw_mode()?;
        io::stdout().execute(LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    }
}
