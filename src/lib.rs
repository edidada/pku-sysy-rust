pub mod ir;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Number(i32),
    Symbol(u8),
    Operator(String),
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
        } else if pos + 1 < bytes.len() && [b"<=".as_slice(), b">=", b"==", b"!=", b"&&", b"||"].contains(&&bytes[pos..pos+2]) {
            tokens.push(Token::Operator(source[pos..pos+2].to_owned())); pos += 2;
        } else if b"+-!*/%<>".contains(&bytes[pos]) {
            tokens.push(Token::Operator(source[pos..pos+1].to_owned())); pos += 1;
        } else if b"(){};".contains(&bytes[pos]) {
            tokens.push(Token::Symbol(bytes[pos]));
            pos += 1;
        } else {
            return Err(format!("unexpected character at byte {pos}"));
        }
    }
    Ok(tokens)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp { Plus, Minus, Not }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp { Mul, Div, Mod, Add, Sub, Lt, Gt, Le, Ge, Eq, Ne, And, Or }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Number(i32),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}
#[derive(Debug, PartialEq, Eq)]
pub struct Function { pub name: String, pub expression: Expr }

struct Parser { tokens: Vec<Token>, position: usize }
impl Parser {
    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.tokens.get(self.position) != Some(&token) {
            return Err(format!("expected {token:?} at token {}", self.position));
        }
        self.position += 1; Ok(())
    }
    fn unary(&mut self) -> Result<Expr, String> {
        let token=self.tokens.get(self.position).cloned();
        match token {
            Some(Token::Operator(op)) if matches!(op.as_str(),"+"|"-"|"!") => {
                self.position+=1;
                let kind=match op.as_str(){"+"=>UnaryOp::Plus,"-"=>UnaryOp::Minus,_=>UnaryOp::Not};
                Ok(Expr::Unary(kind,Box::new(self.unary()?)))
            }
            Some(Token::Number(value)) => {self.position+=1;Ok(Expr::Number(value))}
            Some(Token::Symbol(b'(')) => {
                self.position+=1;let value=self.expression(1)?;
                self.expect(Token::Symbol(b')'))?;Ok(value)
            }
            _=>Err(format!("expected expression at token {}",self.position)),
        }
    }
    fn expression(&mut self, minimum: u8) -> Result<Expr,String> {
        let mut lhs=self.unary()?;
        while let Some(Token::Operator(op))=self.tokens.get(self.position) {
            let (kind,priority)=match op.as_str() {
                "||"=>(BinaryOp::Or,1),"&&"=>(BinaryOp::And,2),
                "=="=>(BinaryOp::Eq,3),"!="=>(BinaryOp::Ne,3),
                "<"=>(BinaryOp::Lt,4),">"=>(BinaryOp::Gt,4),"<="=>(BinaryOp::Le,4),">="=>(BinaryOp::Ge,4),
                "+"=>(BinaryOp::Add,5),"-"=>(BinaryOp::Sub,5),
                "*"=>(BinaryOp::Mul,6),"/"=>(BinaryOp::Div,6),"%"=>(BinaryOp::Mod,6),_=>break,
            };
            if priority<minimum {break;}
            self.position+=1;
            let rhs=self.expression(priority+1)?;
            lhs=Expr::Binary(kind,Box::new(lhs),Box::new(rhs));
        }
        Ok(lhs)
    }
}
pub fn parse(source: &str) -> Result<Function, String> {
    let mut parser=Parser{tokens:lex(source)?,position:0};
    for token in [Token::Word("int".into()),Token::Word("main".into()),Token::Symbol(b'('),Token::Symbol(b')'),Token::Symbol(b'{'),Token::Word("return".into())] {
        parser.expect(token)?;
    }
    let expression=parser.expression(1)?;
    parser.expect(Token::Symbol(b';'))?;parser.expect(Token::Symbol(b'}'))?;
    if parser.position!=parser.tokens.len(){return Err("unexpected tokens after function".into());}
    Ok(Function{name:"main".into(),expression})
}
pub fn compile(source: &str, mode: &str) -> Result<String, String> {
    let program=ir::Program::from(parse(source)?);
    match mode {"-koopa"=>Ok(program.koopa()),"-riscv"|"-perf"=>Ok(program.riscv()),_=>Err(format!("unsupported mode: {mode}"))}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expr(s:&str)->Expr {parse(&format!("int main(){{return {s};}}")).unwrap().expression}
    #[test]
    fn bases_and_boundaries(){
        for n in 0..4096 {for literal in [n.to_string(),format!("0{n:o}"),format!("0x{n:x}")] {assert_eq!(expr(&literal),Expr::Number(n));}}
        assert_eq!(expr("2147483647"),Expr::Number(i32::MAX));
    }
    #[test]
    fn precedence_associativity_and_unary(){
        let n=|x|Box::new(Expr::Number(x));
        assert_eq!(expr("1+2*3"),Expr::Binary(BinaryOp::Add,n(1),Box::new(Expr::Binary(BinaryOp::Mul,n(2),n(3)))));
        assert_eq!(expr("9-4-2"),Expr::Binary(BinaryOp::Sub,Box::new(Expr::Binary(BinaryOp::Sub,n(9),n(4))),n(2)));
        assert_eq!(expr("! - +3"),Expr::Unary(UnaryOp::Not,Box::new(Expr::Unary(UnaryOp::Minus,Box::new(Expr::Unary(UnaryOp::Plus,n(3)))))));
        assert!(matches!(expr("1||2&&3==4<5+6*7"),Expr::Binary(BinaryOp::Or,_,_)));
    }
    #[test]
    fn comments_and_malformed_inputs(){
        assert_eq!(expr("/*a*/0x2a//comment\n"),Expr::Number(42));
        for s in ["", "/*", "/**", "int main(){return 08;}","int main(){return 0x;}","int main(){return 2147483648;}","int main(){return 1_;}","int main(){return 1a;}","int main(){return 1}","int main(){return 1;} garbage","int foo(){return 0;}","in/**/t main(){return 0;}","你好","int main(){return 1/**/2;}"] {assert!(parse(s).is_err(),"{s}");}
        for s in ["()","1+","*2","1&2","1|2","1=2","(1+2","1,2","2**3","1< =2"] {assert!(parse(&format!("int main(){{return {s};}}")).is_err(),"{s}");}
    }
    #[test]
    fn modes_share_ir(){
        assert_eq!(compile("int main(){return 052;}","-koopa").unwrap(),"fun @main(): i32 {\n%entry:\n  ret 42\n}\n");
        let ir=compile("int main(){return 1+2*3;}","-koopa").unwrap();
        assert!(ir.contains("%t0 = mul 2, 3"));assert!(ir.contains("%t1 = add 1, %t0"));
        assert!(compile("int main(){return 1;}","bad").is_err());
    }
}
