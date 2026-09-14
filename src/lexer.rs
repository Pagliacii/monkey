use crate::token::{Token, TokenType, lookup_keyword};

fn is_letter(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

pub struct Lexer {
    input: String,
    position: usize,
    read_position: usize,
    ch: Option<char>,
}

impl Lexer {
    pub fn new(input: impl Into<String>) -> Self {
        let mut l = Lexer {
            input: input.into(),
            position: 0,
            read_position: 0,
            ch: None,
        };
        l.read_char();
        l
    }

    pub fn next_token(&mut self) -> Token {
        self.next_token_with_span().0
    }

    pub(crate) fn next_token_with_span(&mut self) -> (Token, std::ops::Range<usize>) {
        self.skip_whitespace();
        let start = self.position;
        let token = self.read_token();
        (token, start..self.position)
    }

    fn read_token(&mut self) -> Token {
        let tok = match self.ch {
            Some('=') => {
                if self.peek_char() == Some('=') {
                    self.read_char();
                    Token::new(TokenType::EQ, "==")
                } else {
                    Token::new(TokenType::ASSIGN, "=")
                }
            }
            Some('+') => Token::new(TokenType::PLUS, "+"),
            Some('-') => Token::new(TokenType::MINUS, "-"),
            Some('!') => {
                if self.peek_char() == Some('=') {
                    self.read_char();
                    Token::new(TokenType::NOT_EQ, "!=")
                } else {
                    Token::new(TokenType::BANG, "!")
                }
            }
            Some('*') => Token::new(TokenType::ASTERISK, "*"),
            Some('/') => Token::new(TokenType::SLASH, "/"),
            Some('<') => Token::new(TokenType::LT, "<"),
            Some('>') => Token::new(TokenType::GT, ">"),
            Some('(') => Token::new(TokenType::LPAREN, "("),
            Some(')') => Token::new(TokenType::RPAREN, ")"),
            Some('{') => Token::new(TokenType::LBRACE, "{"),
            Some('}') => Token::new(TokenType::RBRACE, "}"),
            Some(',') => Token::new(TokenType::COMMA, ","),
            Some(';') => Token::new(TokenType::SEMICOLON, ";"),
            Some('"') => Token::new(TokenType::STRING, self.read_string()),
            Some('[') => Token::new(TokenType::LBRACKET, "["),
            Some(']') => Token::new(TokenType::RBRACKET, "]"),
            Some(':') => Token::new(TokenType::COLON, ":"),
            Some(ch) => {
                if is_letter(ch) {
                    let literal = self.read_identifier();
                    return Token::new(lookup_keyword(&literal), literal);
                }
                if ch.is_ascii_digit() {
                    let literal = self.read_number();
                    return Token::new(TokenType::INT, literal);
                }
                Token::new(TokenType::ILLEGAL, ch)
            }
            None => Token::new(TokenType::EOF, ""),
        };
        self.read_char();
        tok
    }

    fn read_char(&mut self) {
        self.position = self.read_position;
        self.ch = self.input[self.position..].chars().next();
        if let Some(character) = self.ch {
            self.read_position += character.len_utf8();
        }
    }

    fn read_string(&mut self) -> String {
        let mut string_literal = String::new();
        loop {
            self.read_char();
            match self.ch {
                Some('"') | Some('\0') | None => break,
                Some(ch) => string_literal.push(ch),
            }
        }
        string_literal
    }

    fn read_identifier(&mut self) -> String {
        let position = self.position;
        while let Some(ch) = self.ch {
            if is_letter(ch) {
                self.read_char();
            } else {
                break;
            }
        }
        self.input[position..self.position].to_string()
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.ch {
            if ch.is_whitespace() {
                self.read_char();
            } else {
                break;
            }
        }
    }

    fn read_number(&mut self) -> String {
        let position = self.position;
        while let Some(ch) = self.ch {
            if ch.is_ascii_digit() {
                self.read_char();
            } else {
                break;
            }
        }
        self.input[position..self.position].to_string()
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.read_position..].chars().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_token_spans_in_source_bytes() {
        let input = " let x = \"你好\" != 12;";
        let mut lexer = Lexer::new(input);
        for expected in ["let", "x", "=", "\"你好\"", "!=", "12", ";"] {
            let (_, span) = lexer.next_token_with_span();
            assert_eq!(&input[span], expected);
        }
        for _ in 0..2 {
            let (token, span) = lexer.next_token_with_span();
            assert_eq!(token.token_type(), TokenType::EOF);
            assert_eq!(span, input.len()..input.len());
        }
    }

    #[test]
    fn spans_unterminated_strings_and_non_ascii_input() {
        for (input, expected_type, expected_literal) in [
            ("\"你好", TokenType::STRING, "你好"),
            ("\"你好\"", TokenType::STRING, "你好"),
            ("🙂", TokenType::ILLEGAL, "🙂"),
        ] {
            let mut lexer = Lexer::new(input);
            let (token, span) = lexer.next_token_with_span();
            assert_eq!(token, Token::new(expected_type, expected_literal));
            assert_eq!(span, 0..input.len());
            assert_eq!(lexer.next_token().token_type(), TokenType::EOF);
        }
    }

    #[test]
    fn test_next_token() {
        let input = r#"let five = 5;
let ten = 10;

let add = fn(x, y) {
  x + y;
};

let result = add(five, ten);
!-/*5;
5 < 10 > 5;

if (5 < 10) {
  return true;
} else {
  return false;
}

10 == 10;
10 != 9;
"foobar"
"foo bar"
[1, 2];
{"foo": "bar"}
macro(x, y) { x + y; };
"#;

        let expected = [
            (TokenType::LET, "let"),
            (TokenType::IDENT, "five"),
            (TokenType::ASSIGN, "="),
            (TokenType::INT, "5"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::LET, "let"),
            (TokenType::IDENT, "ten"),
            (TokenType::ASSIGN, "="),
            (TokenType::INT, "10"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::LET, "let"),
            (TokenType::IDENT, "add"),
            (TokenType::ASSIGN, "="),
            (TokenType::FUNCTION, "fn"),
            (TokenType::LPAREN, "("),
            (TokenType::IDENT, "x"),
            (TokenType::COMMA, ","),
            (TokenType::IDENT, "y"),
            (TokenType::RPAREN, ")"),
            (TokenType::LBRACE, "{"),
            (TokenType::IDENT, "x"),
            (TokenType::PLUS, "+"),
            (TokenType::IDENT, "y"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::RBRACE, "}"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::LET, "let"),
            (TokenType::IDENT, "result"),
            (TokenType::ASSIGN, "="),
            (TokenType::IDENT, "add"),
            (TokenType::LPAREN, "("),
            (TokenType::IDENT, "five"),
            (TokenType::COMMA, ","),
            (TokenType::IDENT, "ten"),
            (TokenType::RPAREN, ")"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::BANG, "!"),
            (TokenType::MINUS, "-"),
            (TokenType::SLASH, "/"),
            (TokenType::ASTERISK, "*"),
            (TokenType::INT, "5"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::INT, "5"),
            (TokenType::LT, "<"),
            (TokenType::INT, "10"),
            (TokenType::GT, ">"),
            (TokenType::INT, "5"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::IF, "if"),
            (TokenType::LPAREN, "("),
            (TokenType::INT, "5"),
            (TokenType::LT, "<"),
            (TokenType::INT, "10"),
            (TokenType::RPAREN, ")"),
            (TokenType::LBRACE, "{"),
            (TokenType::RETURN, "return"),
            (TokenType::TRUE, "true"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::RBRACE, "}"),
            (TokenType::ELSE, "else"),
            (TokenType::LBRACE, "{"),
            (TokenType::RETURN, "return"),
            (TokenType::FALSE, "false"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::RBRACE, "}"),
            (TokenType::INT, "10"),
            (TokenType::EQ, "=="),
            (TokenType::INT, "10"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::INT, "10"),
            (TokenType::NOT_EQ, "!="),
            (TokenType::INT, "9"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::STRING, "foobar"),
            (TokenType::STRING, "foo bar"),
            (TokenType::LBRACKET, "["),
            (TokenType::INT, "1"),
            (TokenType::COMMA, ","),
            (TokenType::INT, "2"),
            (TokenType::RBRACKET, "]"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::LBRACE, "{"),
            (TokenType::STRING, "foo"),
            (TokenType::COLON, ":"),
            (TokenType::STRING, "bar"),
            (TokenType::RBRACE, "}"),
            (TokenType::MACRO, "macro"),
            (TokenType::LPAREN, "("),
            (TokenType::IDENT, "x"),
            (TokenType::COMMA, ","),
            (TokenType::IDENT, "y"),
            (TokenType::RPAREN, ")"),
            (TokenType::LBRACE, "{"),
            (TokenType::IDENT, "x"),
            (TokenType::PLUS, "+"),
            (TokenType::IDENT, "y"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::RBRACE, "}"),
            (TokenType::SEMICOLON, ";"),
            (TokenType::EOF, ""),
        ];

        let mut lexer = Lexer::new(input);
        for (index, (token_type, literal)) in expected.into_iter().enumerate() {
            assert_eq!(
                lexer.next_token(),
                Token::new(token_type, literal),
                "unexpected token at index {index}"
            );
        }
    }
}
