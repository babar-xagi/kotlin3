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
        self.tokenize_mode(&mut tokens, None)?;
        Ok(tokens)
    }

    fn tokenize_mode(
        &mut self,
        tokens: &mut Vec<Token<'a>>,
        mut brace_depth: Option<&mut usize>,
    ) -> Result<(), LexError> {
        while let Some(res) = self.inner.next() {
            let span_range = self.inner.span();
            let span = Span::new(span_range.start as u32, span_range.end as u32);
            let text = &self.source[span_range.clone()];
            match res {
                Ok(TokenKind::Quote) => {
                    self.handle_string(span_range.start, false, tokens)?;
                }
                Ok(TokenKind::TripleQuote) => {
                    self.handle_string(span_range.start, true, tokens)?;
                }
                Ok(TokenKind::LBrace) => {
                    if let Some(ref mut depth) = brace_depth {
                        **depth += 1;
                    }
                    tokens.push(Token {
                        kind: TokenKind::LBrace,
                        text,
                        span,
                    });
                }
                Ok(TokenKind::RBrace) => {
                    if let Some(ref mut depth) = brace_depth {
                        if **depth == 0 {
                            return Ok(());
                        }
                        **depth -= 1;
                    }
                    tokens.push(Token {
                        kind: TokenKind::RBrace,
                        text,
                        span,
                    });
                }
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
        Ok(())
    }

    fn handle_string(
        &mut self,
        quote_start: usize,
        is_triple: bool,
        tokens: &mut Vec<Token<'a>>,
    ) -> Result<(), LexError> {
        let delim = if is_triple { "\"\"\"" } else { "\"" };
        let delim_len = delim.len();
        let content_start = quote_start + delim_len;

        let mut has_interpolation = false;
        let mut i = content_start;
        let bytes = self.source.as_bytes();
        let len = bytes.len();
        let mut closing_pos = None;

        while i < len {
            if self.source[i..].starts_with(delim) {
                closing_pos = Some(i);
                break;
            }
            if !is_triple && bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'$' && i + 1 < len {
                if bytes[i + 1] == b'{' {
                    has_interpolation = true;
                    break;
                } else if bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_' {
                    has_interpolation = true;
                    break;
                }
            }
            i += 1;
        }

        if !has_interpolation {
            let close = match closing_pos {
                Some(c) => c,
                None => {
                    let mut found = None;
                    while i < len {
                        if self.source[i..].starts_with(delim) {
                            found = Some(i);
                            break;
                        }
                        if !is_triple && bytes[i] == b'\\' {
                            i += 2;
                            continue;
                        }
                        i += 1;
                    }
                    found.unwrap_or(len)
                }
            };
            let end_pos = (close + delim_len).min(len);
            let text = &self.source[quote_start..end_pos];
            let span = Span::new(quote_start as u32, end_pos as u32);
            let kind = if is_triple {
                TokenKind::MultiLineStringLiteral
            } else {
                TokenKind::StringLiteral
            };
            tokens.push(Token { kind, text, span });

            let to_bump = end_pos.saturating_sub(quote_start + delim_len);
            if to_bump > 0 {
                self.inner.bump(to_bump);
            }
            return Ok(());
        }

        let start_span = Span::new(quote_start as u32, content_start as u32);
        tokens.push(Token {
            kind: TokenKind::StringStart,
            text: &self.source[quote_start..content_start],
            span: start_span,
        });

        let mut cur_inner_pos = content_start;
        let mut pos = content_start;

        while pos < len {
            if self.source[pos..].starts_with(delim) {
                let end_pos = pos + delim_len;
                let span = Span::new(pos as u32, end_pos as u32);
                tokens.push(Token {
                    kind: TokenKind::StringEnd,
                    text: &self.source[pos..end_pos],
                    span,
                });
                if end_pos > cur_inner_pos {
                    self.inner.bump(end_pos - cur_inner_pos);
                }
                return Ok(());
            }

            if self.source[pos..].starts_with("${") {
                let expr_start = pos;
                let span = Span::new(expr_start as u32, (expr_start + 2) as u32);
                tokens.push(Token {
                    kind: TokenKind::TemplateExprStart,
                    text: &self.source[expr_start..expr_start + 2],
                    span,
                });
                pos += 2;
                if pos > cur_inner_pos {
                    self.inner.bump(pos - cur_inner_pos);
                }

                let mut depth = 0;
                self.tokenize_mode(tokens, Some(&mut depth))?;

                let rbrace_span = self.inner.span();
                let end_span = Span::new(rbrace_span.start as u32, rbrace_span.end as u32);
                tokens.push(Token {
                    kind: TokenKind::TemplateExprEnd,
                    text: &self.source[rbrace_span.clone()],
                    span: end_span,
                });
                cur_inner_pos = rbrace_span.end;
                pos = rbrace_span.end;
            } else if bytes[pos] == b'$'
                && pos + 1 < len
                && (bytes[pos + 1].is_ascii_alphabetic() || bytes[pos + 1] == b'_')
            {
                let start_id = pos + 1;
                let mut end_id = start_id;
                while end_id < len && (bytes[end_id].is_ascii_alphanumeric() || bytes[end_id] == b'_') {
                    end_id += 1;
                }
                tokens.push(Token {
                    kind: TokenKind::TemplateExprStart,
                    text: &self.source[pos..pos + 1],
                    span: Span::new(pos as u32, (pos + 1) as u32),
                });
                tokens.push(Token {
                    kind: TokenKind::Identifier,
                    text: &self.source[start_id..end_id],
                    span: Span::new(start_id as u32, end_id as u32),
                });
                tokens.push(Token {
                    kind: TokenKind::TemplateExprEnd,
                    text: "",
                    span: Span::new(end_id as u32, end_id as u32),
                });
                pos = end_id;
                if pos > cur_inner_pos {
                    self.inner.bump(pos - cur_inner_pos);
                    cur_inner_pos = pos;
                }
            } else {
                let start_lit = pos;
                while pos < len && !self.source[pos..].starts_with(delim) {
                    if self.source[pos..].starts_with("${") {
                        break;
                    }
                    if bytes[pos] == b'$'
                        && pos + 1 < len
                        && (bytes[pos + 1].is_ascii_alphabetic() || bytes[pos + 1] == b'_')
                    {
                        break;
                    }
                    if !is_triple && bytes[pos] == b'\\' {
                        pos += 2.min(len - pos);
                    } else {
                        pos += 1;
                    }
                }
                if pos > start_lit {
                    let span = Span::new(start_lit as u32, pos as u32);
                    tokens.push(Token {
                        kind: TokenKind::StringContent,
                        text: &self.source[start_lit..pos],
                        span,
                    });
                    if pos > cur_inner_pos {
                        self.inner.bump(pos - cur_inner_pos);
                        cur_inner_pos = pos;
                    }
                }
            }
        }

        let span = Span::new(quote_start as u32, pos as u32);
        tokens.push(Token {
            kind: TokenKind::StringEnd,
            text: "",
            span,
        });
        Ok(())
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
