use crate::ast::*;
use crate::lexer::{Token, TokenKind};

pub fn parse(tokens: Vec<Token>) -> Result<Program, String> {
    Parser { tokens, at: 0 }.program()
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn program(&mut self) -> Result<Program, String> {
        let mut functions = Vec::new();
        while !matches!(self.peek().kind, TokenKind::Eof) {
            functions.push(self.function()?);
        }
        Ok(Program { functions })
    }

    fn function(&mut self) -> Result<Function, String> {
        let line = self.peek().line;
        self.keyword("fn")?;
        let id = self.ident()?;
        if !id.starts_with('@') {
            return self.err("function identity must start with '@'");
        }
        self.symbol('(')?;
        let mut params = Vec::new();
        if !self.check_symbol(')') {
            loop {
                let name = self.ident()?;
                self.symbol(':')?;
                let ty = self.ty()?;
                params.push(Param { name, ty });
                if self.take_symbol(',') {
                    continue;
                }
                break;
            }
        }
        self.symbol(')')?;
        self.arrow()?;
        let result = self.ty()?;
        let body = self.block()?;
        Ok(Function {
            id,
            params,
            result,
            body,
            line,
        })
    }

    fn ty(&mut self) -> Result<TypeSyntax, String> {
        let mut mutable = false;
        if self.check_ident("mut") {
            self.at += 1;
            mutable = true;
        }
        let name = self.ident()?;
        if name == "none" {
            return Ok(TypeSyntax::None);
        }
        if name == "Array" {
            self.symbol('<')?;
            let element_name = self.ident()?;
            let element = Scalar::parse(&element_name)
                .ok_or_else(|| self.message("unsupported array element type"))?;
            self.symbol(',')?;
            let rank_text = self.number()?;
            let rank = rank_text
                .parse()
                .map_err(|_| self.message("array rank must be an integer"))?;
            self.symbol('>')?;
            let unit = self.optional_unit()?;
            return Ok(TypeSyntax::Array {
                element,
                rank,
                unit,
                mutable,
            });
        }
        if mutable {
            return self.err("'mut' is only valid before Array in v0.1");
        }
        let scalar =
            Scalar::parse(&name).ok_or_else(|| self.message(&format!("unknown type '{name}'")))?;
        Ok(TypeSyntax::Scalar(scalar, self.optional_unit()?))
    }

    fn optional_unit(&mut self) -> Result<Option<String>, String> {
        if !self.take_symbol('[') {
            return Ok(None);
        }
        let mut text = String::new();
        let mut depth = 1;
        while depth > 0 {
            match self.next().kind.clone() {
                TokenKind::Ident(s) | TokenKind::Number(s) => text.push_str(&s),
                TokenKind::Symbol('[') => {
                    depth += 1;
                    text.push('[');
                }
                TokenKind::Symbol(']') => {
                    depth -= 1;
                    if depth > 0 {
                        text.push(']');
                    }
                }
                TokenKind::Symbol(c) if matches!(c, '*' | '/' | '^' | '-') => text.push(c),
                _ => return self.err("invalid unit expression"),
            }
        }
        Ok(Some(text))
    }

    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.symbol('{')?;
        let mut body = Vec::new();
        while !self.take_symbol('}') {
            body.push(self.statement()?);
        }
        Ok(body)
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        if self.check_ident("let") {
            self.at += 1;
            let mutable = if self.check_ident("mut") {
                self.at += 1;
                true
            } else {
                false
            };
            let name = self.ident()?;
            let annotation = if self.take_symbol(':') {
                Some(self.ty()?)
            } else {
                None
            };
            self.symbol('=')?;
            let value = self.expr()?;
            self.symbol(';')?;
            return Ok(Stmt::Let {
                name,
                mutable,
                annotation,
                value,
            });
        }
        if self.check_ident("for") {
            self.at += 1;
            let index = self.ident()?;
            self.keyword("in")?;
            let start = self.expr()?;
            self.range()?;
            let end = self.expr()?;
            return Ok(Stmt::For {
                index,
                start,
                end,
                body: self.block()?,
            });
        }
        if self.check_ident("return") {
            self.at += 1;
            if self.take_symbol(';') {
                return Ok(Stmt::Return(None));
            }
            let value = self.expr()?;
            self.symbol(';')?;
            return Ok(Stmt::Return(Some(value)));
        }
        if self.check_ident("print") {
            self.at += 1;
            self.symbol('(')?;
            let value = self.expr()?;
            let unit = if matches!(self.peek().kind, TokenKind::Arrow) {
                self.at += 1;
                Some(self.unit_until(')')?)
            } else {
                None
            };
            self.symbol(')')?;
            self.symbol(';')?;
            return Ok(Stmt::Print { value, unit });
        }
        let expr = self.expr()?;
        if self.take_symbol('=') {
            let value = self.expr()?;
            self.symbol(';')?;
            Ok(Stmt::Assign {
                target: expr,
                value,
            })
        } else {
            self.symbol(';')?;
            Ok(Stmt::Expr(expr))
        }
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.add()
    }
    fn add(&mut self) -> Result<Expr, String> {
        let mut left = self.mul()?;
        while let TokenKind::Symbol(op @ ('+' | '-')) = self.peek().kind {
            self.at += 1;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(self.mul()?),
            };
        }
        Ok(left)
    }
    fn mul(&mut self) -> Result<Expr, String> {
        let mut left = self.unary()?;
        while let TokenKind::Symbol(op @ ('*' | '/')) = self.peek().kind {
            self.at += 1;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(self.unary()?),
            };
        }
        Ok(left)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if let TokenKind::Symbol(op @ ('+' | '-')) = self.peek().kind {
            self.at += 1;
            return Ok(Expr::Unary {
                op,
                value: Box::new(self.unary()?),
            });
        }
        self.postfix()
    }
    fn postfix(&mut self) -> Result<Expr, String> {
        let mut value = self.primary()?;
        loop {
            if self.take_symbol('(') {
                let Expr::Var(function) = value else {
                    return self.err("only named functions can be called");
                };
                let mut args = Vec::new();
                if !self.check_symbol(')') {
                    loop {
                        args.push(self.expr()?);
                        if self.take_symbol(',') {
                            continue;
                        }
                        break;
                    }
                }
                self.symbol(')')?;
                value = Expr::Call { function, args };
            } else if self.take_symbol('[') {
                let index = self.expr()?;
                self.symbol(']')?;
                value = Expr::Index {
                    array: Box::new(value),
                    index: Box::new(index),
                };
            } else {
                break;
            }
        }
        Ok(value)
    }
    fn primary(&mut self) -> Result<Expr, String> {
        match self.next().kind.clone() {
            TokenKind::String(value) => Ok(Expr::String(value)),
            TokenKind::Number(text) => {
                let (raw, scalar) = split_number(&text).ok_or_else(|| {
                    self.message("numeric literals require a suffix such as f64 or i32")
                })?;
                let unit = if matches!(&self.peek().kind, TokenKind::Ident(s) if !is_keyword(s) && !s.starts_with('@'))
                {
                    Some(self.unit_atom_product())
                } else {
                    None
                };
                Ok(Expr::Number {
                    text: raw,
                    scalar,
                    unit,
                })
            }
            TokenKind::Ident(name) => Ok(Expr::Var(name)),
            TokenKind::Symbol('(') => {
                let e = self.expr()?;
                self.symbol(')')?;
                Ok(e)
            }
            TokenKind::Symbol('[') => {
                let mut values = Vec::new();
                if !self.check_symbol(']') {
                    loop {
                        values.push(self.expr()?);
                        if self.take_symbol(',') {
                            continue;
                        }
                        break;
                    }
                }
                self.symbol(']')?;
                Ok(Expr::Array(values))
            }
            _ => self.err("expected expression"),
        }
    }

    fn unit_atom_product(&mut self) -> String {
        let mut out = self.ident().expect("checked identifier");
        while matches!(self.peek().kind, TokenKind::Symbol('*' | '/' | '^')) {
            if let TokenKind::Symbol(c) = self.next().kind {
                out.push(c);
            }
            match self.next().kind.clone() {
                TokenKind::Ident(s) | TokenKind::Number(s) => out.push_str(&s),
                TokenKind::Symbol('-') => {
                    out.push('-');
                    if let TokenKind::Number(s) = self.next().kind.clone() {
                        out.push_str(&s);
                    }
                }
                _ => break,
            }
        }
        out
    }
    fn unit_until(&mut self, stop: char) -> Result<String, String> {
        let mut out = String::new();
        while !self.check_symbol(stop) {
            match self.next().kind.clone() {
                TokenKind::Ident(s) | TokenKind::Number(s) => out.push_str(&s),
                TokenKind::Symbol(c) if matches!(c, '*' | '/' | '^' | '-') => out.push(c),
                _ => return self.err("invalid conversion unit"),
            }
        }
        Ok(out)
    }
    fn ident(&mut self) -> Result<String, String> {
        match self.next().kind.clone() {
            TokenKind::Ident(s) => Ok(s),
            _ => self.err("expected identifier"),
        }
    }
    fn number(&mut self) -> Result<String, String> {
        match self.next().kind.clone() {
            TokenKind::Number(s) => Ok(s),
            _ => self.err("expected number"),
        }
    }
    fn keyword(&mut self, word: &str) -> Result<(), String> {
        if self.check_ident(word) {
            self.at += 1;
            Ok(())
        } else {
            self.err(&format!("expected '{word}'"))
        }
    }
    fn symbol(&mut self, c: char) -> Result<(), String> {
        if self.take_symbol(c) {
            Ok(())
        } else {
            self.err(&format!("expected '{c}'"))
        }
    }
    fn arrow(&mut self) -> Result<(), String> {
        if matches!(self.peek().kind, TokenKind::Arrow) {
            self.at += 1;
            Ok(())
        } else {
            self.err("expected '->'")
        }
    }
    fn range(&mut self) -> Result<(), String> {
        if matches!(self.peek().kind, TokenKind::Range) {
            self.at += 1;
            Ok(())
        } else {
            self.err("expected '..'")
        }
    }
    fn check_ident(&self, s: &str) -> bool {
        matches!(&self.peek().kind, TokenKind::Ident(x) if x == s)
    }
    fn check_symbol(&self, c: char) -> bool {
        matches!(self.peek().kind, TokenKind::Symbol(x) if x == c)
    }
    fn take_symbol(&mut self, c: char) -> bool {
        if self.check_symbol(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.at]
    }
    fn next(&mut self) -> &Token {
        let at = self.at;
        self.at += 1;
        &self.tokens[at]
    }
    fn message(&self, msg: &str) -> String {
        format!("{}:{}: {msg}", self.peek().line, self.peek().column)
    }
    fn err<T>(&self, msg: &str) -> Result<T, String> {
        Err(self.message(msg))
    }
}

fn split_number(text: &str) -> Option<(String, Scalar)> {
    for (suffix, ty) in [
        ("f64", Scalar::F64),
        ("f32", Scalar::F32),
        ("i64", Scalar::I64),
        ("i32", Scalar::I32),
    ] {
        if let Some(raw) = text.strip_suffix(suffix) {
            return Some((raw.replace('_', ""), ty));
        }
    }
    None
}

fn is_keyword(s: &str) -> bool {
    matches!(s, "in" | "let" | "mut" | "for" | "return" | "print" | "fn")
}
