use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::io::{Write as IoWrite, stdout, stdin, Stdout, };
use std::sync::{Mutex, RwLock};
use termion::event::Key;
use termion::raw::IntoRawMode;

enum InputState {
    SearchPrompt(String),
    CommandPrompt(String),
    Free,
    Exit,
}

// TODO: Should we lock the whole thing or ?
pub struct Pager2 {
    offset: RwLock<(u16, u16)>,
    lines: RwLock<Vec<String>>,
    stdout: Mutex<termion::raw::RawTerminal<Stdout>>,
    input_state: InputState,
}

impl Drop for Pager2 {
    fn drop(&mut self) {
        write!(stdout(), "{}", termion::cursor::Show).unwrap();
    }
}

impl Pager2 {
    pub fn new() -> Result<Pager2, Box<Error>> {
        Ok(Pager2 {
            lines: RwLock::new(vec!()),
            stdout: Mutex::new(stdout().into_raw_mode().unwrap()),
            offset: RwLock::new((0, 0)),
            input_state: InputState::Free,
        })
    }

    pub fn add(&self, line: &str) {
        if let Ok(mut lines) = self.lines.write() {
            lines.push(line.to_owned());
        }
        self.draw();
    }

    fn draw(&self) {
        self.draw_base();
    }

    fn draw_base(&self) {
        let mut buf = String::with_capacity(300);
        let size = termion::terminal_size().unwrap();
        let offset = self.offset.read().unwrap().clone();

        write!(buf, "{}{}", termion::cursor::Goto(1, 1), termion::clear::All).unwrap();

        if let Ok(lines) = self.lines.read() {
            write!(buf, "{}{}", termion::cursor::Goto(1, 1), termion::clear::All).unwrap();
            if let Some(range) = lines.get(offset.1 as usize .. std::cmp::min(lines.len(), size.1 as usize - 1) + offset.1 as usize) {
                for line in range.iter() {
                    write!(buf, "{}{}", line, "\r").unwrap();
                }
            }

            let scroller_text = format!("{numerator}/{denominator}",
                                        numerator = offset.1,
                                        denominator = lines.len());

            write!(buf, "{start}{fg}{bg}{text}{fg_reset}{bg_reset}{end}",
                                   bg = termion::color::Bg(termion::color::Rgb(255, 255, 0)),
                                   fg = termion::color::Fg(termion::color::Rgb(0, 0, 0)),
                                   text = scroller_text,
                                   fg_reset = termion::color::Fg(termion::color::Reset),
                                   bg_reset = termion::color::Bg(termion::color::Reset),
                                   start = termion::cursor::Goto(size.0 + 1 - scroller_text.len() as u16, size.1),
                                   end = termion::cursor::Goto(1, size.1)).unwrap();
        }

        if let Ok(mut stdout) = self.stdout.lock() {
            write!(stdout, "{}", buf).unwrap();
            stdout.flush().unwrap();
        }
    }

    fn get_handler(&self) -> fn(&Pager2, &termion::event::Key) -> InputState {
        match self.input_state {
            InputState::Free => Pager2::free_handler,
            _ => panic!("WOOPS"),
        }
    }

    fn free_handler(&self, key: &termion::event::Key) -> InputState {
        match key {
            Key::Ctrl('c') => return InputState::Exit,
            Key::Char('q') => return InputState::Exit,
            Key::Char('j') => self.slide((0, 1)),
            Key::Char('k') => self.slide((0, -1)),
            Key::Ctrl(c)   => println!("Ctrl-{}", c),
            Key::Down      => self.slide((0, 1)),
            Key::Up        => self.slide((0, -1)),
            Key::PageUp    => self.page(true),
            Key::PageDown  => self.page(false),
            c              => {
                println!("Handling {:?}", c);
            },
        }
        return InputState::Free;
    }

    pub fn run(&self) {
        use termion::input::TermRead;
        let stdin = stdin();
        self.draw();

        for c in stdin.keys() {
            let handler = self.get_handler();
            let state = handler(self, &c.unwrap());
            if let InputState::Exit = state {
                break;
            }

            self.draw();
        }
    }

    fn page(&self, up: bool) {
        let size = termion::terminal_size().unwrap();
        let amnt = size.1 as i16 / 2 * if up { -1 } else { 1 };
        self.slide((0, amnt));
    }

    pub fn reset(&self) {
        if let Ok(mut lines) = self.lines.write() {
            lines.clear();
        }

        self.slide((0, 0));
    }

    fn slide(&self, diff: (i16, i16)) {
        let size = termion::terminal_size().unwrap();
        if let Ok(lines) = self.lines.read() {
            if let Ok(mut offset) = self.offset.write() {
                // TODO Horiz scrolling...
                let diff_target = offset.1 as i16 + diff.1;
                let min_scroll = 0;
                let max_scroll = std::cmp::max(0, lines.len() as i16 - size.1 as i16);
                offset.1 = clamp(diff_target, min_scroll, max_scroll) as u16;
            }
        }
    }
}

fn clamp<T: PartialOrd>(x: T, l: T, u: T) -> T {
    if x < l {
        l
    }
    else if x > u {
        u
    }
    else {
        x
    }
}


