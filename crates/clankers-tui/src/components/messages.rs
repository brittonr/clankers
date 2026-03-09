//! Message display with scrolling

/// Scroll state for message view
#[derive(Debug, Clone)]
pub struct MessageScroll {
    pub offset: usize,
    pub auto_scroll: bool,
}

impl Default for MessageScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageScroll {
    pub fn new() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.offset = self.offset.saturating_add(amount);
        // auto_scroll is re-enabled during render when we detect we're at the bottom
    }

    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_scroll = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
    }
}
