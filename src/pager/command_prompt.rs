use super::state::{PagerState};

#[derive(Clone)]
pub struct CommandPrompt {
    pub previous_state: PagerState,
    pub prefix: char,
    pub input: String,
    pub pos: usize,
}

impl CommandPrompt {
    /// Create an empty CommandPrompt.
    pub fn new(previous_state: PagerState, prefix: char) -> CommandPrompt {
        CommandPrompt {
            previous_state,
            prefix,
            input: "".to_owned(),
            pos: 0,
        }
    }

    /// Make a new prompt with the given character inserted where cursor currently is.
    pub fn append(&self, c: char) -> CommandPrompt {
        let mut new_prompt = self.clone();
        new_prompt.input.insert(self.pos, c);
        new_prompt.pos += 1;
        new_prompt
    }
}

