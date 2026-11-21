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
        } else if b"(){};,=".contains(&bytes[pos]) {
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
    Variable(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}
#[derive(Debug, PartialEq, Eq)]
pub struct Function { pub name: String, pub items: Vec<BlockItem> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition { pub name: String, pub initializer: Option<Expr> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockItem {
    Declaration { constant: bool, definitions: Vec<Definition> },
    Assignment { name: String, value: Expr },
    Return(Expr),
}

struct Parser { tokens: Vec<Token>, position: usize }
impl Parser {
    fn expect(&mut self, token: Token) -> Result<(), String> {
        if self.tokens.get(self.position) != Some(&token) {
            return Err(format!("expected {token:?} at token {}", self.position));
        }
        self.position += 1; Ok(())
    }
    fn identifier(&mut self) -> Result<String, String> {
        match self.tokens.get(self.position).cloned() {
            Some(Token::Word(name)) if !["int", "const", "return", "void", "if", "else", "while", "break", "continue"].contains(&name.as_str()) => {
                self.position += 1; Ok(name)
            }
            _ => Err(format!("expected identifier at token {}", self.position)),
        }
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
            Some(Token::Word(_)) => self.identifier().map(Expr::Variable),
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
    parser.expect(Token::Word("int".into()))?;
    let name=parser.identifier()?;
    for token in [Token::Symbol(b'('),Token::Symbol(b')'),Token::Symbol(b'{')] {
        parser.expect(token)?;
    }
    let mut items=Vec::new();
    while parser.tokens.get(parser.position) != Some(&Token::Symbol(b'}')) {
        let token=parser.tokens.get(parser.position).cloned();
        let item=match token {
            Some(Token::Word(word)) if word=="int" || word=="const" => {
                let constant=word=="const";
                if constant {parser.position+=1;}
                parser.expect(Token::Word("int".into()))?;
                let mut definitions=Vec::new();
                loop {
                    let name=parser.identifier()?;
                    let initializer=if parser.tokens.get(parser.position)==Some(&Token::Symbol(b'=')) {
                        parser.position+=1;Some(parser.expression(1)?)
                    } else if constant {return Err("constant requires an initializer".into());} else {None};
                    definitions.push(Definition{name,initializer});
                    if parser.tokens.get(parser.position)!=Some(&Token::Symbol(b',')){break;}
                    parser.position+=1;
                }
                BlockItem::Declaration{constant,definitions}
            }
            Some(Token::Word(word)) if word=="return" => {
                parser.position+=1;BlockItem::Return(parser.expression(1)?)
            }
            Some(Token::Word(_)) => {
                let name=parser.identifier()?;parser.expect(Token::Symbol(b'='))?;
                BlockItem::Assignment{name,value:parser.expression(1)?}
            }
            _=>return Err(format!("expected declaration, assignment or return at token {}",parser.position)),
        };
        parser.expect(Token::Symbol(b';'))?;items.push(item);
    }
    parser.expect(Token::Symbol(b'}'))?;
    if parser.position!=parser.tokens.len(){return Err("unexpected tokens after function".into());}
    Ok(Function{name,items})
}

pub fn compile(source: &str, mode: &str) -> Result<String, String> {
    let program=ir::Program::try_from(parse(source)?)?;
    match mode {"-koopa"=>Ok(program.koopa()),"-riscv"|"-perf"=>Ok(program.riscv()),_=>Err(format!("unsupported mode: {mode}"))}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expr(s:&str)->Expr {
        match parse(&format!("int main(){{return {s};}}")).unwrap().items.pop().unwrap() {
            BlockItem::Return(value)=>value,_=>panic!("expected return"),
        }
    }
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
        for s in ["", "/*", "/**", "int main(){return 08;}","int main(){return 0x;}","int main(){return 2147483648;}","int main(){return 1_;}","int main(){return 1a;}","int main(){return 1}","int main(){return 1;} garbage","int return(){return 0;}","in/**/t main(){return 0;}","你好","int main(){return 1/**/2;}"] {assert!(parse(s).is_err(),"{s}");}
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

#[cfg(test)]
mod declaration_tests {
    use super::*;
    #[test]
    fn constants_are_folded_and_variables_get_storage() {
        let ir=compile("int main(){const int x=233*4,y=x/2;int a=10,b=a+y;a=b+1;return a;}","-koopa").unwrap();
        assert!(ir.contains("alloc i32"));assert!(ir.contains("store 10, %v0"));
        assert!(ir.contains("466"));assert!(!ir.contains("mul 233, 4"));
        assert!(ir.contains("load %v"));
    }
    #[test]
    fn reject_invalid_bindings_and_constant_arithmetic() {
        for body in ["const int x;return 0;", "int a;int a;return 0;", "const int a=1;int a;return 0;",
          "return a;int a=1;", "int a=2;const int b=a;return b;", "const int a=b,b=1;return a;",
          "const int a=1;a=2;return a;", "a=1;return 0;", "const int a=1/0;return a;",
          "const int a=2147483647+1;return a;", "int a=1;return (a=2);", "int return=1;return 0;"] {
            assert!(compile(&format!("int main(){{{body}}}"),"-koopa").is_err(),"{body}");
        }
    }
    #[test]
    fn local_main_and_unreachable_statements() {
        let ir=compile("int main(){int main=5;return main;main=9;return 1;}","-koopa").unwrap();
        assert_eq!(ir.matches("  ret ").count(),1);assert!(!ir.contains("store 9"));
        assert!(compile("int main(){return 0;const int a=1;a=2;}","-koopa").is_err());
    }
}

#[cfg(test)]
mod function_name_tests {
    use super::*;
    #[test]
    fn function_identifier_follows_the_grammar() {
        let source="int answer(){const int value=7;return value;}";
        assert_eq!(parse(source).unwrap().name,"answer");
        assert!(compile(source,"-koopa").unwrap().contains("fun @answer(): i32"));
        assert!(compile(source,"-riscv").unwrap().contains(".globl answer\nanswer:"));
        for keyword in ["int","const","return","while"] {
            assert!(parse(&format!("int {keyword}(){{return 0;}}")).is_err());
        }
    }
}
