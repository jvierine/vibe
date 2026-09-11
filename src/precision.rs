//! Empirical precision experiments retain the algorithm and widen representations.
use crate::ast::*;

pub fn promote(mut program: Program) -> Result<Program, String> {
    for function in &mut program.functions {
        for param in &mut function.params {
            promote_type(&mut param.ty);
        }
        promote_type(&mut function.result);
        block(&mut function.body)?;
    }
    Ok(program)
}

fn scalar(s: &mut Scalar) {
    *s = match &*s {
        Scalar::F32 => Scalar::F64,
        Scalar::Complex64 => Scalar::Complex128,
        other => other.clone(),
    };
}
fn promote_type(t: &mut TypeSyntax) {
    match t {
        TypeSyntax::Scalar(s, _) | TypeSyntax::Array { element: s, .. } => scalar(s),
        TypeSyntax::None => {}
    }
}
fn expression(e: &mut Expr) -> Result<(), String> {
    match e {
        Expr::Complex { .. } => return Err("precision promotion of complex literals is not implemented; supply complex data through typed array parameters".into()),
        Expr::Number { scalar: s, .. } => scalar(s),
        Expr::Array(values) | Expr::Call { args: values, .. } => { for v in values { expression(v)?; } }
        Expr::Index { array: left, index: right } | Expr::Binary { left, right, .. } => { expression(left)?; expression(right)?; }
        Expr::Unary { value, .. } => expression(value)?,
        Expr::Var(_) | Expr::String(_) => {}
    }
    Ok(())
}
fn block(body: &mut [Stmt]) -> Result<(), String> {
    for stmt in body {
        match stmt {
            Stmt::Let {
                annotation, value, ..
            } => {
                if let Some(t) = annotation {
                    promote_type(t);
                }
                expression(value)?;
            }
            Stmt::Assign { target, value } => {
                expression(target)?;
                expression(value)?;
            }
            Stmt::For {
                start, end, body, ..
            } => {
                expression(start)?;
                expression(end)?;
                block(body)?;
            }
            Stmt::Return(Some(e)) | Stmt::Expr(e) | Stmt::Print { value: e, .. } => expression(e)?,
            Stmt::Return(None) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn promotion_preserves_identity_and_changes_complex_arithmetic() {
        let function = Function {
            id: "@test_product".into(),
            line: 0,
            params: vec![Param {
                name: "z".into(),
                ty: TypeSyntax::Scalar(Scalar::Complex64, None),
            }],
            result: TypeSyntax::Scalar(Scalar::Complex64, None),
            body: vec![Stmt::Return(Some(Expr::Binary {
                op: '*',
                left: Box::new(Expr::Var("z".into())),
                right: Box::new(Expr::Var("z".into())),
            }))],
        };
        let program = promote(Program {
            functions: vec![function],
        })
        .unwrap();
        assert_eq!(program.functions[0].id, "@test_product");
        assert_eq!(
            program.functions[0].result,
            TypeSyntax::Scalar(Scalar::Complex128, None)
        );
        let checked = crate::check::check(program).unwrap();
        assert!(
            crate::codegen::emit_c(&checked)
                .unwrap()
                .contains("double _Complex vibe_")
        );
    }

    #[test]
    fn complex_and_real_cannot_mix_implicitly() {
        let f = Function {
            id: "@bad".into(),
            line: 0,
            params: vec![],
            result: TypeSyntax::Scalar(Scalar::Complex64, None),
            body: vec![Stmt::Return(Some(Expr::Binary {
                op: '+',
                left: Box::new(Expr::Complex {
                    real: "1".into(),
                    imag: "2".into(),
                }),
                right: Box::new(Expr::Number {
                    text: "1".into(),
                    scalar: Scalar::F32,
                    unit: None,
                }),
            }))],
        };
        assert!(crate::check::check(Program { functions: vec![f] }).is_err());
    }
}
