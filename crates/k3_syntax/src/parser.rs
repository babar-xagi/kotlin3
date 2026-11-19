use crate::ast::*;
use crate::lexer::Token;
use crate::span::Span;
use crate::token::TokenKind;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("Parse error at {span}: {message}")]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    cursor: usize,
    source_len: u32,
    in_when_body: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>, source_len: u32) -> Self {
        Self {
            tokens,
            cursor: 0,
            source_len,
            in_when_body: false,
        }
    }

    // --- Cursor Navigation ---

    pub fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    pub fn peek(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.cursor)
    }

    pub fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(|t| t.kind.clone())
    }

    pub fn current_span(&self) -> Span {
        self.peek()
            .map(|t| t.span)
            .unwrap_or_else(|| Span::empty(self.source_len))
    }

    pub fn advance(&mut self) -> Option<Token<'a>> {
        if !self.is_at_end() {
            let tok = self.tokens[self.cursor].clone();
            self.cursor += 1;
            Some(tok)
        } else {
            None
        }
    }

    pub fn check(&self, kind: TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    pub fn match_token(&mut self, kind: TokenKind) -> Option<Token<'a>> {
        if self.check(kind) {
            self.advance()
        } else {
            None
        }
    }

    pub fn expect(&mut self, kind: TokenKind, msg: &str) -> Result<Token<'a>, ParseError> {
        if let Some(tok) = self.match_token(kind) {
            Ok(tok)
        } else {
            Err(ParseError {
                message: msg.to_string(),
                span: self.current_span(),
            })
        }
    }

    pub fn is_identifier_like(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Identifier
                | TokenKind::Value
                | TokenKind::Data
                | TokenKind::Get
                | TokenKind::Set
                | TokenKind::Field
                | TokenKind::File
                | TokenKind::By
                | TokenKind::Actual
                | TokenKind::Expect
                | TokenKind::Constructor
                | TokenKind::Init
                | TokenKind::Param
                | TokenKind::Property
                | TokenKind::Receiver
                | TokenKind::SetParam
                | TokenKind::Where
                | TokenKind::Crossinline
                | TokenKind::Noinline
                | TokenKind::Reified
                | TokenKind::Dynamic
                | TokenKind::Delegate
                | TokenKind::Annotation
                | TokenKind::Companion
                | TokenKind::Const
                | TokenKind::Lateinit
                | TokenKind::Vararg
                | TokenKind::Inline
                | TokenKind::Suspend
                | TokenKind::Override
                | TokenKind::Operator
                | TokenKind::Infix
                | TokenKind::Tailrec
                | TokenKind::External
                | TokenKind::Sealed
                | TokenKind::Open
                | TokenKind::Final
                | TokenKind::Abstract
                | TokenKind::Public
                | TokenKind::Private
                | TokenKind::Protected
                | TokenKind::Internal
        )
    }

    pub fn expect_ident(&mut self, msg: &str) -> Result<Ident, ParseError> {
        if let Some(tok) = self.peek() {
            if Self::is_identifier_like(&tok.kind) {
                let tok = self.advance().unwrap();
                let mut name = tok.text.to_string();
                if name.starts_with('`') && name.ends_with('`') && name.len() >= 2 {
                    name = name[1..name.len() - 1].to_string();
                }
                return Ok(Ident::new(name, tok.span));
            }
        }
        Err(ParseError {
            message: msg.to_string(),
            span: self.current_span(),
        })
    }

    // --- File & Header Parsing ---

    pub fn parse_file(&mut self) -> Result<KtFile, ParseError> {
        let start_span = self.current_span();
        let package = self.parse_package_header()?;
        let mut imports = Vec::new();
        while self.check(TokenKind::Import) {
            imports.push(self.parse_import_directive()?);
        }

        let mut declarations = Vec::new();
        while !self.is_at_end() {
            // Skip optional semicolons
            while self.match_token(TokenKind::Semicolon).is_some() {}
            if self.is_at_end() {
                break;
            }
            declarations.push(self.parse_declaration()?);
        }

        let end_span = declarations
            .last()
            .map(|d| d.span())
            .unwrap_or(start_span);

        Ok(KtFile {
            package,
            imports,
            declarations,
            span: start_span.merge(end_span),
        })
    }

    pub fn parse_package_header(&mut self) -> Result<Option<PackageHeader>, ParseError> {
        if let Some(pkg_tok) = self.match_token(TokenKind::Package) {
            let name = self.parse_qualified_name("expected package name")?;
            self.match_token(TokenKind::Semicolon);
            let span = pkg_tok.span.merge(name.span);
            Ok(Some(PackageHeader { name, span }))
        } else {
            Ok(None)
        }
    }

    pub fn parse_import_directive(&mut self) -> Result<ImportDirective, ParseError> {
        let import_tok = self.expect(TokenKind::Import, "expected 'import'")?;
        let path = self.parse_qualified_name("expected import path")?;
        let mut is_star = false;
        let mut end_span = path.span;

        if self.match_token(TokenKind::Dot).is_some() {
            if let Some(star) = self.match_token(TokenKind::Star) {
                is_star = true;
                end_span = star.span;
            } else {
                return Err(ParseError {
                    message: "expected '*' after '.' in import".to_string(),
                    span: self.current_span(),
                });
            }
        }

        let mut alias = None;
        if self.match_token(TokenKind::As).is_some() {
            let alias_ident = self.expect_ident("expected import alias identifier")?;
            end_span = alias_ident.span;
            alias = Some(alias_ident);
        }

        self.match_token(TokenKind::Semicolon);

        Ok(ImportDirective {
            path,
            is_star,
            alias,
            span: import_tok.span.merge(end_span),
        })
    }

    pub fn parse_qualified_name(&mut self, err_msg: &str) -> Result<QualifiedName, ParseError> {
        let first = self.expect_ident(err_msg)?;
        let mut parts = vec![first];
        while self.match_token(TokenKind::Dot).is_some() {
            let next = self.expect_ident("expected identifier after '.'")?;
            parts.push(next);
        }
        let span = parts.first().unwrap().span.merge(parts.last().unwrap().span);
        Ok(QualifiedName { parts, span })
    }

    // --- Declarations ---

    pub fn parse_declaration(&mut self) -> Result<Declaration, ParseError> {
        let modifiers = self.parse_modifiers();

        if self.check(TokenKind::Fun) {
            self.parse_function(modifiers).map(Declaration::Function)
        } else if self.check(TokenKind::Val) || self.check(TokenKind::Var) {
            self.parse_property(modifiers).map(Declaration::Property)
        } else if self.check(TokenKind::Class)
            || self.check(TokenKind::Interface)
            || self.check(TokenKind::Object)
        {
            self.parse_class(modifiers).map(Declaration::Class)
        } else {
            Err(ParseError {
                message: format!(
                    "unexpected token '{:?}' in declaration",
                    self.peek_kind()
                ),
                span: self.current_span(),
            })
        }
    }

    pub fn parse_modifiers(&mut self) -> Vec<Modifier> {
        let mut mods = Vec::new();
        while let Some(kind) = self.peek_kind() {
            let m = match kind {
                TokenKind::Public => Modifier::Public,
                TokenKind::Private => Modifier::Private,
                TokenKind::Protected => Modifier::Protected,
                TokenKind::Internal => Modifier::Internal,
                TokenKind::Open => Modifier::Open,
                TokenKind::Final => Modifier::Final,
                TokenKind::Abstract => Modifier::Abstract,
                TokenKind::Sealed => Modifier::Sealed,
                TokenKind::Data => Modifier::Data,
                TokenKind::Inline => Modifier::Inline,
                TokenKind::Suspend => Modifier::Suspend,
                TokenKind::Override => Modifier::Override,
                TokenKind::Operator => Modifier::Operator,
                TokenKind::Infix => Modifier::Infix,
                TokenKind::Tailrec => Modifier::Tailrec,
                TokenKind::External => Modifier::External,
                TokenKind::Annotation => Modifier::Annotation,
                TokenKind::Companion => Modifier::Companion,
                TokenKind::Const => Modifier::Const,
                TokenKind::Lateinit => Modifier::Lateinit,
                TokenKind::Vararg => Modifier::Vararg,
                TokenKind::Actual => Modifier::Actual,
                TokenKind::Expect => Modifier::Expect,
                _ => break,
            };
            self.advance();
            mods.push(m);
        }
        mods
    }

    pub fn parse_function(&mut self, modifiers: Vec<Modifier>) -> Result<NamedFunction, ParseError> {
        let fun_tok = self.expect(TokenKind::Fun, "expected 'fun'")?;

        // Optional type parameters: `<T, R>`
        let mut type_params = Vec::new();
        if self.match_token(TokenKind::LAngle).is_some() {
            loop {
                type_params.push(self.expect_ident("expected type parameter name")?);
                if self.match_token(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.expect(TokenKind::RAngle, "expected '>' after type parameters")?;
        }

        // Could be `Receiver.name` or just `name`
        let mut receiver_type = None;
        let mut name = self.expect_ident("expected function name")?;

        if self.check(TokenKind::LAngle) {
            // Receiver type with type arguments: e.g. List<T>.name
            let mut type_args = Vec::new();
            self.advance(); // consume '<'
            loop {
                type_args.push(self.parse_type()?);
                if self.match_token(TokenKind::Comma).is_none() {
                    break;
                }
            }
            let rangle = self.expect(TokenKind::RAngle, "expected '>' after type arguments")?;
            self.expect(TokenKind::Dot, "expected '.' after receiver type")?;
            let receiver_span = name.span.merge(rangle.span);
            let name_span = name.span;
            receiver_type = Some(TypeRef::User {
                name: QualifiedName {
                    parts: vec![name],
                    span: name_span,
                },
                type_args,
                is_nullable: false,
                span: receiver_span,
            });
            name = self.expect_ident("expected function name after receiver")?;
        } else if self.match_token(TokenKind::Dot).is_some() {
            // Simple receiver: e.g. String.name
            let receiver_span = name.span;
            receiver_type = Some(TypeRef::User {
                name: QualifiedName {
                    parts: vec![name.clone()],
                    span: receiver_span,
                },
                type_args: vec![],
                is_nullable: false,
                span: receiver_span,
            });
            name = self.expect_ident("expected function name after receiver")?;
        }

        self.expect(TokenKind::LParen, "expected '(' after function name")?;
        let mut value_params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                value_params.push(self.parse_value_param()?);
                if self.match_token(TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        let rparen = self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        let mut return_type = None;
        if self.match_token(TokenKind::Colon).is_some() {
            return_type = Some(self.parse_type()?);
        }

        let mut body = None;
        let mut end_span = return_type
            .as_ref()
            .map(|t| t.span())
            .unwrap_or(rparen.span);

        if self.match_token(TokenKind::Assign).is_some() {
            let expr = self.parse_expression()?;
            end_span = expr.span();
            body = Some(FunctionBody::Expression(Box::new(expr)));
        } else if self.check(TokenKind::LBrace) {
            let block = self.parse_block()?;
            end_span = block.span();
            if let Expression::Block(stmts, _) = block {
                body = Some(FunctionBody::Block(stmts));
            }
        }

        let start_span = fun_tok.span;
        Ok(NamedFunction {
            modifiers,
            type_params,
            receiver_type,
            name,
            value_params,
            return_type,
            body,
            span: start_span.merge(end_span),
        })
    }

    pub fn parse_value_param(&mut self) -> Result<ValueParam, ParseError> {
        let is_val = self.match_token(TokenKind::Val).is_some();
        let is_var = !is_val && self.match_token(TokenKind::Var).is_some();
        let name = self.expect_ident("expected parameter name")?;
        self.expect(TokenKind::Colon, "expected ':' after parameter name")?;
        let type_ref = self.parse_type()?;

        let mut default_value = None;
        let mut end_span = type_ref.span();
        if self.match_token(TokenKind::Assign).is_some() {
            let expr = self.parse_expression()?;
            end_span = expr.span();
            default_value = Some(expr);
        }

        let span = name.span.merge(end_span);
        Ok(ValueParam {
            is_val,
            is_var,
            name,
            type_ref,
            default_value,
            span,
        })
    }

    pub fn parse_property(&mut self, modifiers: Vec<Modifier>) -> Result<PropertyDeclaration, ParseError> {
        let is_var = if self.match_token(TokenKind::Var).is_some() {
            true
        } else {
            self.expect(TokenKind::Val, "expected 'val' or 'var'")?;
            false
        };

        let name = self.expect_ident("expected property name")?;
        let name_span = name.span;
        let mut type_ref = None;
        if self.match_token(TokenKind::Colon).is_some() {
            type_ref = Some(self.parse_type()?);
        }

        let mut initializer = None;
        let mut end_span = type_ref
            .as_ref()
            .map(|t| t.span())
            .unwrap_or(name_span);

        if self.match_token(TokenKind::Assign).is_some() {
            let expr = self.parse_expression()?;
            end_span = expr.span();
            initializer = Some(expr);
        }

        self.match_token(TokenKind::Semicolon);

        Ok(PropertyDeclaration {
            modifiers,
            is_var,
            name,
            type_ref,
            initializer,
            span: name_span.merge(end_span),
        })
    }

    pub fn parse_class(&mut self, modifiers: Vec<Modifier>) -> Result<ClassDeclaration, ParseError> {
        let kind = if self.match_token(TokenKind::Interface).is_some() {
            ClassKind::Interface
        } else if self.match_token(TokenKind::Object).is_some() {
            ClassKind::Object
        } else {
            self.expect(TokenKind::Class, "expected 'class', 'interface', or 'object'")?;
            if modifiers.contains(&Modifier::Data) {
                ClassKind::DataClass
            } else if modifiers.contains(&Modifier::Sealed) {
                ClassKind::SealedClass
            } else {
                ClassKind::Class
            }
        };

        let name = self.expect_ident("expected class name")?;
        let name_span = name.span;
        let mut primary_constructor_params = None;

        if self.match_token(TokenKind::LParen).is_some() {
            let mut params = Vec::new();
            if !self.check(TokenKind::RParen) {
                loop {
                    params.push(self.parse_value_param()?);
                    if self.match_token(TokenKind::Comma).is_none() {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen, "expected ')' after constructor params")?;
            primary_constructor_params = Some(params);
        }

        let mut super_types = Vec::new();
        if self.match_token(TokenKind::Colon).is_some() {
            loop {
                let ty = self.parse_type()?;
                let mut args = None;
                if self.match_token(TokenKind::LParen).is_some() {
                    let mut arg_list = Vec::new();
                    if !self.check(TokenKind::RParen) {
                        loop {
                            arg_list.push(self.parse_value_argument()?);
                            if self.match_token(TokenKind::Comma).is_none() {
                                break;
                            }
                        }
                    }
                    self.expect(TokenKind::RParen, "expected ')' after supertype arguments")?;
                    args = Some(arg_list);
                }
                let span = ty.span();
                super_types.push(SuperTypeInvocation {
                    type_ref: ty,
                    arguments: args,
                    span,
                });
                if self.match_token(TokenKind::Comma).is_none() {
                    break;
                }
            }
        }

        let mut declarations = Vec::new();
        let mut end_span = name.span;

        if self.match_token(TokenKind::LBrace).is_some() {
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                while self.match_token(TokenKind::Semicolon).is_some() {}
                if self.check(TokenKind::RBrace) {
                    break;
                }
                declarations.push(self.parse_declaration()?);
            }
            let rbrace = self.expect(TokenKind::RBrace, "expected '}' closing class body")?;
            end_span = rbrace.span;
        }

        Ok(ClassDeclaration {
            modifiers,
            kind,
            name,
            primary_constructor_params,
            super_types,
            declarations,
            span: name_span.merge(end_span),
        })
    }

    // --- Type Parsing ---

    pub fn parse_type(&mut self) -> Result<TypeRef, ParseError> {
        if let Some(lparen) = self.match_token(TokenKind::LParen) {
            let mut params = Vec::new();
            if !self.check(TokenKind::RParen) {
                loop {
                    params.push(self.parse_type()?);
                    if self.match_token(TokenKind::Comma).is_none() {
                        break;
                    }
                }
            }
            let _rparen = self.expect(TokenKind::RParen, "expected ')' in function type")?;
            self.expect(TokenKind::Arrow, "expected '->' in function type")?;
            let return_type = self.parse_type()?;
            let is_nullable = self.match_token(TokenKind::Question).is_some();
            let span = lparen.span.merge(return_type.span());
            return Ok(TypeRef::Function {
                params,
                return_type: Box::new(return_type),
                is_nullable,
                span,
            });
        }

        let qname = self.parse_qualified_name("expected type name")?;
        let mut type_args = Vec::new();

        if self.match_token(TokenKind::LAngle).is_some() {
            loop {
                type_args.push(self.parse_type()?);
                if self.match_token(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.expect(TokenKind::RAngle, "expected '>' after type arguments")?;
        }

        let is_nullable = self.match_token(TokenKind::Question).is_some();
        let span = qname.span;
        Ok(TypeRef::User {
            name: qname,
            type_args,
            is_nullable,
            span,
        })
    }

    // --- Expression & Statement Parsing (Pratt Parser) ---

    pub fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_expr_bp(0)
    }

    pub fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expression, ParseError> {
        let mut lhs = self.parse_prefix_or_primary()?;

        loop {
            // Check for postfix operators (calls, member access, index, !!)
            if self.is_generic_call() {
                self.expect(TokenKind::LAngle, "expected '<'")?;
                let mut type_args = Vec::new();
                loop {
                    type_args.push(self.parse_type()?);
                    if self.match_token(TokenKind::Comma).is_none() {
                        break;
                    }
                }
                self.expect(TokenKind::RAngle, "expected '>' after type arguments")?;
                lhs = self.parse_call_expression_with_type_args(lhs, type_args)?;
                continue;
            }

            if self.check(TokenKind::LParen) {
                // Function call
                lhs = self.parse_call_expression(lhs)?;
                continue;
            }

            if self.check(TokenKind::LBracket) {
                // Index access [i, j]
                lhs = self.parse_index_expression(lhs)?;
                continue;
            }

            if self.check(TokenKind::Dot) || self.check(TokenKind::SafeNav) {
                let is_safe = self.match_token(TokenKind::SafeNav).is_some();
                if !is_safe {
                    self.expect(TokenKind::Dot, "expected '.'")?;
                }
                let member = self.expect_ident("expected member identifier after '.' or '?.'")?;
                let span = lhs.span().merge(member.span);
                lhs = Expression::MemberAccess {
                    target: Box::new(lhs),
                    member,
                    is_safe,
                    span,
                };
                continue;
            }

            if let Some(tok) = self.match_token(TokenKind::BangBang) {
                let span = lhs.span().merge(tok.span);
                lhs = Expression::Unary {
                    op: UnaryOp::Pos, // Not-null assertion mapped to unary
                    expr: Box::new(lhs),
                    is_prefix: false,
                    span,
                };
                continue;
            }

            if let Some(tok) = self.match_token(TokenKind::PlusPlus) {
                let span = lhs.span().merge(tok.span);
                lhs = Expression::Unary {
                    op: UnaryOp::Inc,
                    expr: Box::new(lhs),
                    is_prefix: false,
                    span,
                };
                continue;
            }

            if let Some(tok) = self.match_token(TokenKind::MinusMinus) {
                let span = lhs.span().merge(tok.span);
                lhs = Expression::Unary {
                    op: UnaryOp::Dec,
                    expr: Box::new(lhs),
                    is_prefix: false,
                    span,
                };
                continue;
            }

            // Check for trailing lambda: `foo(a) { ... }`
            if self.check(TokenKind::LBrace) && !self.is_statement_start() {
                let lambda = self.parse_lambda()?;
                let span = lhs.span().merge(lambda.span());
                if let Expression::Call { ref mut trailing_lambda, .. } = lhs {
                    if trailing_lambda.is_none() {
                        *trailing_lambda = Some(Box::new(lambda));
                        continue;
                    }
                }
                lhs = Expression::Call {
                    callee: Box::new(lhs),
                    type_args: vec![],
                    args: vec![],
                    trailing_lambda: Some(Box::new(lambda)),
                    span,
                };
                continue;
            }

            // Infix operators
            if self.in_when_body && self.is_at_when_entry_start() {
                break;
            }

            let op_kind = match self.peek_kind() {
                Some(k) => k,
                None => break,
            };

            // Check if it's an infix identifier (e.g. shl, shr, ushr, and, or, xor, until, downTo, step, etc.)
            if op_kind == TokenKind::Identifier {
                if let Some(tok) = self.peek() {
                    let infix_op = match tok.text {
                        "shl" => Some(BinaryOp::Shl),
                        "shr" => Some(BinaryOp::Shr),
                        "ushr" => Some(BinaryOp::Ushr),
                        "and" => Some(BinaryOp::BitAnd),
                        "or" => Some(BinaryOp::BitOr),
                        "xor" => Some(BinaryOp::BitXor),
                        "until" => Some(BinaryOp::Until),
                        "downTo" => Some(BinaryOp::DownTo),
                        "step" => Some(BinaryOp::Step),
                        _ => None,
                    };
                    if let Some(bin_op) = infix_op {
                        let left_bp = 14;
                        let right_bp = 15;
                        if left_bp < min_bp {
                            break;
                        }
                        self.advance();
                        let rhs = self.parse_expr_bp(right_bp)?;
                        let span = lhs.span().merge(rhs.span());
                        lhs = Expression::Binary {
                            left: Box::new(lhs),
                            op: bin_op,
                            right: Box::new(rhs),
                            span,
                        };
                        continue;
                    }
                }
            }

            let (left_bp, right_bp, bin_op) = match Self::infix_binding_power(&op_kind) {
                Some(tuple) => tuple,
                None => break,
            };

            if left_bp < min_bp {
                break;
            }

            self.advance(); // consume operator token

            let rhs = self.parse_expr_bp(right_bp)?;
            let span = lhs.span().merge(rhs.span());
            lhs = Expression::Binary {
                left: Box::new(lhs),
                op: bin_op,
                right: Box::new(rhs),
                span,
            };
        }

        Ok(lhs)
    }

    fn infix_binding_power(kind: &TokenKind) -> Option<(u8, u8, BinaryOp)> {
        match kind {
            // Assignment (right associative)
            TokenKind::Elvis => Some((3, 2, BinaryOp::Elvis)),
            TokenKind::OrOr => Some((4, 5, BinaryOp::Or)),
            TokenKind::AndAnd => Some((6, 7, BinaryOp::And)),
            TokenKind::EqEq => Some((8, 9, BinaryOp::Eq)),
            TokenKind::ExclEq => Some((8, 9, BinaryOp::NotEq)),
            TokenKind::EqEqEq => Some((8, 9, BinaryOp::IdentityEq)),
            TokenKind::ExclEqEq => Some((8, 9, BinaryOp::IdentityNotEq)),
            TokenKind::LAngle => Some((10, 11, BinaryOp::Less)),
            TokenKind::LessEq => Some((10, 11, BinaryOp::LessEq)),
            TokenKind::RAngle => Some((10, 11, BinaryOp::Greater)),
            TokenKind::GreaterEq => Some((10, 11, BinaryOp::GreaterEq)),
            TokenKind::In => Some((12, 13, BinaryOp::In)),
            TokenKind::NotIn => Some((12, 13, BinaryOp::NotIn)),
            TokenKind::Is => Some((12, 13, BinaryOp::Is)),
            TokenKind::NotIs => Some((12, 13, BinaryOp::NotIs)),
            TokenKind::As => Some((12, 13, BinaryOp::As)),
            TokenKind::AsSafe => Some((12, 13, BinaryOp::AsSafe)),
            TokenKind::DotDot => Some((14, 15, BinaryOp::Range)),
            TokenKind::DotDotLess => Some((14, 15, BinaryOp::RangeExclusive)),
            TokenKind::Plus => Some((16, 17, BinaryOp::Add)),
            TokenKind::Minus => Some((16, 17, BinaryOp::Sub)),
            TokenKind::Star => Some((18, 19, BinaryOp::Mul)),
            TokenKind::Slash => Some((18, 19, BinaryOp::Div)),
            TokenKind::Percent => Some((18, 19, BinaryOp::Mod)),
            _ => None,
        }
    }

    fn is_at_when_entry_start(&self) -> bool {
        let mut depth_paren = 0;
        let mut depth_brace = 0;
        let mut depth_bracket = 0;
        let mut idx = self.cursor;

        while idx < self.tokens.len() {
            match self.tokens[idx].kind {
                TokenKind::LParen => depth_paren += 1,
                TokenKind::RParen => {
                    if depth_paren == 0 {
                        break;
                    }
                    depth_paren -= 1;
                }
                TokenKind::LBracket => depth_bracket += 1,
                TokenKind::RBracket => {
                    if depth_bracket == 0 {
                        break;
                    }
                    depth_bracket -= 1;
                }
                TokenKind::LBrace => depth_brace += 1,
                TokenKind::RBrace => {
                    if depth_brace == 0 {
                        break;
                    }
                    depth_brace -= 1;
                }
                TokenKind::Arrow => {
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                        return true;
                    }
                }
                _ => {}
            }
            idx += 1;
        }
        false
    }

    fn is_generic_call(&self) -> bool {
        if self.cursor >= self.tokens.len() || self.tokens[self.cursor].kind != TokenKind::LAngle {
            return false;
        }
        let mut depth = 0;
        let mut idx = self.cursor;
        while idx < self.tokens.len() {
            match self.tokens[idx].kind {
                TokenKind::LAngle => depth += 1,
                TokenKind::RAngle => {
                    depth -= 1;
                    if depth == 0 {
                        if idx + 1 < self.tokens.len() {
                            let next_k = &self.tokens[idx + 1].kind;
                            return *next_k == TokenKind::LParen || *next_k == TokenKind::LBrace;
                        }
                        return false;
                    }
                }
                TokenKind::Semicolon | TokenKind::RBrace | TokenKind::RParen => return false,
                _ => {}
            }
            idx += 1;
        }
        false
    }

    fn is_statement_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(
                TokenKind::Val
                    | TokenKind::Var
                    | TokenKind::Fun
                    | TokenKind::Class
                    | TokenKind::Return
                    | TokenKind::Break
                    | TokenKind::Continue
            )
        )
    }

    fn parse_call_expression(&mut self, callee: Expression) -> Result<Expression, ParseError> {
        self.parse_call_expression_with_type_args(callee, vec![])
    }

    fn parse_call_expression_with_type_args(
        &mut self,
        callee: Expression,
        type_args: Vec<TypeRef>,
    ) -> Result<Expression, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' for call")?;
        let mut args = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                args.push(self.parse_value_argument()?);
                if self.match_token(TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        let rparen = self.expect(TokenKind::RParen, "expected ')' closing call")?;
        let mut end_span = rparen.span;
        let mut trailing_lambda = None;

        if self.check(TokenKind::LBrace) {
            let lambda = self.parse_lambda()?;
            end_span = lambda.span();
            trailing_lambda = Some(Box::new(lambda));
        }

        let span = callee.span().merge(end_span);
        Ok(Expression::Call {
            callee: Box::new(callee),
            type_args,
            args,
            trailing_lambda,
            span,
        })
    }

    fn parse_index_expression(&mut self, target: Expression) -> Result<Expression, ParseError> {
        self.expect(TokenKind::LBracket, "expected '['")?;
        let mut indices = Vec::new();
        loop {
            indices.push(self.parse_expression()?);
            if self.match_token(TokenKind::Comma).is_none() {
                break;
            }
        }
        let rbracket = self.expect(TokenKind::RBracket, "expected ']'")?;
        let span = target.span().merge(rbracket.span);
        Ok(Expression::IndexAccess {
            target: Box::new(target),
            indices,
            span,
        })
    }

    fn parse_value_argument(&mut self) -> Result<ValueArgument, ParseError> {
        let start_span = self.current_span();
        let mut name = None;

        // Check if argument is named `name = expr`
        if self.check(TokenKind::Identifier) {
            if self.cursor + 1 < self.tokens.len()
                && self.tokens[self.cursor + 1].kind == TokenKind::Assign
            {
                let id = self.expect_ident("expected identifier")?;
                self.expect(TokenKind::Assign, "expected '='")?;
                name = Some(id);
            }
        }

        let value = self.parse_expression()?;
        let span = start_span.merge(value.span());
        Ok(ValueArgument { name, value, span })
    }

    fn parse_prefix_or_primary(&mut self) -> Result<Expression, ParseError> {
        let tok = match self.peek() {
            Some(t) => t.clone(),
            None => {
                return Err(ParseError {
                    message: "unexpected end of file".to_string(),
                    span: Span::empty(self.source_len),
                });
            }
        };

        match tok.kind {
            // Unary Prefix
            TokenKind::Excl => {
                self.advance();
                let expr = self.parse_expr_bp(20)?;
                let span = tok.span.merge(expr.span());
                Ok(Expression::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                    is_prefix: true,
                    span,
                })
            }
            TokenKind::Minus => {
                self.advance();
                let expr = self.parse_expr_bp(20)?;
                let span = tok.span.merge(expr.span());
                Ok(Expression::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                    is_prefix: true,
                    span,
                })
            }
            TokenKind::Plus => {
                self.advance();
                let expr = self.parse_expr_bp(20)?;
                let span = tok.span.merge(expr.span());
                Ok(Expression::Unary {
                    op: UnaryOp::Pos,
                    expr: Box::new(expr),
                    is_prefix: true,
                    span,
                })
            }
            TokenKind::PlusPlus => {
                self.advance();
                let expr = self.parse_expr_bp(20)?;
                let span = tok.span.merge(expr.span());
                Ok(Expression::Unary {
                    op: UnaryOp::Inc,
                    expr: Box::new(expr),
                    is_prefix: true,
                    span,
                })
            }
            TokenKind::MinusMinus => {
                self.advance();
                let expr = self.parse_expr_bp(20)?;
                let span = tok.span.merge(expr.span());
                Ok(Expression::Unary {
                    op: UnaryOp::Dec,
                    expr: Box::new(expr),
                    is_prefix: true,
                    span,
                })
            }

            // Primary Literals
            TokenKind::IntegerLiteral => {
                self.advance();
                let is_long = tok.text.ends_with('l') || tok.text.ends_with('L');
                let clean_text = tok.text.trim_end_matches(|c| c == 'u' || c == 'U' || c == 'l' || c == 'L');
                let val = if clean_text.starts_with("0x") || clean_text.starts_with("0X") {
                    i64::from_str_radix(&clean_text[2..].replace('_', ""), 16).unwrap_or(0)
                } else if clean_text.starts_with("0b") || clean_text.starts_with("0B") {
                    i64::from_str_radix(&clean_text[2..].replace('_', ""), 2).unwrap_or(0)
                } else {
                    clean_text.replace('_', "").parse::<i64>().unwrap_or(0)
                };
                if is_long {
                    Ok(Expression::LongLiteral(val, tok.span))
                } else {
                    Ok(Expression::IntegerLiteral(val, tok.span))
                }
            }
            TokenKind::FloatLiteral => {
                self.advance();
                let clean_text = tok.text.trim_end_matches(|c| c == 'f' || c == 'F');
                let val = clean_text.replace('_', "").parse::<f64>().unwrap_or(0.0);
                Ok(Expression::FloatLiteral(val, tok.span))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expression::BooleanLiteral(true, tok.span))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression::BooleanLiteral(false, tok.span))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expression::NullLiteral(tok.span))
            }
            TokenKind::StringStart => {
                let start_tok = self.advance().unwrap();
                let mut parts = Vec::new();
                while !self.check(TokenKind::StringEnd) && !self.is_at_end() {
                    if self.match_token(TokenKind::TemplateExprStart).is_some() {
                        let expr = self.parse_expression()?;
                        self.expect(TokenKind::TemplateExprEnd, "expected '}' after template expression")?;
                        parts.push(expr);
                    } else if let Some(content_tok) = self.match_token(TokenKind::StringContent) {
                        parts.push(Expression::StringLiteral(content_tok.text.to_string(), content_tok.span));
                    } else {
                        return Err(ParseError {
                            message: format!("unexpected token in string template: '{:?}'", self.peek_kind()),
                            span: self.current_span(),
                        });
                    }
                }
                let end_tok = self.expect(TokenKind::StringEnd, "expected '\"' closing string template")?;
                Ok(Expression::StringTemplate(parts, start_tok.span.merge(end_tok.span)))
            }
            TokenKind::StringLiteral | TokenKind::MultiLineStringLiteral => {
                self.advance();
                let unquoted = if tok.text.starts_with("\"\"\"") && tok.text.ends_with("\"\"\"") && tok.text.len() >= 6 {
                    tok.text[3..tok.text.len() - 3].to_string()
                } else if tok.text.starts_with('"') && tok.text.ends_with('"') && tok.text.len() >= 2 {
                    tok.text[1..tok.text.len() - 1].to_string()
                } else {
                    tok.text.to_string()
                };
                Ok(Expression::StringLiteral(unquoted, tok.span))
            }
            TokenKind::This => {
                self.advance();
                Ok(Expression::This(tok.span))
            }
            TokenKind::Super => {
                self.advance();
                Ok(Expression::Super(tok.span))
            }
            k if Self::is_identifier_like(&k) => {
                let id = self.expect_ident("expected identifier")?;
                Ok(Expression::Identifier(id))
            }

            // Parenthesized expression
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.expect(TokenKind::RParen, "expected ')'")?;
                Ok(expr)
            }

            // Control flow expressions
            TokenKind::If => self.parse_if(),
            TokenKind::When => self.parse_when(),
            TokenKind::For => self.parse_for(),
            TokenKind::While => self.parse_while(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Break => {
                self.advance();
                Ok(Expression::Break(tok.span))
            }
            TokenKind::Continue => {
                self.advance();
                Ok(Expression::Continue(tok.span))
            }
            TokenKind::LBrace => self.parse_block(),

            _ => Err(ParseError {
                message: format!("unexpected token in expression: '{:?}'", tok.kind),
                span: tok.span,
            }),
        }
    }

    fn wrap_assignment_if_present(&mut self, target: Expression) -> Result<Expression, ParseError> {
        let assign_op = match self.peek_kind() {
            Some(TokenKind::Assign) => Some(AssignOp::Assign),
            Some(TokenKind::PlusAssign) => Some(AssignOp::PlusAssign),
            Some(TokenKind::MinusAssign) => Some(AssignOp::MinusAssign),
            Some(TokenKind::StarAssign) => Some(AssignOp::MulAssign),
            Some(TokenKind::SlashAssign) => Some(AssignOp::DivAssign),
            Some(TokenKind::PercentAssign) => Some(AssignOp::ModAssign),
            _ => None,
        };

        if let Some(op) = assign_op {
            self.advance();
            let value = self.parse_expression()?;
            let span = target.span().merge(value.span());
            Ok(Expression::Block(
                vec![Statement::Assignment {
                    target,
                    op,
                    value,
                    span,
                }],
                span,
            ))
        } else {
            Ok(target)
        }
    }

    pub fn parse_if(&mut self) -> Result<Expression, ParseError> {
        let if_tok = self.expect(TokenKind::If, "expected 'if'")?;
        self.expect(TokenKind::LParen, "expected '(' after 'if'")?;
        let condition = self.parse_expression()?;
        self.expect(TokenKind::RParen, "expected ')' after condition")?;

        let then_expr = self.parse_expression()?;
        let then_branch = self.wrap_assignment_if_present(then_expr)?;
        let mut else_branch = None;
        let mut end_span = then_branch.span();

        if self.match_token(TokenKind::Else).is_some() {
            let else_expr = self.parse_expression()?;
            let else_final = self.wrap_assignment_if_present(else_expr)?;
            end_span = else_final.span();
            else_branch = Some(Box::new(else_final));
        }

        Ok(Expression::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
            span: if_tok.span.merge(end_span),
        })
    }

    pub fn parse_when(&mut self) -> Result<Expression, ParseError> {
        let when_tok = self.expect(TokenKind::When, "expected 'when'")?;
        let mut subject = None;
        if self.match_token(TokenKind::LParen).is_some() {
            subject = Some(Box::new(self.parse_expression()?));
            self.expect(TokenKind::RParen, "expected ')' after when subject")?;
        }

        self.expect(TokenKind::LBrace, "expected '{' after when")?;
        let mut entries = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            while self.match_token(TokenKind::Semicolon).is_some() {}
            if self.check(TokenKind::RBrace) {
                break;
            }

            let entry_start = self.current_span();
            let mut conditions = Vec::new();

            if self.match_token(TokenKind::Else).is_some() {
                // Else entry: no conditions
            } else {
                loop {
                    if self.match_token(TokenKind::Is).is_some() {
                        let ty = self.parse_type()?;
                        conditions.push(WhenCondition::Is(ty, false));
                    } else if self.match_token(TokenKind::NotIs).is_some() {
                        let ty = self.parse_type()?;
                        conditions.push(WhenCondition::Is(ty, true));
                    } else if self.match_token(TokenKind::In).is_some() {
                        let expr = self.parse_expression()?;
                        conditions.push(WhenCondition::In(expr, false));
                    } else if self.match_token(TokenKind::NotIn).is_some() {
                        let expr = self.parse_expression()?;
                        conditions.push(WhenCondition::In(expr, true));
                    } else {
                        let expr = self.parse_expression()?;
                        conditions.push(WhenCondition::Expression(expr));
                    }

                    if self.match_token(TokenKind::Comma).is_none() {
                        break;
                    }
                }
            }

            self.expect(TokenKind::Arrow, "expected '->' in when branch")?;
            let prev_in_when = self.in_when_body;
            self.in_when_body = true;
            let body = self.parse_expression()?;
            self.in_when_body = prev_in_when;
            self.match_token(TokenKind::Semicolon);
            let entry_span = entry_start.merge(body.span());
            entries.push(WhenEntry {
                conditions,
                body,
                span: entry_span,
            });
        }

        let rbrace = self.expect(TokenKind::RBrace, "expected '}' closing when")?;
        Ok(Expression::When {
            subject,
            entries,
            span: when_tok.span.merge(rbrace.span),
        })
    }

    pub fn parse_for(&mut self) -> Result<Expression, ParseError> {
        let for_tok = self.expect(TokenKind::For, "expected 'for'")?;
        self.expect(TokenKind::LParen, "expected '(' after 'for'")?;
        let var = self.expect_ident("expected loop variable")?;
        self.expect(TokenKind::In, "expected 'in' after loop variable")?;
        let iterable = self.parse_expression()?;
        self.expect(TokenKind::RParen, "expected ')' after iterable")?;
        let body_expr = self.parse_expression()?;
        let body = self.wrap_assignment_if_present(body_expr)?;

        let span = for_tok.span.merge(body.span());
        Ok(Expression::For {
            variable: var,
            iterable: Box::new(iterable),
            body: Box::new(body),
            span,
        })
    }

    pub fn parse_while(&mut self) -> Result<Expression, ParseError> {
        let while_tok = self.expect(TokenKind::While, "expected 'while'")?;
        self.expect(TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.parse_expression()?;
        self.expect(TokenKind::RParen, "expected ')' after condition")?;
        let body_expr = self.parse_expression()?;
        let body = self.wrap_assignment_if_present(body_expr)?;

        let span = while_tok.span.merge(body.span());
        Ok(Expression::While {
            condition: Box::new(condition),
            body: Box::new(body),
            is_do_while: false,
            span,
        })
    }

    pub fn parse_do_while(&mut self) -> Result<Expression, ParseError> {
        let do_tok = self.expect(TokenKind::Do, "expected 'do'")?;
        let body = self.parse_expression()?;
        self.expect(TokenKind::While, "expected 'while' after do body")?;
        self.expect(TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.parse_expression()?;
        let rparen = self.expect(TokenKind::RParen, "expected ')'")?;

        let span = do_tok.span.merge(rparen.span);
        Ok(Expression::While {
            condition: Box::new(condition),
            body: Box::new(body),
            is_do_while: true,
            span,
        })
    }

    pub fn parse_return(&mut self) -> Result<Expression, ParseError> {
        let ret_tok = self.expect(TokenKind::Return, "expected 'return'")?;
        let mut value = None;
        let mut end_span = ret_tok.span;

        if !self.check(TokenKind::Semicolon)
            && !self.check(TokenKind::RBrace)
            && !self.is_at_end()
            && !self.is_statement_start()
        {
            let expr = self.parse_expression()?;
            end_span = expr.span();
            value = Some(Box::new(expr));
        }

        self.match_token(TokenKind::Semicolon);

        Ok(Expression::Return {
            value,
            span: ret_tok.span.merge(end_span),
        })
    }

    pub fn parse_block(&mut self) -> Result<Expression, ParseError> {
        let lbrace = self.expect(TokenKind::LBrace, "expected '{'")?;
        let mut statements = Vec::new();

        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            while self.match_token(TokenKind::Semicolon).is_some() {}
            if self.check(TokenKind::RBrace) {
                break;
            }
            statements.push(self.parse_statement()?);
        }

        let rbrace = self.expect(TokenKind::RBrace, "expected '}' closing block")?;
        Ok(Expression::Block(statements, lbrace.span.merge(rbrace.span)))
    }

    pub fn parse_lambda(&mut self) -> Result<Expression, ParseError> {
        let lbrace = self.expect(TokenKind::LBrace, "expected '{'")?;
        let mut params = Vec::new();

        // Check if lambda has parameters `a, b ->`
        // Heuristic: check if arrow appears before closing brace
        let mut has_arrow = false;
        let mut lookahead = self.cursor;
        while lookahead < self.tokens.len() {
            if self.tokens[lookahead].kind == TokenKind::Arrow {
                has_arrow = true;
                break;
            }
            if self.tokens[lookahead].kind == TokenKind::LBrace
                || self.tokens[lookahead].kind == TokenKind::RBrace
            {
                break;
            }
            lookahead += 1;
        }

        if has_arrow {
            loop {
                params.push(self.expect_ident("expected lambda parameter name")?);
                if self.match_token(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.expect(TokenKind::Arrow, "expected '->' in lambda")?;
        }

        let mut body = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            while self.match_token(TokenKind::Semicolon).is_some() {}
            if self.check(TokenKind::RBrace) {
                break;
            }
            body.push(self.parse_statement()?);
        }

        let rbrace = self.expect(TokenKind::RBrace, "expected '}'")?;
        Ok(Expression::Lambda {
            params,
            body,
            span: lbrace.span.merge(rbrace.span),
        })
    }

    pub fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        let modifiers = self.parse_modifiers();

        if self.check(TokenKind::Fun)
            || self.check(TokenKind::Val)
            || self.check(TokenKind::Var)
            || self.check(TokenKind::Class)
            || self.check(TokenKind::Interface)
            || self.check(TokenKind::Object)
        {
            if self.check(TokenKind::Fun) {
                return self.parse_function(modifiers).map(|f| Statement::Declaration(Declaration::Function(f)));
            } else if self.check(TokenKind::Val) || self.check(TokenKind::Var) {
                return self.parse_property(modifiers).map(|p| Statement::Declaration(Declaration::Property(p)));
            } else {
                return self.parse_class(modifiers).map(|c| Statement::Declaration(Declaration::Class(c)));
            }
        }

        let expr = self.parse_expression()?;

        // Check for assignment: `a = b`, `a += b`, etc.
        let assign_op = match self.peek_kind() {
            Some(TokenKind::Assign) => Some(AssignOp::Assign),
            Some(TokenKind::PlusAssign) => Some(AssignOp::PlusAssign),
            Some(TokenKind::MinusAssign) => Some(AssignOp::MinusAssign),
            Some(TokenKind::StarAssign) => Some(AssignOp::MulAssign),
            Some(TokenKind::SlashAssign) => Some(AssignOp::DivAssign),
            Some(TokenKind::PercentAssign) => Some(AssignOp::ModAssign),
            _ => None,
        };

        if let Some(op) = assign_op {
            self.advance();
            let value = self.parse_expression()?;
            let span = expr.span().merge(value.span());
            self.match_token(TokenKind::Semicolon);
            Ok(Statement::Assignment {
                target: expr,
                op,
                value,
                span,
            })
        } else {
            self.match_token(TokenKind::Semicolon);
            Ok(Statement::Expression(expr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_simple_function() {
        let code = r#"
            fun add(a: Int, b: Int): Int {
                return a + b
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let file = parser.parse_file().expect("failed to parse valid function");

        assert_eq!(file.declarations.len(), 1);
        match &file.declarations[0] {
            Declaration::Function(f) => {
                assert_eq!(f.name.name, "add");
                assert_eq!(f.value_params.len(), 2);
                assert_eq!(f.value_params[0].name.name, "a");
                assert_eq!(f.value_params[1].name.name, "b");
                assert!(f.return_type.is_some());
                assert!(f.body.is_some());
            }
            _ => panic!("expected function declaration"),
        }
    }

    #[test]
    fn test_parse_when_expression() {
        let code = r#"
            fun check(x: Any): String = when (x) {
                1 -> "one"
                is String -> "string"
                else -> "other"
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let file = parser.parse_file().expect("failed to parse when expression");
        assert_eq!(file.declarations.len(), 1);
    }

    #[test]
    fn test_parse_class_with_members() {
        let code = r#"
            data class User(val name: String, var age: Int) {
                fun greeting(): String {
                    return "Hello " + name
                }
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens, code.len() as u32);
        let file = parser.parse_file().expect("failed to parse class");
        assert_eq!(file.declarations.len(), 1);
        match &file.declarations[0] {
            Declaration::Class(c) => {
                assert_eq!(c.name.name, "User");
                assert_eq!(c.kind, ClassKind::DataClass);
                assert_eq!(c.primary_constructor_params.as_ref().unwrap().len(), 2);
                assert_eq!(c.declarations.len(), 1);
            }
            _ => panic!("expected class declaration"),
        }
    }

    #[test]
    fn test_parse_nested_string_template_format() {
        let code = r#"
            fun main() {
                val elapsedMs = 12.345
                println("${"%.3f".format(java.util.Locale.US, elapsedMs)}")
            }
        "#;
        let mut lexer = Lexer::new(code);
        let tokens = lexer.tokenize().expect("failed to tokenize nested string template");
        let mut parser = Parser::new(tokens, code.len() as u32);
        let file = parser.parse_file().expect("failed to parse nested string template");
        assert_eq!(file.declarations.len(), 1);
    }
}
