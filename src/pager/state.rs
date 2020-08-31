/// User input tracker for the main pager input state.
#[derive(Clone)]
pub struct PagerState {
    search: Option<String>,
    current_line: usize,
    current_pos: usize,
}

impl PagerState {
    /// Create an empty PagerState.
    pub fn new(search: Option<String>) -> PagerState {
        PagerState {
            search,
            current_line: 0,
            current_pos: 0,
        }
    }
}

