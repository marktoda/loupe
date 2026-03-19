#[derive(Debug, Default)]
pub struct SearchState {
    pub query: String,
    pub is_active: bool,
    pub matches: Vec<usize>,
    pub current_match: usize,
    pub highlights_visible: bool,
}
