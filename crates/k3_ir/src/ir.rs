use k3_fir::{ConeKotlinType, ConeNullability, ConeProjection};
use k3_syntax::Span;

pub fn make_class_type(name: &str, type_args: Vec<ConeKotlinType>, is_nullable: bool) -> ConeKotlinType {
    ConeKotlinType::ClassLike {
        name: name.to_string(),
        type_arguments: type_args
            .into_iter()
            .map(|t| ConeProjection::Invariant(Box::new(t)))
            .collect(),
        nullability: if is_nullable {
            ConeNullability::Nullable
        } else {
            ConeNullability::NotNull
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrVisibility {
    Public,
    Private,
    Protected,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrClassKind {
    Class,
    Interface,
    DataClass,
    ValueClass,
    Enum,
    Object,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrDeclarationOrigin {
    Defined,
    SyntheticContinuation,
    SyntheticDefaultStub,
    Inlined,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrTypeOpKind {
    Is,
    NotIs,
    As,
    SafeAs,
    InstanceOf,
    CheckCast,
}

#[derive(Debug, Clone)]
pub struct IrModule {
    pub name: String,
    pub files: Vec<IrFile>,
}

#[derive(Debug, Clone)]
pub struct IrFile {
    pub file_name: String,
    pub package_name: String,
    pub declarations: Vec<IrDeclaration>,
}

#[derive(Debug, Clone)]
pub enum IrDeclaration {
    Function(IrFunction),
    Class(IrClass),
    Property(IrProperty),
    Field(IrField),
}

impl IrDeclaration {
    pub fn name(&self) -> &str {
        match self {
            IrDeclaration::Function(f) => &f.name,
            IrDeclaration::Class(c) => &c.name,
            IrDeclaration::Property(p) => &p.name,
            IrDeclaration::Field(f) => &f.name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub is_suspend: bool,
    pub is_inline: bool,
    pub is_static: bool,
    pub visibility: IrVisibility,
    pub receiver_type: Option<ConeKotlinType>,
    pub value_params: Vec<IrValueParam>,
    pub return_type: ConeKotlinType,
    pub body: Option<IrBody>,
    pub origin: IrDeclarationOrigin,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IrClass {
    pub name: String,
    pub kind: IrClassKind,
    pub super_types: Vec<ConeKotlinType>,
    pub primary_constructor_params: Option<Vec<IrValueParam>>,
    pub declarations: Vec<IrDeclaration>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IrProperty {
    pub name: String,
    pub prop_type: ConeKotlinType,
    pub is_val: bool,
    pub initializer: Option<IrExpression>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IrField {
    pub name: String,
    pub field_type: ConeKotlinType,
    pub is_final: bool,
    pub is_static: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IrValueParam {
    pub name: String,
    pub param_type: ConeKotlinType,
    pub default_value: Option<IrExpression>,
    pub index: u32,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IrBody {
    pub statements: Vec<IrStatement>,
}

#[derive(Debug, Clone)]
pub enum IrStatement {
    Declaration(IrDeclaration),
    Assignment {
        target: IrExpression,
        value: IrExpression,
        span: Span,
    },
    Expression(IrExpression),
}

#[derive(Debug, Clone)]
pub struct IrBranch {
    pub conditions: Vec<IrBranchCondition>,
    pub body: IrExpression,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum IrBranchCondition {
    Expression(IrExpression),
    TypeCheck {
        target_type: ConeKotlinType,
        is_negated: bool,
    },
    Else,
}

#[derive(Debug, Clone)]
pub enum IrExpression {
    ConstInt(i64, ConeKotlinType, Span),
    ConstFloat(f64, ConeKotlinType, Span),
    ConstString(String, Span),
    ConstBoolean(bool, Span),
    ConstNull(Span),
    GetLocal(String, ConeKotlinType, Span),
    SetLocal(String, Box<IrExpression>, Span),
    GetField {
        receiver: Option<Box<IrExpression>>,
        class_name: String,
        field_name: String,
        field_type: ConeKotlinType,
        span: Span,
    },
    SetField {
        receiver: Option<Box<IrExpression>>,
        class_name: String,
        field_name: String,
        value: Box<IrExpression>,
        span: Span,
    },
    Call {
        callee: String,
        receiver: Option<Box<IrExpression>>,
        args: Vec<IrExpression>,
        return_type: ConeKotlinType,
        is_inline: bool,
        span: Span,
    },
    ConstructorCall {
        class_name: String,
        args: Vec<IrExpression>,
        return_type: ConeKotlinType,
        span: Span,
    },
    Block {
        statements: Vec<IrStatement>,
        result: Option<Box<IrExpression>>,
        span: Span,
    },
    When {
        subject: Option<Box<IrExpression>>,
        branches: Vec<IrBranch>,
        span: Span,
    },
    For {
        variable: String,
        iterable: Box<IrExpression>,
        body: Box<IrExpression>,
        span: Span,
    },
    While {
        condition: Box<IrExpression>,
        body: Box<IrExpression>,
        span: Span,
    },
    Return {
        value: Option<Box<IrExpression>>,
        span: Span,
    },
    TypeOp {
        op: IrTypeOpKind,
        expr: Box<IrExpression>,
        target_type: ConeKotlinType,
        span: Span,
    },
    StringTemplate(Vec<IrExpression>, Span),
    Lambda {
        params: Vec<String>,
        body: Vec<IrStatement>,
        return_type: ConeKotlinType,
        span: Span,
    },
    SuspendCall {
        callee: String,
        args: Vec<IrExpression>,
        return_type: ConeKotlinType,
        suspension_point: u32,
        span: Span,
    },
    StateSwitch {
        label_expr: Box<IrExpression>,
        cases: Vec<(i32, Vec<IrStatement>)>,
        span: Span,
    },
}

impl IrExpression {
    pub fn span(&self) -> Span {
        match self {
            IrExpression::ConstInt(_, _, s)
            | IrExpression::ConstFloat(_, _, s)
            | IrExpression::ConstString(_, s)
            | IrExpression::ConstBoolean(_, s)
            | IrExpression::ConstNull(s)
            | IrExpression::GetLocal(_, _, s)
            | IrExpression::SetLocal(_, _, s)
            | IrExpression::GetField { span: s, .. }
            | IrExpression::SetField { span: s, .. }
            | IrExpression::Call { span: s, .. }
            | IrExpression::ConstructorCall { span: s, .. }
            | IrExpression::Block { span: s, .. }
            | IrExpression::When { span: s, .. }
            | IrExpression::For { span: s, .. }
            | IrExpression::While { span: s, .. }
            | IrExpression::Return { span: s, .. }
            | IrExpression::TypeOp { span: s, .. }
            | IrExpression::StringTemplate(_, s)
            | IrExpression::Lambda { span: s, .. }
            | IrExpression::SuspendCall { span: s, .. }
            | IrExpression::StateSwitch { span: s, .. } => *s,
        }
    }
}
