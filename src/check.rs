use crate::ast::*;
use crate::units::{self, Unit};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug)]
pub enum CheckedType {
    Scalar(Scalar, Unit),
    Array(Scalar, usize, Unit, bool),
    None,
    Error,
}

impl CheckedType {
    fn describe(&self) -> String {
        match self {
            Self::Scalar(s, u) => format!("{}[{}]", s.name(), u.display),
            Self::Array(s, r, u, _) => format!("Array<{},{}>[{}]", s.name(), r, u.display),
            Self::None => "none".into(),
            Self::Error => "<error>".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FunctionInfo {
    pub params: Vec<CheckedType>,
    pub result: CheckedType,
}

#[derive(Clone, Debug)]
pub struct CheckedProgram {
    pub program: Program,
    pub functions: BTreeMap<String, FunctionInfo>,
    pub calls: BTreeMap<String, Vec<String>>,
    pub effects: BTreeMap<String, BTreeSet<String>>,
}

pub fn check(program: Program) -> Result<CheckedProgram, Vec<String>> {
    let mut errors = Vec::new();
    let mut functions = BTreeMap::new();
    for f in &program.functions {
        if functions.contains_key(&f.id) {
            errors.push(format!("line {}: duplicate identity {}", f.line, f.id));
            continue;
        }
        let params = f
            .params
            .iter()
            .map(|p| checked_type(&p.ty))
            .collect::<Result<Vec<_>, _>>();
        let mut parameter_names = BTreeSet::new();
        for parameter in &f.params {
            if !parameter_names.insert(&parameter.name) {
                errors.push(format!(
                    "line {}: duplicate parameter '{}' in {}",
                    f.line, parameter.name, f.id
                ));
            }
        }
        let result = checked_type(&f.result);
        match (params, result) {
            (Ok(params), Ok(result)) => {
                functions.insert(f.id.clone(), FunctionInfo { params, result });
            }
            (Err(e), _) | (_, Err(e)) => errors.push(format!("line {}: {e}", f.line)),
        }
    }
    let mut calls = BTreeMap::new();
    if errors.is_empty() {
        for f in &program.functions {
            let mut env = HashMap::new();
            for (p, ty) in f.params.iter().zip(&functions[&f.id].params) {
                env.insert(
                    p.name.clone(),
                    (ty.clone(), matches!(ty, CheckedType::Array(_, _, _, true))),
                );
            }
            let mut found_calls = Vec::new();
            check_block(
                &f.body,
                &mut env,
                &functions,
                &functions[&f.id].result,
                &mut found_calls,
                &mut errors,
            );
            found_calls.sort();
            found_calls.dedup();
            calls.insert(f.id.clone(), found_calls);
            if !matches!(f.body.last(), Some(Stmt::Return(_))) {
                errors.push(format!(
                    "line {}: function {} must end with an explicit return",
                    f.line, f.id
                ));
            }
        }
    }
    if let Some(main) = functions.get("@app.main") {
        let valid_result = matches!(
            &main.result,
            CheckedType::Scalar(Scalar::I32, unit) if unit.dimensions.is_empty()
        );
        if !main.params.is_empty() || !valid_result {
            errors.push("@app.main must have signature () -> i32".into());
        }
    }
    if errors.is_empty() {
        let effects = infer_effects(&program, &calls);
        Ok(CheckedProgram {
            program,
            functions,
            calls,
            effects,
        })
    } else {
        Err(errors)
    }
}

fn infer_effects(
    program: &Program,
    calls: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut effects: BTreeMap<String, BTreeSet<String>> = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), direct_effects(&function.body)))
        .collect();
    loop {
        let previous = effects.clone();
        for (caller, callees) in calls {
            for callee in callees {
                if let Some(callee_effects) = previous.get(callee) {
                    effects
                        .entry(caller.clone())
                        .or_default()
                        .extend(callee_effects.iter().cloned());
                }
            }
        }
        if effects == previous {
            return effects;
        }
    }
}

fn direct_effects(body: &[Stmt]) -> BTreeSet<String> {
    let mut effects = BTreeSet::new();
    for statement in body {
        match statement {
            Stmt::Print { .. } => {
                effects.insert("io.stdout".into());
            }
            Stmt::For { body, .. } => effects.extend(direct_effects(body)),
            _ => {}
        }
    }
    effects
}

fn checked_type(ty: &TypeSyntax) -> Result<CheckedType, String> {
    Ok(match ty {
        TypeSyntax::Scalar(s, u) => CheckedType::Scalar(s.clone(), units::parse(u.as_deref())?),
        TypeSyntax::Array {
            element,
            rank,
            unit,
            mutable,
        } => CheckedType::Array(
            element.clone(),
            *rank,
            units::parse(unit.as_deref())?,
            *mutable,
        ),
        TypeSyntax::None => CheckedType::None,
    })
}

fn check_block(
    body: &[Stmt],
    env: &mut HashMap<String, (CheckedType, bool)>,
    functions: &BTreeMap<String, FunctionInfo>,
    result: &CheckedType,
    calls: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    for stmt in body {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                annotation,
                value,
            } => match expr_type(value, env, functions, calls) {
                Ok(inferred) => {
                    let actual = if let Some(a) = annotation {
                        match checked_type(a) {
                            Ok(a) => {
                                if let Err(e) = assignable(&a, &inferred) {
                                    errors.push(format!("binding {name}: {e}"));
                                }
                                a
                            }
                            Err(e) => {
                                errors.push(e);
                                inferred
                            }
                        }
                    } else {
                        inferred
                    };
                    if env.contains_key(name) {
                        errors.push(format!(
                            "binding '{name}' shadows an existing binding; shadowing is forbidden"
                        ));
                    } else {
                        env.insert(name.clone(), (actual, *mutable));
                    }
                }
                Err(e) => {
                    errors.push(format!("binding {name}: {e}"));
                    env.insert(name.clone(), (CheckedType::Error, *mutable));
                }
            },
            Stmt::Assign { target, value } => {
                let valid_target = matches!(target, Expr::Var(_))
                    || matches!(target, Expr::Index { array, .. } if matches!(array.as_ref(), Expr::Var(_)));
                if !valid_target {
                    errors.push(
                        "assignment target must be a mutable binding or its direct array element"
                            .into(),
                    );
                }
                let lhs = expr_type(target, env, functions, calls);
                let rhs = expr_type(value, env, functions, calls);
                if let Expr::Var(name) = target
                    && !env.get(name).is_some_and(|(_, m)| *m)
                {
                    errors.push(format!("cannot assign immutable binding '{name}'"));
                }
                if let Expr::Index { array, .. } = target
                    && let Expr::Var(name) = array.as_ref()
                    && !env.get(name).is_some_and(|(_, m)| *m)
                {
                    errors.push(format!("cannot assign through immutable array '{name}'"));
                }
                match (lhs, rhs) {
                    (Ok(a), Ok(b)) => {
                        if let Err(e) = assignable(&a, &b) {
                            errors.push(e);
                        }
                    }
                    (Err(e), _) | (_, Err(e)) => errors.push(e),
                }
            }
            Stmt::For {
                index,
                start,
                end,
                body,
            } => {
                if env.contains_key(index) {
                    errors.push(format!(
                        "loop index '{index}' shadows an existing binding; shadowing is forbidden"
                    ));
                }
                let start_type = expr_type(start, env, functions, calls);
                let end_type = expr_type(end, env, functions, calls);
                for bound in [&start_type, &end_type] {
                    match bound {
                        Ok(CheckedType::Scalar(Scalar::I64, unit))
                            if unit.dimensions.is_empty() => {}
                        Ok(CheckedType::Error) => {}
                        Ok(ty) => errors.push(format!(
                            "loop bounds must be dimensionless i64 values, got {}",
                            ty.describe()
                        )),
                        Err(error) => errors.push(error.clone()),
                    }
                }
                let mut loop_env = env.clone();
                loop_env.insert(
                    index.clone(),
                    (
                        CheckedType::Scalar(Scalar::I64, Unit::dimensionless()),
                        false,
                    ),
                );
                check_block(body, &mut loop_env, functions, result, calls, errors);
            }
            Stmt::Return(value) => {
                let got = match value {
                    Some(v) => expr_type(v, env, functions, calls),
                    None => Ok(CheckedType::None),
                };
                match got {
                    Ok(got) => {
                        if let Err(e) = assignable(result, &got) {
                            errors.push(format!("return: {e}"));
                        }
                    }
                    Err(e) => errors.push(e),
                }
            }
            Stmt::Print {
                value: Expr::String(_),
                unit,
            } => {
                if unit.is_some() {
                    errors.push("a string cannot have a display unit".into());
                }
            }
            Stmt::Print { value, unit } => match expr_type(value, env, functions, calls) {
                Ok(CheckedType::Scalar(Scalar::Complex64 | Scalar::Complex128, _)) => errors
                    .push("complex printing requires explicit real/imaginary projection".into()),
                Ok(CheckedType::Scalar(_, actual)) => {
                    if let Some(name) = unit {
                        match units::parse(Some(name)) {
                            Ok(wanted) if !wanted.compatible(&actual) => errors
                                .push(format!("cannot display [{}] as [{name}]", actual.display)),
                            Err(e) => errors.push(e),
                            _ => {}
                        }
                    }
                }
                Ok(CheckedType::Error) => {}
                Ok(t) => errors.push(format!(
                    "print supports scalars in v0.1, got {}",
                    t.describe()
                )),
                Err(e) => errors.push(e),
            },
            Stmt::Expr(e) => {
                if let Err(e) = expr_type(e, env, functions, calls) {
                    errors.push(e);
                }
            }
        }
    }
}

fn expr_type(
    expr: &Expr,
    env: &HashMap<String, (CheckedType, bool)>,
    functions: &BTreeMap<String, FunctionInfo>,
    calls: &mut Vec<String>,
) -> Result<CheckedType, String> {
    match expr {
        Expr::Complex { real, imag } => {
            validate_scaled_literal(real, &Scalar::F32, 1.0)?;
            validate_scaled_literal(imag, &Scalar::F32, 1.0)?;
            Ok(CheckedType::Scalar(
                Scalar::Complex64,
                Unit::dimensionless(),
            ))
        }
        Expr::String(_) => Err("strings are only supported directly inside print in v0.1".into()),
        Expr::Number { text, scalar, unit } => {
            let unit = units::parse(unit.as_deref())?;
            validate_scaled_literal(text, scalar, unit.scale)?;
            Ok(CheckedType::Scalar(scalar.clone(), unit))
        }
        Expr::Var(name) => env
            .get(name)
            .map(|x| x.0.clone())
            .ok_or_else(|| format!("unknown binding '{name}'")),
        Expr::Array(values) => {
            let first = values
                .first()
                .ok_or("empty array literals need an explicit type")?;
            let t = expr_type(first, env, functions, calls)?;
            let CheckedType::Scalar(s, u) = t else {
                return Err("nested array literals are not supported in v0.1".into());
            };
            for v in &values[1..] {
                assignable(
                    &CheckedType::Scalar(s.clone(), u.clone()),
                    &expr_type(v, env, functions, calls)?,
                )?;
            }
            Ok(CheckedType::Array(s, 1, u, true))
        }
        Expr::Index { array, index } => {
            match expr_type(index, env, functions, calls)? {
                CheckedType::Scalar(Scalar::I32 | Scalar::I64, u) if u.dimensions.is_empty() => {}
                CheckedType::Error => return Ok(CheckedType::Error),
                t => {
                    return Err(format!(
                        "array index must be dimensionless integer, got {}",
                        t.describe()
                    ));
                }
            }
            match expr_type(array, env, functions, calls)? {
                CheckedType::Array(s, 1, u, _) => Ok(CheckedType::Scalar(s, u)),
                CheckedType::Array(_, rank, _, _) => {
                    Err(format!("rank-{rank} indexing is not implemented yet"))
                }
                CheckedType::Error => Ok(CheckedType::Error),
                t => Err(format!("cannot index {}", t.describe())),
            }
        }
        Expr::Call { function, args } if function == "len" => {
            if args.len() != 1 {
                return Err("len expects one argument".into());
            }
            match expr_type(&args[0], env, functions, calls)? {
                CheckedType::Array(..) => {
                    Ok(CheckedType::Scalar(Scalar::I64, Unit::dimensionless()))
                }
                CheckedType::Error => Ok(CheckedType::Error),
                t => Err(format!("len expects an array, got {}", t.describe())),
            }
        }
        Expr::Call { function, args } => {
            let info = functions
                .get(function)
                .ok_or_else(|| format!("unknown function '{function}'"))?;
            if args.len() != info.params.len() {
                return Err(format!(
                    "{function} expects {} arguments, got {}",
                    info.params.len(),
                    args.len()
                ));
            }
            for (i, (arg, expected)) in args.iter().zip(&info.params).enumerate() {
                assignable(expected, &expr_type(arg, env, functions, calls)?)
                    .map_err(|e| format!("argument {} to {function}: {e}", i + 1))?;
                if matches!(expected, CheckedType::Array(_, _, _, true)) {
                    let mutable = match arg {
                        Expr::Var(name) => env.get(name).is_some_and(|(_, mutable)| *mutable),
                        _ => false,
                    };
                    if !mutable {
                        return Err(format!(
                            "argument {} to {function} must be a mutable array binding",
                            i + 1
                        ));
                    }
                }
            }
            calls.push(function.clone());
            Ok(info.result.clone())
        }
        Expr::Unary { value, .. } => expr_type(value, env, functions, calls),
        Expr::Binary { op, left, right } => {
            let (a, b) = (
                expr_type(left, env, functions, calls)?,
                expr_type(right, env, functions, calls)?,
            );
            match (a, b) {
                (CheckedType::Error, _) | (_, CheckedType::Error) => Ok(CheckedType::Error),
                (CheckedType::Scalar(sa, ua), CheckedType::Scalar(sb, ub)) if sa == sb => {
                    match op {
                        '+' | '-' if ua.compatible(&ub) => Ok(CheckedType::Scalar(sa, ua)),
                        '+' | '-' => Err(format!(
                            "unit mismatch: cannot {op} [{}] and [{}]",
                            ua.display, ub.display
                        )),
                        '*' => Ok(CheckedType::Scalar(sa, ua.mul(&ub))),
                        '/' => Ok(CheckedType::Scalar(sa, ua.div(&ub))),
                        _ => unreachable!(),
                    }
                }
                (a, b) => Err(format!(
                    "operator {op} requires equal scalar representations, got {} and {}",
                    a.describe(),
                    b.describe()
                )),
            }
        }
    }
}

fn validate_scaled_literal(text: &str, scalar: &Scalar, scale: f64) -> Result<(), String> {
    let valid = match scalar {
        Scalar::F32 => text
            .parse::<f32>()
            .is_ok_and(|value| (value * scale as f32).is_finite()),
        Scalar::F64 => text
            .parse::<f64>()
            .is_ok_and(|value| (value * scale).is_finite()),
        Scalar::I32 => text.parse::<i32>().is_ok_and(|value| {
            let scaled = value as f64 * scale;
            scaled.fract() == 0.0 && scaled >= i32::MIN as f64 && scaled <= i32::MAX as f64
        }),
        Scalar::I64 => text.parse::<i64>().is_ok_and(|value| {
            if scale == 1.0 {
                true
            } else {
                let scaled = value as f64 * scale;
                scaled.fract() == 0.0 && scaled >= i64::MIN as f64 && scaled <= i64::MAX as f64
            }
        }),
        Scalar::Bool | Scalar::Complex64 | Scalar::Complex128 => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "literal {text} is not representable after unit scaling as {}",
            scalar.name()
        ))
    }
}

fn assignable(expected: &CheckedType, actual: &CheckedType) -> Result<(), String> {
    let ok = match (expected, actual) {
        (CheckedType::Error, _) | (_, CheckedType::Error) => true,
        (CheckedType::None, CheckedType::None) => true,
        (CheckedType::Scalar(a, ua), CheckedType::Scalar(b, ub)) => a == b && ua.compatible(ub),
        (CheckedType::Array(a, ra, ua, _), CheckedType::Array(b, rb, ub, _)) => {
            a == b && ra == rb && ua.compatible(ub)
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "expected {}, got {}",
            expected.describe(),
            actual.describe()
        ))
    }
}
