#[derive(Default)]
pub(super) struct Lexer {
    pub in_comment: bool,
}

impl Lexer {
    pub fn line(&mut self, text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if self.in_comment {
                if ch == '}' && chars.peek() == Some(&'}') {
                    chars.next();
                    self.in_comment = false;
                }
            } else if ch == '\\' {
                result.push(ch);
                if let Some(escaped) = chars.next() {
                    result.push(escaped);
                }
            } else if ch == '{' && chars.peek() == Some(&'{') {
                chars.next();
                self.in_comment = true;
            } else {
                result.push(ch);
            }
        }
        result
    }
}
