
#[derive(Debug, Default, Clone)]
pub struct WordBuffer {
    chars: Vec<char>,
}

pub const MAX_WORD_LEN: usize = 64;

impl WordBuffer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, c: char) -> bool {
        if self.chars.len() >= MAX_WORD_LEN {
            self.clear();
            return false;
        }
        self.chars.push(c);
        true
    }
    pub fn backspace(&mut self) -> bool {
        self.chars.pop().is_some()
    }
    pub fn clear(&mut self) {
        for c in self.chars.iter_mut() {
            *c = '\0';
        }
        self.chars.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
    pub fn len(&self) -> usize {
        self.chars.len()
    }
    pub fn as_string(&self) -> String {
        self.chars.iter().collect()
    }
    pub fn take(&mut self) -> String {
        let s = self.as_string();
        self.clear();
        s
    }
    pub fn set(&mut self, s: &str) {
        self.clear();
        self.chars.extend(s.chars().take(MAX_WORD_LEN));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_backspace_take() {
        let mut b = WordBuffer::new();
        for c in "helo".chars() {
            b.push(c);
        }
        b.backspace();
        b.push('l');
        b.push('o');
        assert_eq!(b.len(), 5);
        assert_eq!(b.take(), "hello");
        assert!(b.is_empty());
        assert!(!b.backspace());
    }

    #[test]
    fn overflow_clears() {
        let mut b = WordBuffer::new();
        for _ in 0..MAX_WORD_LEN {
            assert!(b.push('a'));
        }
        assert!(!b.push('a'));
        assert!(b.is_empty());
    }
}
