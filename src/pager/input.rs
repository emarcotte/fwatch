use termion::event::Key;
use super::{
    Pager,
    state::{
        PagerState,
    },
    command_prompt::{
        CommandPrompt,
    },
};

/// Top-level tracking for the state of the pager input system.
pub(crate) enum InputState {
    /// The main pager state, contains other details to track input.
    Free(PagerState),

    /// The command prompt is setting up a search
    SearchPrompt(CommandPrompt),

    /// User asked to exit.
    Exit,
}

pub(crate) fn handle_key(pager: &Pager, state: &InputState, key: &Key) -> InputState {
    match state {
        InputState::SearchPrompt(s) => search_handler(pager, &s, key),
        InputState::Free(s)         => free_handler(pager, s, key),
        _ => panic!("WOOPS"),
    }
}

fn search_handler(_pager: &Pager, prompt: &CommandPrompt, key: &Key) -> InputState {
    match key {
        Key::Char('\n') => {
            InputState::Free(PagerState::new(Some(prompt.input.to_owned())))
        },
        Key::Esc => {
            InputState::Free(prompt.previous_state.clone())
        },
        Key::Char(c) => {
            InputState::SearchPrompt(prompt.append(*c))
        },
        _ => {
            InputState::Free(prompt.previous_state.clone())
        },
    }
}

fn free_handler(pager: &Pager, state: &PagerState, key: &Key) -> InputState {
    match key {
        Key::Char('/') => return InputState::SearchPrompt(
            CommandPrompt::new(state.clone(), '/')
        ),
        Key::Ctrl('c') => return InputState::Exit,
        Key::Char('q') => return InputState::Exit,
        Key::Char('j') => pager.slide((0, 1)),
        Key::Char('k') => pager.slide((0, -1)),
        Key::Ctrl(c)   => println!("Ctrl-{}", c),
        Key::Down      => pager.slide((0, 1)),
        Key::Up        => pager.slide((0, -1)),
        Key::PageUp    => pager.page(true),
        Key::PageDown  => pager.page(false),
        c              => {
            println!("Handling {:?}", c);
        },
    }
    return InputState::Free(state.clone());
}

