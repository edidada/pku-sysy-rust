#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Number(i32),
    Symbol(u8),
}

fn lex(source: &str) -> Result<Vec<Token>, String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
        } else if bytes[pos..].starts_with(b"//") {
            while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
        } else if bytes[pos..].starts_with(b"/*") {
            let start = pos;
            pos += 2;
            while pos + 1 < bytes.len() && &bytes[pos..pos + 2] != b"*/" { pos += 1; }
            if pos + 1 == bytes.len() || pos == bytes.len() {
                return Err(format!("unterminated block comment at byte {start}"));
            }
            pos += 2;
        } else if bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'_' {
            let start = pos;
            while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') { pos += 1; }
            tokens.push(Token::Word(source[start..pos].to_owned()));
        } else if bytes[pos].is_ascii_digit() {
            let start = pos;
            while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') { pos += 1; }
            let literal = &source[start..pos];
            let (radix, digits) = if literal.starts_with("0x") || literal.starts_with("0X") {
                (16, &literal[2..])
            } else if literal.starts_with('0') { (8, literal) } else { (10, literal) };
            let number = i32::from_str_radix(digits, radix)
                .map_err(|_| format!("invalid or out-of-range integer at byte {start}"))?;
            tokens.push(Token::Number(number));
        } else if b"(){};".contains(&bytes[pos]) {
            tokens.push(Token::Symbol(bytes[pos]));
            pos += 1;
        } else {
            return Err(format!("unexpected character at byte {pos}"));
        }
    }
    Ok(tokens)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub return_value: i32,
}

pub fn parse(source: &str) -> Result<Function, String> {
    let tokens = lex(source)?;
    let prefix = [Token::Word("int".into()), Token::Word("main".into()),
        Token::Symbol(b'('), Token::Symbol(b')'), Token::Symbol(b'{'), Token::Word("return".into())];
    if tokens.len() != 9 || tokens[..6] != prefix || tokens[7..] != [Token::Symbol(b';'), Token::Symbol(b'}')] {
        return Err("expected exactly int main() { return INT_CONST; }".into());
    }
    match tokens[6] {
        Token::Number(value) => Ok(Function { name: "main".into(), return_value: value }),
        _ => Err("expected integer return value".into()),
    }
}

pub fn compile(source: &str, mode: &str) -> Result<String, String> {
    let function = parse(source)?;
    let value = function.return_value;
    match mode {
        "-koopa" => Ok(format!("fun @main(): i32 {{\n%entry:\n  ret {value}\n}}\n")),
        "-riscv" | "-perf" => Ok(format!("  .text\n  .globl main\nmain:\n  li a0, {value}\n  ret\n")),
        _ => Err(format!("unsupported mode: {mode}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bases_and_boundaries() {
        for (literal, expected) in [("0",0),("00",0),("077",63),("0x7fffffff",i32::MAX),("2147483647",i32::MAX),("0XCAFE",51966)] {
            assert_eq!(parse(&format!("int main() {{ return {literal}; }}")).unwrap().return_value,expected);
        }
        for n in 0..4096 {
            for literal in [n.to_string(),format!("0{n:o}"),format!("0x{n:x}")] {
                assert_eq!(parse(&format!("int main(){{return {literal};}}")).unwrap().return_value,n);
            }
        }
    }
    #[test]
    fn comments_are_token_separators() {
        let source = "/*prefix*/ int/**/main(//comment\n) { return/**/0x2a; } //tail";
        assert_eq!(parse(source).unwrap().return_value,42);
        assert!(parse("in/**/t main(){return 0;}").is_err());
        assert!(parse("int main(){return 1/**/2;}").is_err());
    }
    #[test]
    fn malformed_input_is_rejected() {
        for source in ["", "/*", "/**", "int main(){return -1;}", "int main(){return 08;}",
            "int main(){return 0x;}","int main(){return 2147483648;}","int main(){return 0x80000000;}",
            "int main(){return 1_;}","int main(){return 1a;}","int main(){return 1}",
            "int main(){return 1;} garbage", "int foo(){return 0;}", "intmain(){return 0;}", "你好"] {
            assert!(parse(source).is_err(),"{source}");
        }
    }
    #[test]
    fn output_modes_use_ast_value() {
        assert_eq!(compile("int main(){return 052;}","-koopa").unwrap(),"fun @main(): i32 {\n%entry:\n  ret 42\n}\n");
        assert!(compile("int main(){return 42;}","-riscv").unwrap().contains("li a0, 42"));
        assert!(compile("int main(){return 0;}","bad").is_err());
    }
}
