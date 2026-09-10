#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Number(String),
    String(String),
    Symbol(char),
    Arrow,
    Range,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

pub fn lex(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut column) = (0, 1, 1);
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            i += 1;
            line += 1;
            column = 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            column += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                column += 1;
            }
            continue;
        }
        if c == '"' {
            let start = column;
            i += 1;
            column += 1;
            let mut value = String::new();
            let mut closed = false;
            while i < chars.len() {
                match chars[i] {
                    '"' => {
                        i += 1;
                        column += 1;
                        closed = true;
                        break;
                    }
                    '\\' => {
                        let escaped = chars
                            .get(i + 1)
                            .ok_or_else(|| format!("{line}:{column}: unfinished string escape"))?;
                        value.push(match escaped {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '"' => '"',
                            '\\' => '\\',
                            other => {
                                return Err(format!(
                                    "{line}:{column}: unsupported string escape \\{other}"
                                ));
                            }
                        });
                        i += 2;
                        column += 2;
                    }
                    '\n' => return Err(format!("{line}:{column}: newline in string literal")),
                    other => {
                        value.push(other);
                        i += 1;
                        column += 1;
                    }
                }
            }
            if !closed {
                return Err(format!("{line}:{start}: unterminated string literal"));
            }
            out.push(Token {
                kind: TokenKind::String(value),
                line,
                column: start,
            });
            continue;
        }
        let start = column;
        if c == '-' && chars.get(i + 1) == Some(&'>') {
            out.push(Token {
                kind: TokenKind::Arrow,
                line,
                column,
            });
            i += 2;
            column += 2;
            continue;
        }
        if c == '.' && chars.get(i + 1) == Some(&'.') {
            out.push(Token {
                kind: TokenKind::Range,
                line,
                column,
            });
            i += 2;
            column += 2;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
        {
            let begin = i;
            let mut saw_dot = c == '.';
            i += 1;
            column += 1;
            while i < chars.len() {
                let n = chars[i];
                if n.is_ascii_digit() || n == '_' {
                    i += 1;
                    column += 1;
                } else if n == '.' && !saw_dot && chars.get(i + 1) != Some(&'.') {
                    saw_dot = true;
                    i += 1;
                    column += 1;
                } else if matches!(n, 'e' | 'E') {
                    i += 1;
                    column += 1;
                    if chars.get(i).is_some_and(|x| matches!(x, '+' | '-')) {
                        i += 1;
                        column += 1;
                    }
                } else if n.is_ascii_alphanumeric() {
                    i += 1;
                    column += 1;
                } else {
                    break;
                }
            }
            out.push(Token {
                kind: TokenKind::Number(chars[begin..i].iter().collect()),
                line,
                column: start,
            });
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' || c == '@' {
            let begin = i;
            i += 1;
            column += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '_' | '.' | '@'))
            {
                i += 1;
                column += 1;
            }
            out.push(Token {
                kind: TokenKind::Ident(chars[begin..i].iter().collect()),
                line,
                column: start,
            });
            continue;
        }
        if "(){}[],:;=+-*/^<>".contains(c) {
            out.push(Token {
                kind: TokenKind::Symbol(c),
                line,
                column,
            });
            i += 1;
            column += 1;
            continue;
        }
        return Err(format!("{}:{}: unexpected character {:?}", line, column, c));
    }
    out.push(Token {
        kind: TokenKind::Eof,
        line,
        column,
    });
    Ok(out)
}
