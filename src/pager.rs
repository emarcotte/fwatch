mod input;
mod state;
mod command_prompt;

use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::io::{Write as IoWrite, stdout, stdin, Stdout, };
use std::sync::{Mutex, RwLock};
use termion::input::TermRead;
use self::{
    state::PagerState,
    input::InputState,
};
use termion::raw::IntoRawMode;


// TODO: Should we lock the whole thing or ?
pub struct Pager {
    offset: RwLock<(u16, u16)>,
    lines: RwLock<Vec<String>>,
    stdout: Mutex<termion::raw::RawTerminal<Stdout>>,
    state: RwLock<InputState>,
}

impl Drop for Pager {
    fn drop(&mut self) {
        write!(stdout(), "{}", termion::cursor::Show).unwrap();
    }
}

impl Pager {
    pub fn new() -> Result<Pager, Box<dyn Error>> {
        Ok(Pager {
            lines:  RwLock::new(vec!()),
            offset: RwLock::new((0, 0)),
            state:  RwLock::new(InputState::Free(PagerState::new(None))),
            stdout: Mutex::new(stdout().into_raw_mode()?),
        })
    }

    pub fn add(&self, line: &str) {
        if let Ok(mut lines) = self.lines.write() {
            lines.push(line.to_owned());
        }
        self.draw();
    }

    fn draw_prompt(&self, mut buf: String) -> String {
        use std::ops::Deref;
        match self.state.read() {
            Ok(guard) => match guard.deref() {
                InputState::SearchPrompt(prompt) => {
                    let _ = write!(buf, "/{text}", text = prompt.input);
                }
                _ => {},
            }
            _ => {},
        };
        buf
    }

    fn draw(&self) {
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

            let scroller_text = format!(
                "{numerator}/{denominator}",
                numerator = offset.1,
                denominator = lines.len()
            );

            write!(
                buf,
                "{start}{fg}{bg}{text}{fg_reset}{bg_reset}{end}",
                bg = termion::color::Bg(termion::color::Rgb(255, 255, 0)),
                fg = termion::color::Fg(termion::color::Rgb(0, 0, 0)),
                text = scroller_text,
                fg_reset = termion::color::Fg(termion::color::Reset),
                bg_reset = termion::color::Bg(termion::color::Reset),
                start = termion::cursor::Goto(size.0 + 1 - scroller_text.len() as u16, size.1),
                end = termion::cursor::Goto(1, size.1)
            ).unwrap();
        }

        buf = self.draw_prompt(buf);

        if let Ok(mut stdout) = self.stdout.lock() {
            write!(stdout, "{}", buf).unwrap();
            stdout.flush().unwrap();
        }
    }

    pub fn run(&self) {
        let stdin = stdin();
        self.draw();

        for c in stdin.keys() {
            let current_state = self.state.read()
                .expect("State lock poisoned on read");

            // Dispatch to relevant handler and figure out what state changes need to be applied.
            match input::handle_key(self, &current_state, &c.unwrap()) {
                InputState::Exit => break,

                // If given a new state, swap to it.
                next_state => {
                    let mut current_state = self.state.write()
                        .expect("State lock poisoned on write");
                    *current_state = next_state;
                },
            }
            self.draw();
        }
    }

    pub fn page(&self, up: bool) {
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

    pub fn slide(&self, diff: (i16, i16)) {
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


