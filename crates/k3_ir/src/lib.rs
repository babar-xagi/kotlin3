use k3_fir::ConeKotlinType;
use k3_syntax::Span;

#[derive(Debug, Clone)]
pub enum IrElement {
    Module(IrModule),
    File(IrFile),
    Declaration(IrDeclaration),
    Statement(IrStatement),
    Expression(IrExpression),
}

#[derive(Debug, Clone)]
pub struct IrModule {
    pub name: String,
    pub files: Vec<IrFile>,
}

#[derive(Debug, Clone)]
pub struct IrFile {
    pub file_path: String,
    pub package_name: String,
    pub declarations: Vec<IrDeclaration>,
}

#[derive(Debug, Clone)]
pub enum IrDeclaration {
    Function {
        name: String,
        return_type: ConeKotlinType,
        span: Span,
    },
    Class {
        name: String,
        is_interface: bool,
        declarations: Vec<IrDeclaration>,
        span: Span,
    },
    Property {
        name: String,
        prop_type: ConeKotlinType,
        is_var: bool,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum IrStatement {
    Expression(IrExpression),
    Return(Option<IrExpression>),
}

#[derive(Debug, Clone)]
pub enum IrExpression {
    ConstInt(i64),
    ConstString(String),
    ConstBoolean(bool),
    Call {
        callee: String,
        arguments: Vec<IrExpression>,
        type_ref: ConeKotlinType,
    },
}
