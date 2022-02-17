use std::io;
use crossbeam::channel::{self, Receiver};
use crossbeam::select;
use tui::{
    Terminal, backend::CrosstermBackend,
    backend::Backend,
    Frame
};
use crossterm::{
    ExecutableCommand,
    terminal::{enable_raw_mode, disable_raw_mode},
    event::{self, EnableMouseCapture, DisableMouseCapture, Event, KeyEvent, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::widgets::{Widget, Block, Borders, Paragraph, Wrap};
use tui::text::{Span, Spans};
use tui::style::{Style, Color, Modifier};
use tui::layout::{Alignment, Layout, Constraint, Direction};

struct Log {
    level: log::Level,
    msg: String,
}

struct NoticePanel {
}

struct LogPanel {
    logs: Vec<Log>,
}

struct TypingPanel {
    contents: String,
}

pub struct App {
    typing: TypingPanel,
    log: LogPanel,
    notice: NoticePanel
}

impl App {
    fn new() -> Self {
        Self {
            log: LogPanel {
                logs: Vec::new()
            },
            notice: NoticePanel {

            },
            typing: TypingPanel {
                contents: "sdfsd".into()
            }
        }
    }

    fn draw<B: Backend>(&mut self, f: &mut Frame<B>) {
        // let size = f.size();
        // let chunks = Layout::default()
        //     .direction(Direction::Vertical)
        //     .constraints([
        //         Constraint::Min(5),
        //         Constraint::Min(5),
        //         Constraint::Length(3),
        //     ])
        //     .split(size);


        let block = Block::default()
            .borders(Borders::ALL);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(block.inner(f.size()));

        f.render_widget(block, f.size());

        let s = "Veeeeeeeeeeeeeeeery    loooooooooooooooooong   striiiiiiiiiiiiiiiiiiiiiiiiiing.   ";
        let long_line = s.repeat(5);

        let text = vec![
            Spans::from("This is a line "),
            Spans::from(Span::styled(
                    "This is a line   ",
                    Style::default().fg(Color::Red),
            )),
            Spans::from(Span::styled(
                    "This is a line",
                    Style::default().bg(Color::Blue),
            )),
            Spans::from(Span::styled(
                    "This is a longer line",
                    Style::default().add_modifier(Modifier::CROSSED_OUT),
            )),
            Spans::from(Span::styled(&long_line, Style::default().bg(Color::Green))),
            Spans::from(Span::styled(
                    "This is a line",
                    Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::ITALIC),
            )),
            ];
        let paragh = Paragraph::new(text)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .block(Block::default().title("Typer").borders(Borders::NONE))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(paragh, chunks[0]);
    }
}

pub fn run(rx_logs: Receiver<String>) -> io::Result<()> {
    let mut app = App::new();

    // set up key reader
    let (tx_tevent, rx_tevent) = channel::bounded(1024);
    let _h = std::thread::Builder::new()
        .name("term_events".to_owned())
        .spawn(move || {
            while let Ok(event) = crossterm::event::read() {
                tx_tevent.send(event).ok();
            }
        });

    // setup
    enable_raw_mode()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // terminal.backend_mut().execute(EnterAlternateScreen)?;
    terminal.backend_mut().execute(EnableMouseCapture)?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    // set up timer: 60 fps
    let rx_timer = channel::tick(std::time::Duration::from_millis(15));

    loop {
        select! {
            recv(rx_timer) -> _ => {
                terminal.draw(|f| app.draw(f))?;
            },
            recv(rx_tevent) -> eterm => {
                if let Ok(eterm) = eterm {
                    match eterm {
                        Event::Key(KeyEvent { code: KeyCode::Char('q'), .. }) => {
                            break;
                        },
                        _ => {  }
                    }
                }
            },
            recv(rx_logs) -> _ => {
            },
        }
    }

    // restore
    // terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.backend_mut().execute(DisableMouseCapture)?;
    terminal.show_cursor()?;
    disable_raw_mode()?;

    Ok(())
}
