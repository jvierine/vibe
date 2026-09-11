use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scalar {
    Bool,
    I32,
    I64,
    F32,
    F64,
}

impl Scalar {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "bool" => Self::Bool,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "f32" => Self::F32,
            "f64" => Self::F64,
            _ => return None,
        })
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeSyntax {
    Scalar(Scalar, Option<String>),
    Array {
        element: Scalar,
        rank: usize,
        unit: Option<String>,
        mutable: bool,
    },
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Function {
    pub id: String,
    pub params: Vec<Param>,
    pub result: TypeSyntax,
    pub body: Vec<Stmt>,
    pub line: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: TypeSyntax,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Stmt {
    Let {
        name: String,
        mutable: bool,
        annotation: Option<TypeSyntax>,
        value: Expr,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    For {
        index: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    Print {
        value: Expr,
        unit: Option<String>,
    },
    Expr(Expr),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Expr {
    String(String),
    Number {
        text: String,
        scalar: Scalar,
        unit: Option<String>,
    },
    Var(String),
    Array(Vec<Expr>),
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
    },
    Call {
        function: String,
        args: Vec<Expr>,
    },
    Binary {
        op: char,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: char,
        value: Box<Expr>,
    },
}
