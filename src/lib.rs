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
    While { condition: Expr, body: Box<BlockItem> },
    Break,
    Continue,
    If { condition: Expr, then: Box<BlockItem>, otherwise: Option<Box<BlockItem>> },
    Block(Vec<BlockItem>),
    Expression(Option<Expr>),
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
impl Parser {
    fn block(&mut self)->Result<Vec<BlockItem>,String> {
        self.expect(Token::Symbol(b'{'))?;
        let mut items=Vec::new();
        while self.tokens.get(self.position)!=Some(&Token::Symbol(b'}')) {
            if matches!(self.tokens.get(self.position),Some(Token::Word(word)) if word=="const" || word=="int") {
                items.push(self.declaration()?);
            } else {items.push(self.statement()?);}
        }
        self.expect(Token::Symbol(b'}'))?;Ok(items)
    }
    fn declaration(&mut self)->Result<BlockItem,String> {
        let constant=self.tokens.get(self.position)==Some(&Token::Word("const".into()));
        if constant {self.position+=1;}
        self.expect(Token::Word("int".into()))?;
        let mut definitions=Vec::new();
        loop {
            let name=self.identifier()?;
            let initializer=if self.tokens.get(self.position)==Some(&Token::Symbol(b'=')) {
                self.position+=1;Some(self.expression(1)?)
            } else if constant {return Err("constant requires an initializer".into());} else {None};
            definitions.push(Definition{name,initializer});
            if self.tokens.get(self.position)!=Some(&Token::Symbol(b',')){break;}
            self.position+=1;
        }
        self.expect(Token::Symbol(b';'))?;Ok(BlockItem::Declaration{constant,definitions})
    }
    fn statement(&mut self)->Result<BlockItem,String> {
        let token=self.tokens.get(self.position).cloned();
        if token==Some(Token::Symbol(b'{')) {return Ok(BlockItem::Block(self.block()?));}
        if token==Some(Token::Word("while".into())) {
            self.position+=1;self.expect(Token::Symbol(b'('))?;
            let condition=self.expression(1)?;self.expect(Token::Symbol(b')'))?;
            return Ok(BlockItem::While{condition,body:Box::new(self.statement()?)});
        }
        if token==Some(Token::Word("break".into())) || token==Some(Token::Word("continue".into())) {
            self.position+=1;self.expect(Token::Symbol(b';'))?;
            return Ok(if token==Some(Token::Word("break".into())){BlockItem::Break}else{BlockItem::Continue});
        }
        if token==Some(Token::Word("if".into())) {
            self.position+=1;self.expect(Token::Symbol(b'('))?;
            let condition=self.expression(1)?;self.expect(Token::Symbol(b')'))?;
            let then=Box::new(self.statement()?);
            let otherwise=if self.tokens.get(self.position)==Some(&Token::Word("else".into())) {
                self.position+=1;Some(Box::new(self.statement()?))
            } else {None};
            return Ok(BlockItem::If{condition,then,otherwise});
        }
        let item=match token {
            Some(Token::Word(word)) if word=="return"=>{self.position+=1;BlockItem::Return(self.expression(1)?)},
            Some(Token::Symbol(b';'))=>BlockItem::Expression(None),
            Some(Token::Word(_)) if self.tokens.get(self.position+1)==Some(&Token::Symbol(b'='))=>{
                let name=self.identifier()?;self.position+=1;BlockItem::Assignment{name,value:self.expression(1)?}
            }
            _=>BlockItem::Expression(Some(self.expression(1)?)),
        };
        self.expect(Token::Symbol(b';'))?;Ok(item)
    }
}
pub fn parse(source: &str) -> Result<Function, String> {
    let mut parser=Parser{tokens:lex(source)?,position:0};
    parser.expect(Token::Word("int".into()))?;
    let name=parser.identifier()?;
    parser.expect(Token::Symbol(b'('))?;parser.expect(Token::Symbol(b')'))?;
    let items=parser.block()?;
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

#[cfg(test)]
mod scope_tests {
    use super::*;
    #[test]
    fn expression_and_empty_statements_and_blocks() {
        let source="int answer(){int a=1,b=2;{int a=2;b=b+a;}a+3;;{}return b;}";
        assert!(compile(source,"-koopa").unwrap().contains("alloc i32"));
        let ir=compile("int main(){3*7;return 2;}","-koopa").unwrap();
        assert!(ir.contains("mul 3, 7"));
    }
    #[test]
    fn lexical_visibility_and_nearest_binding() {
        for body in ["{int a=1;int a=2;}return 0;", "{int a=1;}return a;",
          "int a=1;{const int a=2;a=3;}return a;", "{const int a=2;}const int b=a;return b;",
          "const int a=1;{const int a=a+1;}return a;", "return 1;{int a=2;}a=3;"] {
            assert!(compile(&format!("int main(){{{body}}}"),"-koopa").is_err(),"{body}");
        }
        assert!(compile("int main(){const int a=1;{int a=2;a=3;}return a;}","-koopa").is_ok());
        assert!(compile("int main(){{int a=1;}{int a=2;}return 0;}","-koopa").is_ok());
    }
    #[test]
    fn nested_return_terminates_the_entire_function() {
        let ir=compile("int main(){int x=1;{return x;x=9;}x=7;return x;}","-koopa").unwrap();
        assert_eq!(ir.matches("  ret ").count(),1);
        assert!(!ir.contains("store 9"));assert!(!ir.contains("store 7"));
    }
}

#[cfg(test)]
mod conditional_tests {
    use super::*;
    #[test]
    fn dangling_else_binds_to_nearest_if() {
        let p=parse("int main(){if(1)if(0)return 2;else return 3;return 4;}").unwrap();
        match &p.items[0] {
            BlockItem::If{then,otherwise:None,..}=>assert!(matches!(then.as_ref(),BlockItem::If{otherwise:Some(_),..})),
            _=>panic!("incorrect dangling else"),
        }
    }
    #[test]
    fn short_circuit_constants_still_validate_identifiers() {
        assert!(compile("int main(){const int x=1||(1/0),y=0&&(1/0);return x+y;}","-koopa").unwrap().contains("add 1, 0"));
        for body in ["const int a=1||missing;return a;", "int x=1;const int a=1||x;return a;",
          "if(1)return 1;else return missing;", "if(0){const int x=1;x=2;}return 0;",
          "if(1)int x=2;return 0;"] {
            assert!(compile(&format!("int main(){{{body}}}"),"-koopa").is_err(),"{body}");
        }
    }
}

#[cfg(test)]
mod loop_tests {
    use super::*;
    #[test]
    fn loop_control_is_lexically_checked_even_when_unreachable() {
        for body in ["break;return 0;","continue;return 0;","return 1;break;","if(0)continue;return 0;",
          "while(0){}break;return 0;","while(1)int x=2;return 0;","while()return 1;return 0;"] {
            assert!(compile(&format!("int main(){{{body}}}"),"-koopa").is_err(),"{body}");
        }
        assert!(compile("int main(){return 0;while(0){break;continue;}}","-koopa").is_ok());
    }
    #[test]
    fn nearest_loop_targets_and_condition_reentry() {
        let ir=compile("int main(){int i=0;while(i<3){i=i+1;while(1){break;}continue;}return i;}","-koopa").unwrap();
        assert!(ir.contains("jump %while_cond_1"));assert!(ir.contains("while_end_"));
        assert!(ir.contains("while_cond_4"));
    }
}
