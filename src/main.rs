use std::io::{self, Write, Read, BufRead, BufReader};
use std::path::Path;
use std::time::Instant;
use crossterm::{cursor, execute, queue};
use crossterm::style::{self, Color, Print, Stylize};
use crossterm::event::{Event, KeyEvent, KeyCode, KeyModifiers};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use crossbeam::{channel, select};

const MAX_WIDTH: usize = 1024;

pub struct Ipsum<'s> {
    mark: lipsum::MarkovChain<'s>,
    last: Vec<u8>,
}

impl<'s> Read for Ipsum<'s> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut pos = 0;
        while pos < buf.len() {
            if self.last.len() > 0 {
                let mut r: &[u8] = &self.last;
                let n = r.read(&mut buf[pos..])?;
                pos += n;
                self.last.clear();
                self.last.push(' ' as u8);
            } else {
                self.last.clear();
            }

            let next = self.mark.generate(1);
            let bytes = next.into_bytes();
            assert!(bytes.len() > 0);
            self.last.extend_from_slice(&bytes[..bytes.len()-1]);
        }

        Ok(buf.len())
    }
}

pub enum TextGen<'s> {
    File(std::fs::File),
    Ipsum(Ipsum<'s>),
    IpsumCustom(Ipsum<'s>),
}

impl<'s> Read for TextGen<'s> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(inner)        => inner.read(buf),
            Self::Ipsum(inner)       => inner.read(buf),
            Self::IpsumCustom(inner) => inner.read(buf),
        }
    }
}

pub struct App<R> {
    width: usize,
    start: Instant,

    right: usize,
    wrong: usize,
    words: usize,
    total: usize,

    buf: Vec<u8>,
    acc: Vec<bool>,

    gen: R,
}

impl<R: Read> App<R> {
    fn new(width: usize, total: usize, gen: R) -> Self {
        assert!(width <= MAX_WIDTH);
        let mut slf = Self {
            width,
            start: Instant::now(),

            right: 0,
            wrong: 0,
            words: 0,
            total,

            buf: vec![],
            acc: vec![],

            gen
        };
        slf.newline().expect("Fail to load content at startup");
        slf
    }

    fn print_help() -> io::Result<()> {
        print!(r#"{}
{}

{}

"#,
        "$ typer".italic(),
        "$ version 0.1.0".italic(),
        r#"- Ctrl-C: exit
- '␣': space
- '·': unknown char
- '§': newline"#);
        io::stdout().flush()?;

        Ok(())
    }

    fn newline(&mut self) -> io::Result<()> {
        if self.acc.len() == self.buf.len() {
            self.buf.clear();
            self.buf.resize((self.width).min(self.total-self.right-self.wrong), 0);
            let n = self.gen.read(&mut self.buf)?;
            if n == 0 { return Err(io::Error::from(io::ErrorKind::UnexpectedEof)); }
            self.buf.resize(n, 0);
            self.acc.clear();
        }
        Ok(())
    }

    fn draw<W: Write>(&self, scr: &mut W, width: u16) -> io::Result<()> {
        if (width as usize) < self.width + 2 { return Ok(()); }

        queue!(
            scr,
            cursor::SavePosition,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::FromCursorDown),

            style::SetForegroundColor(Color::Green),
            Print('['),
            style::ResetColor,
        )?;

        let mut pos = 0;
        for &c in self.buf.iter() {
            let c = match c as char {
                ' '               => { '␣' },
                '\n'              => { '·' },
                c if c.is_ascii() => { c   },
                _                 => { '·' },
            };

            if pos < self.acc.len() {
                let right = self.acc[pos];
                let color = if right { Color::White } else { Color::Red };
                queue!(
                    scr,
                    style::SetForegroundColor(color),
                    Print(c),
                    style::ResetColor,
                )?;
            } else if pos == self.acc.len() {
                queue!(
                    scr,
                    style::SetAttribute(style::Attribute::Underlined),
                    style::SetAttribute(style::Attribute::Bold),
                    // style::SetForegroundColor(Color::Green),
                    Print(c),
                    style::ResetColor,
                )?;
            } else {
                queue!(
                    scr,
                    style::SetForegroundColor(Color::White),
                    style::SetAttribute(style::Attribute::Dim),
                    Print(c),
                    style::ResetColor,
                )?;
            }

            pos += 1;
        }

        if pos < self.width {
            queue!(scr, style::ResetColor, Print('§'),)?;
            pos += 1;
        }

        while pos < self.width {
            queue!(
                scr,
                Print(' '),
            )?;
            pos += 1;
        }

        // 8
        let mut ts = self.start.elapsed().as_secs();
        let t_sec = ts % 60; ts /= 60;
        let t_min = ts % 60; ts /= 60;
        let t_hour = ts % 24;
        let ts = format!("{:02}:{:02}:{:02}", t_hour, t_min, t_sec);

        // 7
        let accuracy = if self.wrong + self.right == 0 {
            " 00.00%".to_owned()
        } else {
            format!("{:>6.2}%", 100.0 * self.right as f32 / ((self.wrong + self.right) as f32))
        };

        // 9
        let cpm = 60.0 * ((self.wrong + self.right) as f32) / self.start.elapsed().as_secs_f32();
        let cpm = format!("{:>4.0}(cpm)", cpm.min(9999.0));

        // 9
        let wpm = 60.0 * (self.words as f32) / self.start.elapsed().as_secs_f32();
        let wpm = format!("{:>4.0}(wpm)", wpm.min(9999.0));

        queue!(
            scr,
            style::SetForegroundColor(Color::Green),
            Print(']'),
            style::ResetColor,

            Print("      "),

            style::SetForegroundColor(Color::Green),
            Print("✓: "),
            style::SetForegroundColor(Color::White),
            Print(accuracy),
            Print(" "),

            style::SetForegroundColor(Color::Cyan),
            Print("⚑: "),
            style::SetForegroundColor(Color::White),
            Print(cpm),
            Print("/"),
            Print(wpm),

            Print(" "),
            Print(ts),

            cursor::RestorePosition,
        )?;
        scr.flush()?;

        Ok(())
    }

    fn uneat(&mut self) {
        if self.acc.len() > 0 {
            if self.acc.len() > 1 && self.buf[self.acc.len()-1] as char == ' ' && self.buf[self.acc.len()-2] as char != ' ' {
                self.words = self.words.saturating_sub(1);
            }

            let acc = self.acc.remove(self.acc.len() - 1);
            if acc {
                self.right -= 1;
            } else {
                self.wrong -= 1;
            }
        }
    }

    fn eat(&mut self, c: char) {
        if self.acc.len() < self.buf.len() {
            if self.acc.len() > 0 && self.buf[self.acc.len()] as char == ' ' && self.buf[self.acc.len()-1] as char != ' ' {
                self.words += 1;
            }
            let _c = self.buf[self.acc.len()] as char;

            let acc = !_c.is_ascii() || (_c == '\n') || _c == c;
            self.acc.push(acc);
            if acc {
                self.right += 1;
            } else {
                self.wrong += 1;
            }
        }
    }
}

fn main() -> io::Result<()> {
    let matches = clap::App::new("typer")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Simple type program")
        .arg(clap::Arg::new("quiet").long("quiet").short('q').help("no help message"))
        .arg(clap::Arg::new("ipsum").long("ipsum").short('i').help("use lipsum engine"))
        .arg(clap::Arg::new("file").long("file").short('f').value_name("FILE").help("no help message"))
        .arg(clap::Arg::new("width").long("width").value_name("WIDTH").help("set text width"))
        .arg(clap::Arg::new("total").long("total").value_name("TOTAL").help("set text total length"))
        .get_matches();

    let quiet = matches.is_present("quiet");
    let file = matches.value_of("file");
    let ipsum = matches.is_present("ipsum");
    let width = matches.value_of("width").map(|s| s.parse().unwrap()).unwrap_or(80);
    let total = matches.value_of("total").map(|s| s.parse().unwrap()).unwrap_or(1024);

    let gen;
    let mut _ipsum_buf = String::with_capacity(1024);
    match (file, ipsum) {
        (None, _) => {
            // ipsum
            let mut mark = lipsum::MarkovChain::new();
            mark.learn(lipsum::LOREM_IPSUM);
            mark.learn(lipsum::LIBER_PRIMUS);
            gen = TextGen::Ipsum(Ipsum {
                mark,
                last: vec![]
            });
        },
        (Some(fpath), true) => {
            let mut mark = lipsum::MarkovChain::new();
            let mut file = std::fs::File::open(fpath).expect("Fail to open file");
            file.read_to_string(&mut _ipsum_buf).expect("Fail to read to string");
            mark.learn(&_ipsum_buf);
            gen = TextGen::Ipsum(Ipsum {
                mark,
                last: vec![]
            });
        },
        (Some(fpath), false) => {
            let file = std::fs::File::open(fpath).expect("Fail to open file");
            gen = TextGen::File(file);
        },
    }

    if !quiet {
        App::<TextGen<'_>>::print_help()?;
    }

    let mut app: App<_> = App::new(width, total, gen);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, cursor::Hide)?;

    // set up key reader
    let (tx_tevent, rx_tevent) = channel::bounded(1024);
    let _h = std::thread::Builder::new()
        .name("term_events".to_owned())
        .spawn(move || {
            while let Ok(event) = crossterm::event::read() {
                tx_tevent.send(event).ok();
            }
        });

    // set up timer: 60 fps
    let rx_timer = channel::tick(std::time::Duration::from_millis(15));

    loop {
        select! {
            recv(rx_timer) -> _ => {
                let (columns, _) = crossterm::terminal::size()?;
                app.draw(&mut stdout, columns)?;
            },
            recv(rx_tevent) -> eterm => {
                if let Ok(eterm) = eterm {
                    match eterm {
                        Event::Key(KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL }) => {
                            break;
                        },
                        Event::Key(KeyEvent { code: KeyCode::Char(c), modifiers: KeyModifiers::SHIFT }) => {
                            app.eat(c.to_ascii_uppercase());
                        },
                        Event::Key(KeyEvent { code: KeyCode::Char(c), modifiers: KeyModifiers::NONE }) => {
                            app.eat(c);
                        },
                        Event::Key(KeyEvent { code: KeyCode::Backspace, modifiers: KeyModifiers::NONE }) => {
                            app.uneat();
                        },
                        Event::Key(KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::NONE }) => {
                            if let Err(_) = app.newline() {
                                break;
                            }
                        },
                        _ => {  }
                    }
                }
            },
        }
    }

    queue!(
        stdout,
        cursor::MoveToColumn(0),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::FromCursorDown),
    )?;
    execute!(stdout, cursor::Show)?;
    disable_raw_mode()?;

    println!("Report: {} words per minute", 42);
    stdout.flush()?;

    Ok(())
}
