//! Narrow explicit f64 math boundary; no implicit promotion or dimensional erasure.
use crate::ast::Scalar;
pub struct Intrinsic {
    pub inputs: Vec<Scalar>,
    pub output: Scalar,
    pub c_name: &'static str,
}
pub fn lookup(id: &str) -> Option<Intrinsic> {
    let (arity, output, c_name) = match id {
        "@math.f64.sqrt" => (1, Scalar::F64, "sqrt"),
        "@math.f64.cbrt" => (1, Scalar::F64, "cbrt"),
        "@math.f64.exp" => (1, Scalar::F64, "exp"),
        "@math.f64.log" => (1, Scalar::F64, "log"),
        "@math.f64.sin" => (1, Scalar::F64, "sin"),
        "@math.f64.cos" => (1, Scalar::F64, "cos"),
        "@math.f64.atan" => (1, Scalar::F64, "atan"),
        "@math.f64.atan2" => (2, Scalar::F64, "atan2"),
        "@math.f64.abs" => (1, Scalar::F64, "fabs"),
        "@math.f64.pow" => (2, Scalar::F64, "pow"),
        "@math.f64.min" => (2, Scalar::F64, "fmin"),
        "@math.f64.max" => (2, Scalar::F64, "fmax"),
        "@math.f64.isfinite" => (1, Scalar::Bool, "isfinite"),
        "@cast.i64_from_f64" => (1, Scalar::I64, "vibe_i64_from_f64"),
        "@cast.f64_from_i64" => {
            return Some(Intrinsic {
                inputs: vec![Scalar::I64],
                output: Scalar::F64,
                c_name: "vibe_f64_from_i64",
            });
        }
        _ => return None,
    };
    Some(Intrinsic {
        inputs: vec![Scalar::F64; arity],
        output,
        c_name,
    })
}

#[cfg(test)]
mod tests {
    use crate::{check, lexer, parser};

    fn checked(source: &str) -> Result<check::CheckedProgram, Vec<String>> {
        check::check(parser::parse(lexer::lex(source).unwrap()).unwrap())
    }

    #[test]
    fn conditions_have_boolean_types_and_lexical_scope() {
        for source in [
            "fn @bad() -> none { while 1i64 { return; } return; }",
            "fn @bad() -> none { if 1f64 < 2f64 { let local = 3f64; } print(local); return; }",
            "fn @bad() -> none { if 1f64[m] < 2f64[s] { return; } return; }",
            "fn @bad() -> none { if 1f64 < 2f32 { return; } return; }",
        ] {
            assert!(checked(source).is_err(), "{source}");
        }
        let program=checked("fn @ok() -> i64 { let mut i = 0i64; while i < 3i64 { if i == 1i64 { print(i); } else { print(0i64); } i = i + 1i64; } return i; }").unwrap();
        assert!(program.effects["@ok"].contains("io.stdout"));
        let c = crate::codegen::emit_c(&program).unwrap();
        assert!(c.contains("while (") && c.contains("else {"));
    }

    #[test]
    fn math_does_not_erase_units_or_implicitly_widen() {
        for expr in [
            "@math.f64.sqrt(1f64[m])",
            "@math.f64.sqrt(1f32)",
            "@math.f64.pow(1f64)",
            "@cast.f64_from_i64(1f64)",
        ] {
            assert!(checked(&format!("fn @bad() -> f64 {{ return {expr}; }}")).is_err());
        }
        assert!(checked("fn @math.f64.sqrt(x:f64) -> f64 { return x; }").is_err());
        assert!(checked("fn @ok() -> f64 { return @math.f64.sqrt(4f64); }").is_ok());
    }

    #[test]
    fn native_control_flow_math_and_runtime_guards() {
        let root =
            std::env::temp_dir().join(format!("vibe-control-runtime-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = "fn @app.main() -> i32 { let mut i = 0i64; let mut x = 0f64; while i < 4i64 { x = x + @math.f64.sqrt(@cast.f64_from_i64(i)); i = i + 1i64; } if x > 4f64 { print(\"control works\"); } return 0i32; }";
        let program = checked(source).unwrap();
        crate::build(&program, &root.join("control")).unwrap();
        let result = std::process::Command::new(root.join("control"))
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap(), "control works\n");
        for (name, expression) in [
            ("negative", "a[-1i64]"),
            ("upper", "a[1i64]"),
            ("cast", "@cast.i64_from_f64(1e30f64)"),
        ] {
            let source = format!(
                "fn @app.main() -> i32 {{ let a = [2i64]; print(a[0i64]); print({expression}); return 0i32; }}"
            );
            crate::build(&checked(&source).unwrap(), &root.join(name)).unwrap();
            let result = std::process::Command::new(root.join(name))
                .output()
                .unwrap();
            assert!(!result.status.success());
            assert!(String::from_utf8(result.stderr).unwrap().contains("Vibe:"));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
