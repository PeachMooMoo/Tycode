pub struct State {
    pub show_reasoning: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            show_reasoning: false,
        }
    }
}
