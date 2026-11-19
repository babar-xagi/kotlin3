use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct KtFile {
    pub package: Option<PackageHeader>,
    pub imports: Vec<ImportDirective>,
    pub declarations: Vec<Declaration>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedName {
    pub parts: Vec<Ident>,
    pub span: Span,
}

impl QualifiedName {
    pub fn to_string(&self) -> String {
        self.parts
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageHeader {
    pub name: QualifiedName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDirective {
    pub path: QualifiedName,
    pub is_star: bool,
    pub alias: Option<Ident>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    Function(NamedFunction),
    Property(PropertyDeclaration),
    Class(ClassDeclaration),
}

impl Declaration {
    pub fn span(&self) -> Span {
        match self {
            Declaration::Function(f) => f.span,
            Declaration::Property(p) => p.span,
            Declaration::Class(c) => c.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamedFunction {
    pub modifiers: Vec<Modifier>,
    pub type_params: Vec<Ident>,
    pub receiver_type: Option<TypeRef>,
    pub name: Ident,
    pub value_params: Vec<ValueParam>,
    pub return_type: Option<TypeRef>,
    pub body: Option<FunctionBody>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionBody {
    Block(Vec<Statement>),
    Expression(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValueParam {
    pub is_val: bool,
    pub is_var: bool,
    pub name: Ident,
    pub type_ref: TypeRef,
    pub default_value: Option<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDeclaration {
    pub modifiers: Vec<Modifier>,
    pub is_var: bool,
    pub name: Ident,
    pub type_ref: Option<TypeRef>,
    pub initializer: Option<Expression>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDeclaration {
    pub modifiers: Vec<Modifier>,
    pub kind: ClassKind,
    pub name: Ident,
    pub primary_constructor_params: Option<Vec<ValueParam>>,
    pub super_types: Vec<SuperTypeInvocation>,
    pub declarations: Vec<Declaration>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Interface,
    Object,
    DataClass,
    EnumClass,
    SealedClass,
    AnnotationClass,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuperTypeInvocation {
    pub type_ref: TypeRef,
    pub arguments: Option<Vec<ValueArgument>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modifier {
    Public,
    Private,
    Protected,
    Internal,
    Open,
    Final,
    Abstract,
    Sealed,
    Data,
    Inline,
    Suspend,
    Override,
    Operator,
    Infix,
    Tailrec,
    External,
    Annotation,
    Companion,
    Const,
    Lateinit,
    Vararg,
    Actual,
    Expect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    User {
        name: QualifiedName,
        type_args: Vec<TypeRef>,
        is_nullable: bool,
        span: Span,
    },
    Function {
        params: Vec<TypeRef>,
        return_type: Box<TypeRef>,
        is_nullable: bool,
        span: Span,
    },
    Nullable {
        inner: Box<TypeRef>,
        span: Span,
    },
}

impl TypeRef {
    pub fn span(&self) -> Span {
        match self {
            TypeRef::User { span, .. } => *span,
            TypeRef::Function { span, .. } => *span,
            TypeRef::Nullable { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Declaration(Declaration),
    Assignment {
        target: Expression,
        op: AssignOp,
        value: Expression,
        span: Span,
    },
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    IntegerLiteral(i64, Span),
    LongLiteral(i64, Span),
    FloatLiteral(f64, Span),
    BooleanLiteral(bool, Span),
    StringLiteral(String, Span),
    StringTemplate(Vec<Expression>, Span),
    NullLiteral(Span),
    Identifier(Ident),
    This(Span),
    Super(Span),
    Binary {
        left: Box<Expression>,
        op: BinaryOp,
        right: Box<Expression>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expression>,
        is_prefix: bool,
        span: Span,
    },
    Call {
        callee: Box<Expression>,
        type_args: Vec<TypeRef>,
        args: Vec<ValueArgument>,
        trailing_lambda: Option<Box<Expression>>,
        span: Span,
    },
    MemberAccess {
        target: Box<Expression>,
        member: Ident,
        is_safe: bool,
        span: Span,
    },
    IndexAccess {
        target: Box<Expression>,
        indices: Vec<Expression>,
        span: Span,
    },
    If {
        condition: Box<Expression>,
        then_branch: Box<Expression>,
        else_branch: Option<Box<Expression>>,
        span: Span,
    },
    When {
        subject: Option<Box<Expression>>,
        entries: Vec<WhenEntry>,
        span: Span,
    },
    Block(Vec<Statement>, Span),
    Lambda {
        params: Vec<Ident>,
        body: Vec<Statement>,
        span: Span,
    },
    Return {
        value: Option<Box<Expression>>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    For {
        variable: Ident,
        iterable: Box<Expression>,
        body: Box<Expression>,
        span: Span,
    },
    While {
        condition: Box<Expression>,
        body: Box<Expression>,
        is_do_while: bool,
        span: Span,
    },
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::IntegerLiteral(_, s)
            | Expression::LongLiteral(_, s)
            | Expression::FloatLiteral(_, s)
            | Expression::BooleanLiteral(_, s)
            | Expression::StringLiteral(_, s)
            | Expression::StringTemplate(_, s)
            | Expression::NullLiteral(s)
            | Expression::This(s)
            | Expression::Super(s)
            | Expression::Binary { span: s, .. }
            | Expression::Unary { span: s, .. }
            | Expression::Call { span: s, .. }
            | Expression::MemberAccess { span: s, .. }
            | Expression::IndexAccess { span: s, .. }
            | Expression::If { span: s, .. }
            | Expression::When { span: s, .. }
            | Expression::Block(_, s)
            | Expression::Lambda { span: s, .. }
            | Expression::Return { span: s, .. }
            | Expression::Break(s)
            | Expression::Continue(s)
            | Expression::For { span: s, .. }
            | Expression::While { span: s, .. } => *s,
            Expression::Identifier(id) => id.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValueArgument {
    pub name: Option<Ident>,
    pub value: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenEntry {
    pub conditions: Vec<WhenCondition>,
    pub body: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WhenCondition {
    Expression(Expression),
    Is(TypeRef, bool),
    In(Expression, bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    IdentityEq,
    IdentityNotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    And,
    Or,
    Elvis,
    Range,
    RangeExclusive,
    In,
    NotIn,
    Is,
    NotIs,
    As,
    AsSafe,
    Shl,
    Shr,
    Ushr,
    BitAnd,
    BitOr,
    BitXor,
    Until,
    DownTo,
    Step,
    Infix(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Pos,
    Not,
    Inc,
    Dec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    PlusAssign,
    MinusAssign,
    MulAssign,
    DivAssign,
    ModAssign,
}
