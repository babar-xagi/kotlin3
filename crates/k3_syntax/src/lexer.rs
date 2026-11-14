use crate::span::Span;
use crate::token::TokenKind;
use logos::Logos;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub text: &'a str,
    pub span: Span,
}

pub struct Lexer<'a> {
    inner: logos::Lexer<'a, TokenKind>,
    source: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: TokenKind::lexer(source),
            source,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token<'a>>, LexError> {
        let mut tokens = Vec::new();
        while let Some(res) = self.inner.next() {
            let span_range = self.inner.span();
            let span = Span::new(span_range.start as u32, span_range.end as u32);
            let text = &self.source[span_range];
            match res {
                Ok(kind) => {
                    tokens.push(Token { kind, text, span });
                }
                Err(()) => {
                    return Err(LexError {
                        span,
                        text: text.to_string(),
                    });
                }
            }
        }
        Ok(tokens)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("Lexer error at {span}: unrecognized token '{text}'")]
pub struct LexError {
    pub span: Span,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function() {
        let source = r#"
            fun main(args: Array<String>) {
                val message = "Hello, Kotlin3!"
                println(message)
            }
        "#;
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().expect("failed to tokenize valid Kotlin");
        
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::Fun);
        assert_eq!(kinds[1], TokenKind::Identifier); // main
        assert_eq!(kinds[2], TokenKind::LParen);
        assert_eq!(kinds[3], TokenKind::Identifier); // args
        assert_eq!(kinds[4], TokenKind::Colon);
        assert_eq!(kinds[5], TokenKind::Identifier); // Array
        assert_eq!(kinds[6], TokenKind::LAngle);
        assert_eq!(kinds[7], TokenKind::Identifier); // String
        assert_eq!(kinds[8], TokenKind::RAngle);
        assert_eq!(kinds[9], TokenKind::RParen);
        assert_eq!(kinds[10], TokenKind::LBrace);
        assert_eq!(kinds[11], TokenKind::Val);
        assert_eq!(kinds[12], TokenKind::Identifier); // message
        assert_eq!(kinds[13], TokenKind::Assign);
        assert_eq!(kinds[14], TokenKind::StringLiteral);
        assert_eq!(kinds[15], TokenKind::Identifier); // println
        assert_eq!(kinds[16], TokenKind::LParen);
        assert_eq!(kinds[17], TokenKind::Identifier); // message
        assert_eq!(kinds[18], TokenKind::RParen);
        assert_eq!(kinds[19], TokenKind::RBrace);
    }

    #[test]
    fn test_operators_and_numbers() {
        let source = "1 + 2 * 3.14 == 7.28 && a?.b ?: 0";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
        assert_eq!(kinds[0], TokenKind::IntegerLiteral);
        assert_eq!(kinds[1], TokenKind::Plus);
        assert_eq!(kinds[2], TokenKind::IntegerLiteral);
        assert_eq!(kinds[3], TokenKind::Star);
        assert_eq!(kinds[4], TokenKind::FloatLiteral);
        assert_eq!(kinds[5], TokenKind::EqEq);
        assert_eq!(kinds[6], TokenKind::FloatLiteral);
        assert_eq!(kinds[7], TokenKind::AndAnd);
        assert_eq!(kinds[8], TokenKind::Identifier);
        assert_eq!(kinds[9], TokenKind::SafeNav);
        assert_eq!(kinds[10], TokenKind::Identifier);
        assert_eq!(kinds[11], TokenKind::Elvis);
        assert_eq!(kinds[12], TokenKind::IntegerLiteral);
    }
}
