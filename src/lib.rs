pub mod ir;
pub mod arrays;

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
        } else if b"(){};,=[]".contains(&bytes[pos]) {
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
    Index { name: String, indices: Vec<Expr> },
    Call { name: String, arguments: Vec<Expr> },
    Unary(UnaryOp, Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}
#[derive(Debug, PartialEq, Eq)]
pub struct Function { pub name: String, pub parameters: Vec<String>, pub parameter_dimensions: Vec<Option<Vec<Expr>>>, pub returns_void: bool, pub items: Vec<BlockItem> }
#[derive(Debug, PartialEq, Eq)]
pub enum TopLevel { Declaration(BlockItem), Function(Function) }
#[derive(Debug, PartialEq, Eq)]
pub struct CompilationUnit { pub items: Vec<TopLevel> }
impl CompilationUnit {
    /// Complete SysY programs require exactly one int main with no parameters.
    /// Parsing itself also supports object fragments used by lower-level backend tests.
    pub fn validate_entry(&self) -> Result<(), String> {
        let mains: Vec<_> = self.items.iter().filter_map(|item| match item {
            TopLevel::Function(function) if function.name == "main" => Some(function), _ => None,
        }).collect();
        if mains.len()!=1 || mains[0].returns_void || !mains[0].parameters.is_empty() {
            return Err("program requires exactly one parameterless int main".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition { pub name: String, pub dimensions: Vec<Expr>, pub initializer: Option<Initializer> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Initializer { Scalar(Expr), List(Vec<Initializer>) }
impl Initializer {
    pub fn as_scalar(&self)->Result<&Expr,String> {
        match self {Self::Scalar(expr)=>Ok(expr),_=>Err("scalar initializer cannot be an aggregate".into())}
    }
    pub fn into_scalar(self)->Result<Expr,String> {
        match self {Self::Scalar(expr)=>Ok(expr),_=>Err("scalar initializer cannot be an aggregate".into())}
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockItem {
    While { condition: Expr, body: Box<BlockItem> },
    Break,
    Continue,
    If { condition: Expr, then: Box<BlockItem>, otherwise: Option<Box<BlockItem>> },
    Block(Vec<BlockItem>),
    Expression(Option<Expr>),
    Declaration { constant: bool, definitions: Vec<Definition> },
    Assignment { name: String, indices: Vec<Expr>, value: Expr },
    Return(Expr),
    ReturnVoid,
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
            Some(Token::Word(_)) => {
                let name=self.identifier()?;
                if self.tokens.get(self.position)!=Some(&Token::Symbol(b'(')) {
                    let indices=self.dimensions()?;
                    return Ok(if indices.is_empty(){Expr::Variable(name)}else{Expr::Index{name,indices}});
                }
                self.position+=1;let mut arguments=Vec::new();
                if self.tokens.get(self.position)!=Some(&Token::Symbol(b')')) {
                    loop {
                        arguments.push(self.expression(1)?);
                        if self.tokens.get(self.position)!=Some(&Token::Symbol(b',')){break;}
                        self.position+=1;
                    }
                }
                self.expect(Token::Symbol(b')'))?;Ok(Expr::Call{name,arguments})
            },
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
    fn dimensions(&mut self)->Result<Vec<Expr>,String> {
        let mut dimensions=Vec::new();
        while self.tokens.get(self.position)==Some(&Token::Symbol(b'[')) {
            self.position+=1;dimensions.push(self.expression(1)?);self.expect(Token::Symbol(b']'))?;
        }
        Ok(dimensions)
    }
    fn initializer(&mut self)->Result<Initializer,String> {
        if self.tokens.get(self.position)!=Some(&Token::Symbol(b'{')) {return Ok(Initializer::Scalar(self.expression(1)?));}
        self.position+=1;let mut values=Vec::new();
        if self.tokens.get(self.position)!=Some(&Token::Symbol(b'}')) {
            loop {
                values.push(self.initializer()?);
                if self.tokens.get(self.position)!=Some(&Token::Symbol(b',')){break;}
                self.position+=1;
            }
        }
        self.expect(Token::Symbol(b'}'))?;Ok(Initializer::List(values))
    }
    fn declaration(&mut self)->Result<BlockItem,String> {
        let constant=self.tokens.get(self.position)==Some(&Token::Word("const".into()));
        if constant {self.position+=1;}
        self.expect(Token::Word("int".into()))?;
        let mut definitions=Vec::new();
        loop {
            let name=self.identifier()?;let dimensions=self.dimensions()?;
            let initializer=if self.tokens.get(self.position)==Some(&Token::Symbol(b'=')) {
                self.position+=1;Some(self.initializer()?)
            } else if constant {return Err("constant requires an initializer".into());} else {None};
            definitions.push(Definition{name,dimensions,initializer});
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
            Some(Token::Word(word)) if word=="return"=>{
                self.position+=1;
                if self.tokens.get(self.position)==Some(&Token::Symbol(b';')){BlockItem::ReturnVoid}
                else {BlockItem::Return(self.expression(1)?)}
            },
            Some(Token::Symbol(b';'))=>BlockItem::Expression(None),
            Some(Token::Word(_))=>{
                let start=self.position;let name=self.identifier()?;let indices=self.dimensions()?;
                if self.tokens.get(self.position)==Some(&Token::Symbol(b'=')) {
                    self.position+=1;BlockItem::Assignment{name,indices,value:self.expression(1)?}
                } else {self.position=start;BlockItem::Expression(Some(self.expression(1)?))}
            }
            _=>BlockItem::Expression(Some(self.expression(1)?)),
        };
        self.expect(Token::Symbol(b';'))?;Ok(item)
    }
}
impl Parser {
    fn function(&mut self)->Result<Function,String> {
        let returns_void=self.tokens.get(self.position)==Some(&Token::Word("void".into()));
        self.expect(Token::Word(if returns_void {"void"} else {"int"}.into()))?;
        let name=self.identifier()?;self.expect(Token::Symbol(b'('))?;let mut parameters=Vec::new();let mut parameter_dimensions=Vec::new();
        if self.tokens.get(self.position)!=Some(&Token::Symbol(b')')) {
            loop {
                self.expect(Token::Word("int".into()))?;parameters.push(self.identifier()?);
                let dimensions=if self.tokens.get(self.position)==Some(&Token::Symbol(b'[')) {
                    self.position+=1;self.expect(Token::Symbol(b']'))?;Some(self.dimensions()?)
                }else{None};
                parameter_dimensions.push(dimensions);
                if self.tokens.get(self.position)!=Some(&Token::Symbol(b',')){break;}
                self.position+=1;
            }
        }
        self.expect(Token::Symbol(b')'))?;let items=self.block()?;
        Ok(Function{name,parameters,parameter_dimensions,returns_void,items})
    }
}
pub fn parse_compilation_unit(source:&str)->Result<CompilationUnit,String> {
    let mut parser=Parser{tokens:lex(source)?,position:0};let mut items=Vec::new();
    while parser.position<parser.tokens.len() {
        let is_function=matches!(parser.tokens.get(parser.position),Some(Token::Word(word)) if word=="int"||word=="void")
            && matches!(parser.tokens.get(parser.position+1),Some(Token::Word(_)))
            && parser.tokens.get(parser.position+2)==Some(&Token::Symbol(b'('));
        if is_function {items.push(TopLevel::Function(parser.function()?));}
        else {items.push(TopLevel::Declaration(parser.declaration()?));}
    }
    if items.is_empty(){return Err("empty compilation unit".into());}
    Ok(CompilationUnit{items})
}
pub fn parse(source: &str) -> Result<Function, String> {
    let mut unit=parse_compilation_unit(source)?;
    if unit.items.len()!=1{return Err("single-function parser requires one function".into());}
    match unit.items.pop().unwrap() {
        TopLevel::Function(function)=>Ok(function),
        _=>Err("single-function parser requires a function".into()),
    }
}

pub fn compile(source: &str, mode: &str) -> Result<String, String> {
    let program=ir::Program::try_from(parse_compilation_unit(source)?)?;
    match mode {"-koopa"=>Ok(program.koopa()),"-riscv"|"-perf"=>Ok(program.riscv()),_=>Err(format!("unsupported mode: {mode}"))}
}

/// Complete executable entry; compile() remains the explicit object-fragment API.
pub fn compile_program(source:&str,mode:&str)->Result<String,String> {
    let unit=parse_compilation_unit(source)?;unit.validate_entry()?;
    let program=ir::Program::try_from(unit)?.isolate_functions();
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

#[cfg(test)]
mod lv8_syntax_tests {
    use super::*;
    #[test]
    fn compilation_unit_retains_declaration_order_and_calls() {
        let unit=parse_compilation_unit("const int seed=3;int g=seed,zero;void bump(int x){g=g+x;return;}int twice(int x){return bump(x)+x;}int main(){bump(twice(2));return g;}").unwrap();
        unit.validate_entry().unwrap();assert_eq!(unit.items.len(),5);
        let TopLevel::Function(function)=&unit.items[2] else {panic!("expected function")};
        assert!(function.returns_void);assert_eq!(function.parameters,vec!["x"]);
        assert!(matches!(function.items.last(),Some(BlockItem::ReturnVoid)));
        let TopLevel::Function(main)=&unit.items[4] else {panic!("expected main")};
        let BlockItem::Expression(Some(Expr::Call{name,arguments}))=&main.items[0] else {panic!("expected call")};
        assert_eq!(name,"bump");assert!(matches!(&arguments[0],Expr::Call{name,..} if name=="twice"));
        // Syntax accepts void use in an expression; semantic lowering must reject it.
    }
    #[test]
    fn arbitrary_scalar_parameter_and_argument_lists() {
        for count in [0,1,8,9,10,40] {
            let params=(0..count).map(|i|format!("int x{i}")).collect::<Vec<_>>().join(",");
            let args=(0..count).map(|i|i.to_string()).collect::<Vec<_>>().join(",");
            let unit=parse_compilation_unit(&format!("int f({params}){{return 7;}}int main(){{return f({args});}}")).unwrap();
            let TopLevel::Function(function)=&unit.items[0] else {panic!()};assert_eq!(function.parameters.len(),count);
            let TopLevel::Function(main)=&unit.items[1] else {panic!()};
            let BlockItem::Return(Expr::Call{arguments,..})=&main.items[0] else {panic!()};assert_eq!(arguments.len(),count);
        }
    }
    #[test]
    fn program_entry_contract_and_malformed_call_syntax() {
        for source in ["int f(){return 0;}","void main(){}","int main(int a){return a;}","int main(){return 0;}int main(){return 1;}"] {
            assert!(parse_compilation_unit(source).unwrap().validate_entry().is_err());
        }
        for source in ["int f(int){}","int f(int x,){}","int main(){f(,1);return 0;}","int main(){f(1,);return 0;}","int main(){return f(1;}","void x;","int f();"] {
            assert!(parse_compilation_unit(source).is_err(),"{source}");
        }
        parse_compilation_unit("void f(){return;}int main(){f();return 0;}").unwrap().validate_entry().unwrap();
    }
}

#[cfg(test)]
mod lv8_lowering_tests {
 use super::*;
 #[test]
 fn functions_globals_recursion_and_void_lower_to_shared_ir(){
  let source="const int seed=3;int g=seed,z;void bump(int n){g=g+n;}int fact(int n){if(n<2)return 1;return n*fact(n-1);}int main(){bump(fact(4));return g+z;}";
  let ir=compile(source,"-koopa").unwrap();
  assert!(ir.contains("global @__sysy_global_0 = alloc i32, 3"));
  assert!(ir.contains("fun @bump(%arg0: i32) {"));assert!(ir.contains("call @fact("));
  let asm=compile(source,"-riscv").unwrap();assert!(asm.contains("call fact"));assert!(asm.contains("sw ra, 0(t3)"));assert!(asm.contains("lw ra, 0(t3)"));
 }
 #[test]
 fn signatures_scope_and_return_contracts_are_checked(){
  for source in [
   "int main(){return missing();}","int main(){return f();}int f(){return 1;}",
   "int f(int x){return x;}int main(){return f();}",
   "int f(int x){return x;}int main(){return f(1,2);}",
   "void f(){}int main(){return f();}","void f(){return 1;}int main(){return 0;}",
   "int main(){return;}","int f(int x,int x){return x;}int main(){return 0;}",
   "int g;int g(){return 1;}int main(){return 0;}","int getint(){return 1;}int main(){return 0;}",
   "int g=1;int h=g;int main(){return h;}","const int g=g;int main(){return g;}",
   "int f(){return 1;}int main(){int f=2;return f();}",
   "void f(){}int main(){return 1+f();}","int main(){return putint(1);}",
   "int main(){return 1;}int f(){return missing();}",
  ] {assert!(compile_program(source,"-koopa").is_err(),"{source}");}
  assert!(compile_program("void f(){return;}int main(){f();return 0;}","-koopa").is_ok());
  assert!(compile_program("int f(){return 2;}","-koopa").is_err());
  assert!(compile("int f(){return 2;}","-koopa").is_ok());
 }
 #[test]
 fn stack_arguments_and_symbol_collisions_are_preserved(){
  let source="int __sysy_global_0;int v0(int a,int b,int c,int d,int e,int f,int g,int h,int i,int j){return i+j;}int main(){return v0(1,2,3,4,5,6,7,8,9,10);}";
  let ir=compile(source,"-koopa").unwrap();assert!(ir.contains("fun @v0("));assert!(ir.contains("%v0_ = alloc"));
  let asm=compile(source,"-riscv").unwrap();assert!(asm.contains("mv t0, a7"));assert!(asm.contains("call v0"));
 }
}

#[cfg(test)]
mod runtime_symbol_tests {
 use super::*;
 #[test]
 fn complete_program_does_not_export_runtime_colliding_function_names(){
  let source="int g;void read(){g=getint();}int main(){read();return g;}";
  let ir=compile_program(source,"-koopa").unwrap();
  assert!(!ir.contains("fun @read("));assert!(ir.contains("call @__sysy_function_0()"));assert!(ir.contains("call @getint()"));
  let asm=compile_program(source,"-riscv").unwrap();assert!(!asm.contains("\nread:"));assert!(asm.contains("call __sysy_function_0"));
  assert!(compile(source,"-koopa").unwrap().contains("fun @read("));
 }
}

#[cfg(test)]
mod lv9_syntax_tests {
 use super::*;
 #[test]
 fn dimensions_aggregate_lists_subscripts_and_array_formals(){
  let unit=parse_compilation_unit("const int n=3;int g[2][n]={{1},{2,3}};int f(int a[][3],int b[],int x){a[x][1]=b[x]+a[x][0];return a[x][1];}int main(){int v[2][3]={};int q[3]={1,2,3};return f(v,q,1);}").unwrap();
  let TopLevel::Declaration(BlockItem::Declaration{definitions,..})=&unit.items[1] else{panic!()};
  assert_eq!(definitions[0].dimensions,vec![Expr::Number(2),Expr::Variable("n".into())]);
  assert!(matches!(&definitions[0].initializer,Some(Initializer::List(values)) if values.len()==2));
  let TopLevel::Function(f)=&unit.items[2] else{panic!()};
  assert_eq!(f.parameter_dimensions,vec![Some(vec![Expr::Number(3)]),Some(vec![]),None]);
  assert!(matches!(&f.items[0],BlockItem::Assignment{indices,..} if indices.len()==2));
  assert!(matches!(&f.items[1],BlockItem::Return(Expr::Index{indices,..}) if indices.len()==2));
 }
 #[test]
 fn indexed_expression_statements_and_malformed_array_syntax(){
  parse_compilation_unit("int main(){int a[2]={1,2};a[0]+a[1];a[1]=a[0]*2;return a[1];}").unwrap();
  for source in ["int a[];","int a[2]={1,};","int a[2]={{1};","int f(int a[3]){return 0;}","int f(int a[][ ]){return 0;}","int main(){int a[2];a[]=1;return 0;}","int main(){return a[1;}" ] {
   assert!(parse_compilation_unit(source).is_err(),"{source}");
  }
 }
}

#[cfg(test)]
mod lv9_lowering_tests {
 use super::*;
 #[test]
 fn global_aggregate_local_arrays_and_parameter_address_steps(){
  let source="const int n=3;int g[2][n]={{1},{2,3}};int sum(int a[][3],int rows){int i=0,r=0;while(i<rows){r=r+a[i][1];i=i+1;}return r;}int main(){int a[2][3]={{4},{5,6}};g[0][2]=7;return sum(a,2)+g[0][2];}";
  let ir=compile_program(source,"-koopa").unwrap();assert!(ir.contains("alloc [[i32, 3], 2], {{1, 0, 0}, {2, 3, 0}}"));
  assert!(ir.contains(": *[i32, 3]"));assert!(ir.contains("getptr "));assert!(ir.contains("getelemptr "));
  let asm=compile_program(source,"-riscv").unwrap();assert!(asm.contains("li t2, 12"));assert!(asm.contains("lw t0, 0(t0)"));assert!(asm.contains("sw t0, 0(t1)"));
 }
 #[test]
 fn arrays_have_types_readonly_rules_and_constant_dimensions(){
  for source in [
   "int main(){const int a[2]={1,2};a[1]=3;return a[1];}",
   "int f(int a[][3]){return a[0][0];}int main(){int a[2][2]={};return f(a);}",
   "int f(int a[]){return a[0];}int main(){int a[2]={1,2};return f(a[0]);}",
   "int main(){int a[2]={};int x=a;return x;}",
   "int main(){int a[2]={};return a[0][1];}",
   "int main(){int a[-1]={};return 0;}",
   "int main(){int n=2;int a[n]={};return 0;}",
   "const int a[2]={1,2};const int b=a[0];int main(){return b;}",
   "int main(){int a[2][2][2]={{},1,{}};return 0;}",
   "int main(){int a[2]={1,2,3};return 0;}",
   "const int n=3;int f(int n,int a[][n]){return a[0][0];}int main(){return 0;}",
  ] {assert!(compile_program(source,"-koopa").is_err(),"{source}");}
  let ir=compile_program("int main(){int a[3];a[0]=7;return a[0];}","-koopa").unwrap();assert!(!ir.contains("store 0,"));
 }
 #[test]
 fn subarray_decay_and_real_array_library_signatures(){
  let source="void bump(int a[]){a[0]=a[0]+1;}int main(){int a[2][3]={{1,2},{3}};bump(a[1]);putarray(3,a[1]);return getarray(a[0]);}";
  let ir=compile_program(source,"-koopa").unwrap();assert!(ir.contains("decl @getarray(*i32): i32"));assert!(ir.contains("decl @putarray(i32, *i32)"));
  assert!(ir.contains("alloc *i32"));
 }
}
