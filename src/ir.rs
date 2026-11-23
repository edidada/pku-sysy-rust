//! Memory IR shared by the Koopa and RV32IM backends.
use crate::{BinaryOp, BlockItem, Expr, Function, UnaryOp, CompilationUnit, TopLevel};
use std::{collections::HashMap, fmt::Write};
#[path="array_lowering.rs"] mod array_lowering;
#[path="riscv_spills.rs"] mod riscv_spills;
use crate::arrays::{element_count,flatten_initializer};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand { Integer(i32), Temporary(usize) }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op { Add, Sub, Mul, Div, Mod, Lt, Gt, Le, Ge, Eq, Ne, And, Or }
impl Op {
    fn koopa(self) -> &'static str { match self {
        Self::Add=>"add",Self::Sub=>"sub",Self::Mul=>"mul",Self::Div=>"div",Self::Mod=>"mod",
        Self::Lt=>"lt",Self::Gt=>"gt",Self::Le=>"le",Self::Ge=>"ge",Self::Eq=>"eq",Self::Ne=>"ne",Self::And=>"and",Self::Or=>"or",
    }}
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    Allocate { slot: usize },
    AllocateArray { slot: usize, shape: Vec<usize> },
    AllocatePointer { slot: usize, shape: Vec<usize> },
    ElementPointer { destination: usize, base: PointerBase, index: Operand, stride: usize, parameter_step: bool },
    IndirectLoad { destination: usize, pointer: Operand },
    IndirectStore { pointer: Operand, value: Operand },
    Parameter { slot: usize, index: usize },
    GlobalLoad { destination: usize, global: usize },
    GlobalStore { global: usize, value: Operand },
    Call { destination: Option<usize>, name: String, arguments: Vec<Operand> },
    ReturnVoid,
    Load { destination: usize, slot: usize },
    Store { slot: usize, value: Operand },
    Binary { destination: usize, op: Op, lhs: Operand, rhs: Operand },
    Branch { condition: Operand, then_target: String, else_target: String },
    Jump(String),
    Return(Operand),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock { pub name: String, pub instructions: Vec<Instruction> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionIr { pub name: String, pub parameters: usize, pub parameter_types: Vec<Option<Vec<usize>>>, pub returns_void: bool, pub blocks: Vec<BasicBlock> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program { pub functions: Vec<FunctionIr>, pub globals: Vec<GlobalData>, pub declarations: Vec<Signature> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature { pub name: String, pub pointers: Vec<Option<Vec<usize>>>, pub returns_void: bool }
fn libraries()->Vec<Signature> {
    [("getint",vec![],false),("getch",vec![],false),("getarray",vec![true],false),
     ("putint",vec![false],true),("putch",vec![false],true),("putarray",vec![false,true],true),
     ("starttime",vec![],true),("stoptime",vec![],true)].into_iter()
      .map(|(name,pointers,returns_void)|Signature{name:name.into(),pointers:pointers.into_iter().map(|p|if p{Some(vec![])}else{None}).collect(),returns_void}).collect()
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalData { pub shape: Vec<usize>, pub values: Vec<i32> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerBase { Slot(usize), Global(usize), Temporary(usize) }
#[derive(Clone)]
enum ArrayStorage { Local(usize), Global(usize), Parameter(usize) }
#[derive(Clone)]
struct ArrayBinding { storage: ArrayStorage, shape: Vec<usize>, readonly: bool }
#[derive(Clone)]
enum Binding { PendingConstant, Constant(i32), Variable(usize), Global(usize), Function(usize), Array(ArrayBinding) }
fn array_type(shape:&[usize])->String {shape.iter().rev().fold("i32".to_owned(),|ty,len|format!("[{ty}, {len}]"))}
fn argument_type(shape:&Option<Vec<usize>>)->String {shape.as_ref().map(|s|format!("*{}",array_type(s))).unwrap_or_else(||"i32".into())}
fn aggregate(shape:&[usize],values:&[i32])->String {
    if shape.is_empty(){return values[0].to_string();}
    if values.iter().all(|v|*v==0){return "zeroinit".into();}
    let width=shape[1..].iter().product::<usize>();
    format!("{{{}}}",(0..shape[0]).map(|i|aggregate(&shape[1..],&values[i*width..(i+1)*width])).collect::<Vec<_>>().join(", "))
}
struct Builder {
    signatures: Vec<Signature>, returns_void: bool,
    blocks: Vec<BasicBlock>, active: Option<usize>, allocations: Vec<Instruction>,
    next: usize, next_slot: usize, scopes: Vec<HashMap<String, Binding>>, has_return: bool, loops: Vec<(usize,usize)>,
}
impl Builder {
    fn lookup(&self,name:&str)->Option<Binding> {self.scopes.iter().rev().find_map(|scope|scope.get(name)).cloned()}
    fn current(&mut self)->&mut HashMap<String,Binding> {self.scopes.last_mut().expect("function scope exists")}
    fn emit(&mut self,instruction:Instruction) {
        if let Some(block)=self.active {
            if matches!(instruction,Instruction::Allocate{..}|Instruction::AllocateArray{..}|Instruction::AllocatePointer{..}) {self.allocations.push(instruction);}
            else {self.blocks[block].instructions.push(instruction);}
        }
    }
    fn block(&mut self,prefix:&str)->usize {
        let id=self.blocks.len();self.blocks.push(BasicBlock{name:format!("{prefix}_{id}"),instructions:Vec::new()});id
    }
    fn name(&self,id:usize)->String {self.blocks[id].name.clone()}
    fn jump(&mut self,id:usize) {self.emit(Instruction::Jump(self.name(id)));self.active=None;}
    fn allocate(&mut self)->usize {
        let slot=self.next_slot;self.next_slot+=1;self.emit(Instruction::Allocate{slot});slot
    }
    fn load_slot(&mut self,slot:usize)->Operand {
        let destination=self.next;self.next+=1;self.emit(Instruction::Load{destination,slot});Operand::Temporary(destination)
    }
    fn binary(&mut self,op:Op,lhs:Operand,rhs:Operand)->Operand {
        let destination=self.next;self.next+=1;self.emit(Instruction::Binary{destination,op,lhs,rhs});Operand::Temporary(destination)
    }
    fn validate(&self,expr:&Expr,constant:bool)->Result<(),String> {
        match expr {
            Expr::Index{name,indices}=>{
                if constant{return Err("array access is not a scalar constant expression".into());}
                self.validate_array_element(name,indices)
            },
            Expr::Call{name,arguments}=>{
                if constant{return Err("function calls are not constant expressions".into());}
                let signature=self.call_signature(name,arguments)?;
                if signature.returns_void{return Err("void call used as a value".into());}Ok(())
            },
            Expr::Number(_)=>Ok(()),
            Expr::Variable(name)=>match self.lookup(name) {
                Some(Binding::Constant(_))=>Ok(()),Some(Binding::Variable(_)|Binding::Global(_)) if !constant=>Ok(()),
                _=>Err(format!("undefined, initializing or nonconstant identifier: {name}")),
            },
            Expr::Unary(_,value)=>self.validate(value,constant),
            Expr::Binary(_,lhs,rhs)=>{self.validate(lhs,constant)?;self.validate(rhs,constant)},
        }
    }
    fn call_signature(&self,name:&str,arguments:&[Expr])->Result<Signature,String> {
        let Some(Binding::Function(id))=self.lookup(name) else {return Err(format!("undefined or shadowed function: {name}"));};
        let signature=self.signatures[id].clone();
        if arguments.len()!=signature.pointers.len(){return Err(format!("argument count mismatch: {name}"));}
        for (argument,pointer) in arguments.iter().zip(&signature.pointers) {
            if let Some(expected)=pointer {
                if self.array_argument_type(argument)?!=*expected{return Err(format!("array argument shape mismatch: {name}"));}
            }else{self.validate(argument,false)?;}
        }
        Ok(signature)
    }
    fn call(&mut self,name:String,arguments:Vec<Expr>)->Result<Operand,String> {
        let signature=self.call_signature(&name,&arguments)?;
        if self.active.is_none(){return Ok(Operand::Integer(0));}
        let arguments=arguments.into_iter().zip(&signature.pointers).map(|(expr,pointer)|self.lower_argument(expr,pointer)).collect::<Result<Vec<_>,_>>()?;
        let destination=if signature.returns_void {None}else{let id=self.next;self.next+=1;Some(id)};
        self.emit(Instruction::Call{destination,name,arguments});
        Ok(destination.map(Operand::Temporary).unwrap_or(Operand::Integer(0)))
    }
    fn constant(&self, expr: &Expr) -> Result<i32, String> {
        let bad=||"undefined or overflowing constant arithmetic".to_owned();
        match expr {
            Expr::Index{..}=>Err("array access is not a scalar constant expression".into()),
            Expr::Call{..}=>Err("function calls are not constant expressions".into()),
            Expr::Number(value)=>Ok(*value),
            Expr::Variable(name)=>match self.lookup(name) {
                Some(Binding::Constant(value))=>Ok(value),
                _=>Err(format!("constant expression needs an earlier constant: {name}")),
            },
            Expr::Unary(op, value)=>{
                let value=self.constant(value)?;
                match op { UnaryOp::Plus=>Ok(value),UnaryOp::Minus=>value.checked_neg().ok_or_else(bad),UnaryOp::Not=>Ok(i32::from(value==0)) }
            }
            Expr::Binary(op, lhs, rhs)=>{
                let a=self.constant(lhs)?;
                if *op==BinaryOp::And && a==0 {return Ok(0);}
                if *op==BinaryOp::Or && a!=0 {return Ok(1);}
                let b=self.constant(rhs)?;
                match op {
                    BinaryOp::Add=>a.checked_add(b).ok_or_else(bad),BinaryOp::Sub=>a.checked_sub(b).ok_or_else(bad),
                    BinaryOp::Mul=>a.checked_mul(b).ok_or_else(bad),BinaryOp::Div=>a.checked_div(b).ok_or_else(bad),BinaryOp::Mod=>a.checked_rem(b).ok_or_else(bad),
                    BinaryOp::Lt=>Ok(i32::from(a<b)),BinaryOp::Gt=>Ok(i32::from(a>b)),BinaryOp::Le=>Ok(i32::from(a<=b)),BinaryOp::Ge=>Ok(i32::from(a>=b)),
                    BinaryOp::Eq=>Ok(i32::from(a==b)),BinaryOp::Ne=>Ok(i32::from(a!=b)),BinaryOp::And=>Ok(i32::from(a!=0 && b!=0)),BinaryOp::Or=>Ok(i32::from(a!=0 || b!=0)),
                }
            }
        }
    }
    fn lower(&mut self,expr:Expr)->Result<Operand,String> {
        self.validate(&expr,false)?;
        if self.active.is_none() {return Ok(Operand::Integer(0));}
        self.lower_validated(expr)
    }
    fn lower_validated(&mut self,expr:Expr)->Result<Operand,String> {
        Ok(match expr {
            Expr::Index{name,indices}=>self.read_array(&name,indices)?,
            Expr::Call{name,arguments}=>self.call(name,arguments)?,
            Expr::Number(value)=>Operand::Integer(value),
            Expr::Variable(name)=>match self.lookup(&name) {
                Some(Binding::Constant(value))=>Operand::Integer(value),
                Some(Binding::Variable(slot))=>self.load_slot(slot),
                Some(Binding::Global(global))=>{let destination=self.next;self.next+=1;self.emit(Instruction::GlobalLoad{destination,global});Operand::Temporary(destination)},
                _=>return Err(format!("undefined identifier: {name}")),
            },
            Expr::Unary(op,value)=>{
                let rhs=self.lower_validated(*value)?;
                match op {UnaryOp::Plus=>rhs,UnaryOp::Minus=>self.binary(Op::Sub,Operand::Integer(0),rhs),UnaryOp::Not=>self.binary(Op::Eq,rhs,Operand::Integer(0))}
            }
            Expr::Binary(op,lhs,rhs) if matches!(op,BinaryOp::And|BinaryOp::Or)=>{
                let lhs=self.lower_validated(*lhs)?;
                let slot=self.allocate();
                self.emit(Instruction::Store{slot,value:Operand::Integer(i32::from(op==BinaryOp::Or))});
                let rhs_block=self.block("sc_rhs");let end=self.block("sc_end");
                let (yes,no)=if op==BinaryOp::And {(rhs_block,end)}else{(end,rhs_block)};
                self.emit(Instruction::Branch{condition:lhs,then_target:self.name(yes),else_target:self.name(no)});
                self.active=Some(rhs_block);
                let value=self.lower_validated(*rhs)?;
                let value=self.binary(Op::Ne,value,Operand::Integer(0));
                self.emit(Instruction::Store{slot,value});self.jump(end);
                self.active=Some(end);self.load_slot(slot)
            }
            Expr::Binary(op,lhs,rhs)=>{
                let lhs=self.lower_validated(*lhs)?;let rhs=self.lower_validated(*rhs)?;
                let op=match op {
                    BinaryOp::Add=>Op::Add,BinaryOp::Sub=>Op::Sub,BinaryOp::Mul=>Op::Mul,BinaryOp::Div=>Op::Div,BinaryOp::Mod=>Op::Mod,
                    BinaryOp::Lt=>Op::Lt,BinaryOp::Gt=>Op::Gt,BinaryOp::Le=>Op::Le,BinaryOp::Ge=>Op::Ge,BinaryOp::Eq=>Op::Eq,BinaryOp::Ne=>Op::Ne,
                    _=>unreachable!(),
                };self.binary(op,lhs,rhs)
            }
        })
    }
    fn item(&mut self,item:BlockItem)->Result<(),String> {
        match item {
            BlockItem::ReturnVoid=>{
                if !self.returns_void{return Err("int function requires a return value".into());}
                self.emit(Instruction::ReturnVoid);self.active=None;self.has_return=true;
            },
            BlockItem::While{condition,body}=>{
                if self.active.is_some() {
                    let test=self.block("while_cond");let body_block=self.block("while_body");let end=self.block("while_end");
                    self.jump(test);self.active=Some(test);
                    let condition=self.lower(condition)?;
                    self.emit(Instruction::Branch{condition,then_target:self.name(body_block),else_target:self.name(end)});
                    self.loops.push((test,end));self.active=Some(body_block);
                    let result=self.item(*body);self.loops.pop();result?;
                    if self.active.is_some(){self.jump(test);}
                    self.active=Some(end);
                } else {
                    self.lower(condition)?;self.loops.push((0,0));
                    let result=self.item(*body);self.loops.pop();result?;
                }
            }
            BlockItem::Break|BlockItem::Continue=>{
                let (test,end)=self.loops.last().copied().ok_or("break/continue outside a loop")?;
                self.jump(if matches!(item,BlockItem::Break){end}else{test});
            }
            BlockItem::Block(items)=>{
                self.scopes.push(HashMap::new());
                let result=(||{for item in items{self.item(item)?;}Ok::<(),String>(())})();
                self.scopes.pop();result?;
            }
            BlockItem::If{condition,then,otherwise}=>{
                let condition=self.lower(condition)?;
                if self.active.is_some() {
                    let yes=self.block("if_then");let no=self.block("if_else");let end=self.block("if_end");
                    self.emit(Instruction::Branch{condition,then_target:self.name(yes),else_target:self.name(no)});
                    self.active=Some(yes);self.item(*then)?;
                    let yes_falls=self.active.is_some();if yes_falls{self.jump(end);}
                    self.active=Some(no);if let Some(otherwise)=otherwise{self.item(*otherwise)?;}
                    let no_falls=self.active.is_some();if no_falls{self.jump(end);}
                    self.active=if yes_falls||no_falls{Some(end)}else{None};
                } else {
                    self.item(*then)?;if let Some(otherwise)=otherwise{self.item(*otherwise)?;}
                }
            }
            BlockItem::Expression(Some(Expr::Call{name,arguments}))=>{self.call(name,arguments)?;},
            BlockItem::Expression(Some(expr))=>{self.lower(expr)?;},BlockItem::Expression(None)=>{},
            BlockItem::Declaration{constant,definitions}=>{
                for definition in definitions {
                    if !definition.dimensions.is_empty(){self.declare_array(definition,constant)?;continue;}
                    if self.current().contains_key(&definition.name){return Err(format!("duplicate identifier: {}",definition.name));}
                    if constant {
                        self.current().insert(definition.name.clone(),Binding::PendingConstant);
                        let expr=definition.initializer.as_ref().ok_or("missing constant initializer")?.as_scalar()?;
                        self.validate(expr,true)?;let value=self.constant(expr)?;
                        self.current().insert(definition.name,Binding::Constant(value));
                    } else {
                        let slot=self.allocate();self.current().insert(definition.name,Binding::Variable(slot));
                        if let Some(initializer)=definition.initializer{let value=self.lower(initializer.into_scalar()?)?;self.emit(Instruction::Store{slot,value});}
                    }
                }
            }
            BlockItem::Assignment{name,indices,value}=>{
                if !indices.is_empty(){return self.assign_array(&name,indices,value);}
                let binding=self.lookup(&name);let value=self.lower(value)?;
                match binding {
                    Some(Binding::Variable(slot))=>self.emit(Instruction::Store{slot,value}),
                    Some(Binding::Global(global))=>self.emit(Instruction::GlobalStore{global,value}),
                    _=>return Err(format!("undefined or constant assignment: {name}")),
                }
            }
            BlockItem::Return(expr)=>{
                if self.returns_void{return Err("void function cannot return a value".into());}
                let value=self.lower(expr)?;self.emit(Instruction::Return(value));self.active=None;self.has_return=true;
            }
        }Ok(())
    }
}
impl Builder {
    fn new(globals:HashMap<String,Binding>,signatures:Vec<Signature>,returns_void:bool)->Self {
        Self{signatures,returns_void,blocks:vec![BasicBlock{name:"entry".into(),instructions:Vec::new()}],active:Some(0),allocations:Vec::new(),next:0,next_slot:0,scopes:vec![globals,HashMap::new()],has_return:false,loops:Vec::new()}
    }
    fn function(mut builder:Self,ast:Function)->Result<FunctionIr,String> {
        let parameter_types=builder.parameter_shapes(&ast)?;
        let parameters=ast.parameters.len();let returns_void=ast.returns_void;
        for (index,name) in ast.parameters.into_iter().enumerate() {
            if builder.current().contains_key(&name){return Err(format!("duplicate parameter: {name}"));}
            let slot=if let Some(shape)=&parameter_types[index] {
                let slot=builder.next_slot;builder.next_slot+=1;builder.emit(Instruction::AllocatePointer{slot,shape:shape.clone()});
                let mut full=vec![0];full.extend(shape);builder.current().insert(name,Binding::Array(ArrayBinding{storage:ArrayStorage::Parameter(slot),shape:full,readonly:false}));slot
            }else{let slot=builder.allocate();builder.current().insert(name,Binding::Variable(slot));slot};
            builder.emit(Instruction::Parameter{slot,index});
        }
        for item in ast.items{builder.item(item)?;}
        if !returns_void && !builder.has_return{return Err("function requires a return statement".into());}
        if builder.active.is_some(){
            builder.emit(if returns_void {Instruction::ReturnVoid}else{Instruction::Return(Operand::Integer(0))});
        }
        let mut entry=std::mem::take(&mut builder.allocations);entry.append(&mut builder.blocks[0].instructions);builder.blocks[0].instructions=entry;
        builder.blocks.retain(|block|!block.instructions.is_empty());
        let names:std::collections::HashSet<_>=builder.blocks.iter().map(|b|b.name.as_str()).collect();
        for block in &builder.blocks {
            for (index,instruction) in block.instructions.iter().enumerate() {
                let terminal=matches!(instruction,Instruction::Return(_)|Instruction::ReturnVoid|Instruction::Branch{..}|Instruction::Jump(_));
                if terminal!=(index+1==block.instructions.len()){return Err("invalid basic block terminator".into());}
                match instruction {
                    Instruction::Jump(target) if !names.contains(target.as_str())=>return Err("missing jump target".into()),
                    Instruction::Branch{then_target,else_target,..} if !names.contains(then_target.as_str())||!names.contains(else_target.as_str())=>return Err("missing branch target".into()),
                    _=>{},
                }
            }
        }
        Ok(FunctionIr{name:ast.name,parameters,parameter_types,returns_void,blocks:builder.blocks})
    }
}
impl TryFrom<Function> for Program {
    type Error=String;
    fn try_from(ast:Function)->Result<Self,String> {Self::try_from(CompilationUnit{items:vec![TopLevel::Function(ast)]})}
}
impl TryFrom<CompilationUnit> for Program {
    type Error=String;
    fn try_from(unit:CompilationUnit)->Result<Self,String> {
        let mut signatures=libraries();let library_count=signatures.len();
        let mut symbols:HashMap<String,Binding>=signatures.iter().enumerate().map(|(id,s)|(s.name.clone(),Binding::Function(id))).collect();
        let mut program=Self{functions:Vec::new(),globals:Vec::new(),declarations:Vec::new()};
        for item in unit.items {
            match item {
                TopLevel::Function(function)=>{
                    if symbols.contains_key(&function.name){return Err(format!("duplicate or reserved global: {}",function.name));}
                    let context=Builder::new(symbols.clone(),signatures.clone(),false);
                    let pointers=context.parameter_shapes(&function)?;
                    let id=signatures.len();signatures.push(Signature{name:function.name.clone(),pointers,returns_void:function.returns_void});
                    // Install before the body to permit direct recursion, but not forward references.
                    symbols.insert(function.name.clone(),Binding::Function(id));
                    program.functions.push(Builder::function(Builder::new(symbols.clone(),signatures.clone(),function.returns_void),function)?);
                }
                TopLevel::Declaration(BlockItem::Declaration{constant,definitions})=>{
                    for definition in definitions {
                        if symbols.contains_key(&definition.name){return Err(format!("duplicate or reserved global: {}",definition.name));}
                        symbols.insert(definition.name.clone(),Binding::PendingConstant);
                        let builder=Builder::new(symbols.clone(),signatures.clone(),false);
                        if !definition.dimensions.is_empty() {
                            let shape=builder.dimension_values(&definition.dimensions)?;
                            let count=element_count(&shape)?;
                            let values=if let Some(initializer)=&definition.initializer {
                                flatten_initializer(&shape,initializer)?.into_iter().map(|value|match value {
                                    Some(expr)=>{builder.validate(&expr,true)?;builder.constant(&expr)},None=>Ok(0)
                                }).collect::<Result<Vec<_>,String>>()?
                            }else if constant{return Err("constant array requires initializer".into());}else{vec![0;count]};
                            let id=program.globals.len();program.globals.push(GlobalData{shape:shape.clone(),values});
                            symbols.insert(definition.name,Binding::Array(ArrayBinding{storage:ArrayStorage::Global(id),shape,readonly:constant}));continue;
                        }
                        let value=if let Some(initializer)=definition.initializer {let expr=initializer.into_scalar()?;builder.validate(&expr,true)?;builder.constant(&expr)?}
                            else if constant {return Err("constant requires initializer".into());}else{0};
                        let binding=if constant {Binding::Constant(value)}else{let id=program.globals.len();program.globals.push(GlobalData{shape:vec![],values:vec![value]});Binding::Global(id)};
                        symbols.insert(definition.name,binding);
                    }
                }
                _=>return Err("invalid top-level declaration".into()),
            }
        }
        let used:std::collections::HashSet<_>=program.functions.iter().flat_map(|f|&f.blocks).flat_map(|b|&b.instructions)
            .filter_map(|i|if let Instruction::Call{name,..}=i{Some(name.as_str())}else{None}).collect();
        program.declarations=signatures[..library_count].iter().filter(|s|used.contains(s.name.as_str())).cloned().collect();
        Ok(program)
    }
}
impl Operand {
    fn text(&self) -> String {match self {Self::Integer(v)=>v.to_string(),Self::Temporary(id)=>format!("%t{id}")}}
    fn load(&self, register: &str, output: &mut String, base: usize) {
        match self {
            Self::Integer(value)=>writeln!(output,"  li {register}, {value}").unwrap(),
            Self::Temporary(id)=>load_stack(output,register,base+id*4),
        }
    }
}
// t3 is reserved as the expanded-address scratch register by the backend.
fn load_stack(output:&mut String,register:&str,offset:usize) {
    if offset<=2047 {writeln!(output,"  lw {register}, {offset}(sp)").unwrap();}
    else {writeln!(output,"  li t3, {offset}\n  add t3, sp, t3\n  lw {register}, 0(t3)").unwrap();}
}
fn store_stack(output:&mut String,register:&str,offset:usize) {
    if offset<=2047 {writeln!(output,"  sw {register}, {offset}(sp)").unwrap();}
    else {writeln!(output,"  li t3, {offset}\n  add t3, sp, t3\n  sw {register}, 0(t3)").unwrap();}
}
impl Program {
    /// Full programs expose only main; source-level names must not interpose libc/runtime symbols.
    pub fn isolate_functions(mut self)->Self {
        let mut prefix="__sysy_function_".to_owned();
        while self.functions.iter().any(|f|f.name.starts_with(&prefix)){prefix.push('_');}
        let names:HashMap<_,_>=self.functions.iter().enumerate().filter(|(_,f)|f.name!="main")
            .map(|(id,f)|(f.name.clone(),format!("{prefix}{id}"))).collect();
        for function in &mut self.functions {
            if let Some(name)=names.get(&function.name){function.name=name.clone();}
            for instruction in function.blocks.iter_mut().flat_map(|b|&mut b.instructions) {
                if let Instruction::Call{name,..}=instruction {if let Some(internal)=names.get(name){*name=internal.clone();}}
            }
        }
        self
    }
    fn global_prefix(&self)->String {
        let mut prefix="__sysy_global_".to_owned();
        while self.functions.iter().any(|f|f.name.starts_with(&prefix)) {prefix.push('_');}
        prefix
    }
    pub fn koopa(&self) -> String {
        let mut output=String::new();let global_prefix=self.global_prefix();
        for (id,value) in self.globals.iter().enumerate(){writeln!(output,"global @{global_prefix}{id} = alloc {}, {}",array_type(&value.shape),aggregate(&value.shape,&value.values)).unwrap();}
        for signature in &self.declarations {
            let parameters=signature.pointers.iter().map(argument_type).collect::<Vec<_>>().join(", ");
            writeln!(output,"decl @{}({}){}",signature.name,parameters,if signature.returns_void {""}else{": i32"}).unwrap();
        }
        for function in &self.functions {
            let parameters=function.parameter_types.iter().enumerate().map(|(id,shape)|format!("%arg{id}: {}",argument_type(shape))).collect::<Vec<_>>().join(", ");
            writeln!(output,"fun @{}({}){} {{",function.name,parameters,if function.returns_void {""}else{": i32"}).unwrap();
            for block in &function.blocks {
                writeln!(output,"%{}:",block.name).unwrap();
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::Allocate { slot }=>writeln!(output,"  %v{slot} = alloc i32").unwrap(),
                        Instruction::AllocateArray{slot,shape}=>writeln!(output,"  %v{slot} = alloc {}",array_type(shape)).unwrap(),
                        Instruction::AllocatePointer{slot,shape}=>writeln!(output,"  %v{slot} = alloc *{}",array_type(shape)).unwrap(),
                        Instruction::ElementPointer{destination,base,index,parameter_step,..}=>{
                            let base=match base{PointerBase::Slot(id)=>format!("%v{id}"),PointerBase::Global(id)=>format!("@{global_prefix}{id}"),PointerBase::Temporary(id)=>format!("%t{id}")};
                            writeln!(output,"  %t{destination} = {} {base}, {}",if *parameter_step{"getptr"}else{"getelemptr"},index.text()).unwrap();
                        }
                        Instruction::IndirectLoad{destination,pointer}=>writeln!(output,"  %t{destination} = load {}",pointer.text()).unwrap(),
                        Instruction::IndirectStore{pointer,value}=>writeln!(output,"  store {}, {}",value.text(),pointer.text()).unwrap(),
                        Instruction::Parameter{slot,index}=>writeln!(output,"  store %arg{index}, %v{slot}").unwrap(),
                        Instruction::GlobalLoad{destination,global}=>writeln!(output,"  %t{destination} = load @{global_prefix}{global}").unwrap(),
                        Instruction::GlobalStore{global,value}=>writeln!(output,"  store {}, @{global_prefix}{global}",value.text()).unwrap(),
                        Instruction::Call{destination,name,arguments}=>{
                            let assignment=destination.map(|id|format!("%t{id} = ")).unwrap_or_default();
                            writeln!(output,"  {assignment}call @{name}({})",arguments.iter().map(Operand::text).collect::<Vec<_>>().join(", ")).unwrap();
                        }
                        Instruction::ReturnVoid=>writeln!(output,"  ret").unwrap(),
                        Instruction::Load { destination, slot }=>writeln!(output,"  %t{destination} = load %v{slot}").unwrap(),
                        Instruction::Store { slot, value }=>writeln!(output,"  store {}, %v{slot}",value.text()).unwrap(),
                        Instruction::Binary { destination, op, lhs, rhs }=>writeln!(output,"  %t{destination} = {} {}, {}",op.koopa(),lhs.text(),rhs.text()).unwrap(),
                        Instruction::Branch{condition,then_target,else_target}=>writeln!(output,"  br {}, %{}, %{}",condition.text(),then_target,else_target).unwrap(),
                        Instruction::Jump(target)=>writeln!(output,"  jump %{target}").unwrap(),
                        Instruction::Return(value)=>writeln!(output,"  ret {}",value.text()).unwrap(),
                    }
                }
            }writeln!(output,"}}").unwrap();
        }
        let globals:std::collections::HashSet<_>=self.functions.iter().map(|f|f.name.as_str()).chain(self.declarations.iter().map(|f|f.name.as_str())).collect();
        let mut renamed=String::new();let chars:Vec<_>=output.chars().collect();let mut i=0;
        while i<chars.len() {
            if chars[i]=='%' {
                renamed.push('%');i+=1;let start=i;
                while i<chars.len() && (chars[i].is_ascii_alphanumeric()||chars[i]=='_'){i+=1;}
                let mut name:String=chars[start..i].iter().collect();
                while globals.contains(name.as_str()){name.push('_');}renamed.push_str(&name);
            }else{renamed.push(chars[i]);i+=1;}
        }
        renamed
    }
    pub fn riscv(&self) -> String {
        let mut output=String::from("  .option norelax\n  .data\n");let global_prefix=self.global_prefix();
        for (id,value) in self.globals.iter().enumerate(){
            writeln!(output,"{global_prefix}{id}:").unwrap();
            if !value.shape.is_empty() && value.values.iter().all(|v|*v==0){writeln!(output,"  .zero {}",value.values.len()*4).unwrap();}
            else{for scalar in &value.values{writeln!(output,"  .word {scalar}").unwrap();}}
        }
        output.push_str("  .text\n");
        let mut branch_id=0;
        for (index, function) in self.functions.iter().enumerate() {
            let instructions=||function.blocks.iter().flat_map(|b|&b.instructions);
            let spill_slots=riscv_spills::slots(function);
            let temporaries=spill_slots.iter().max().map_or(0,|slot|slot+1);
            let calls=instructions().any(|i|matches!(i,Instruction::Call{..}));
            let outgoing=instructions().filter_map(|i|if let Instruction::Call{arguments,..}=i{Some(arguments.len().saturating_sub(8)*4)}else{None}).max().unwrap_or(0);
            let mut cursor=outgoing+temporaries*4;let mut local_offsets=HashMap::new();
            for instruction in instructions(){
                let allocation=match instruction {
                    Instruction::Allocate{slot}|Instruction::AllocatePointer{slot,..}=>Some((*slot,4)),
                    Instruction::AllocateArray{slot,shape}=>Some((*slot,element_count(shape).expect("validated shape")*4)),_=>None,
                };
                if let Some((slot,bytes))=allocation{local_offsets.insert(slot,cursor);cursor+=bytes;}
            }
            // Scalar/pointer slots whose address never escapes can live in
            // callee-saved registers across branches, loops and nested calls.
            // Keep their stack reservations so array addresses remain stable.
            let address_taken:std::collections::HashSet<_>=instructions().filter_map(|i|{
                if let Instruction::ElementPointer{base:PointerBase::Slot(slot),..}=i {Some(*slot)}else{None}
            }).collect();
            let mut promoted=Vec::new();
            for instruction in instructions() {
                if let Instruction::Allocate{slot}|Instruction::AllocatePointer{slot,..}=instruction {
                    if promoted.len()<12 && !address_taken.contains(slot) && !promoted.contains(slot){promoted.push(*slot);}
                }
            }
            let saved_base=cursor;cursor+=promoted.len()*4;
            let promoted_register=|slot:usize|promoted.iter().position(|s|*s==slot);
            let frame=(cursor+if calls{4}else{0}).div_ceil(16)*16;
            let local_offset=|slot:usize|*local_offsets.get(&slot).expect("allocated local slot");
            let temp_offset=|id:usize|outgoing+spill_slots[id]*4;
            let load_operand=|operand:&Operand,register:&str,output:&mut String| {
                match operand {
                    Operand::Integer(value)=>writeln!(output,"  li {register}, {value}").unwrap(),
                    Operand::Temporary(id)=>load_stack(output,register,temp_offset(*id)),
                }
            };
            writeln!(output,"  .globl {}\n{}:",function.name,function.name).unwrap();
            if frame>0 {writeln!(output,"  li t3, {frame}\n  sub sp, sp, t3").unwrap();}
            if calls {writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  sw ra, 0(t3)",frame-4).unwrap();}
            for reg in 0..promoted.len(){store_stack(&mut output,&format!("s{reg}"),saved_base+reg*4);}
            for block in &function.blocks {
                writeln!(output,".L{index}_{}:",block.name).unwrap();
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::Allocate { .. }|Instruction::AllocateArray{..}|Instruction::AllocatePointer{..}=>{},
                        Instruction::ElementPointer{destination,base,index,stride,..}=>{
                            match base {
                                PointerBase::Slot(slot)=>writeln!(output,"  li t3, {}\n  add t0, sp, t3",local_offset(*slot)).unwrap(),
                                PointerBase::Global(global)=>writeln!(output,"  la t0, {global_prefix}{global}").unwrap(),
                                PointerBase::Temporary(id)=>load_operand(&Operand::Temporary(*id),"t0",&mut output),
                            }
                            match index {
                                Operand::Integer(0)=>{},
                                Operand::Integer(value)=>{
                                    let offset=value.wrapping_mul(*stride as i32);
                                    if (-2048..=2047).contains(&offset) {
                                        writeln!(output,"  addi t0, t0, {offset}").unwrap();
                                    } else {
                                        writeln!(output,"  li t1, {offset}\n  add t0, t0, t1").unwrap();
                                    }
                                }
                                _=>{
                                    load_operand(index,"t1",&mut output);
                                    if stride.is_power_of_two() && stride.trailing_zeros()<32 {
                                        writeln!(output,"  slli t1, t1, {}",stride.trailing_zeros()).unwrap();
                                    } else {
                                        writeln!(output,"  li t2, {stride}\n  mul t1, t1, t2").unwrap();
                                    }
                                    writeln!(output,"  add t0, t0, t1").unwrap();
                                }
                            }
                            store_stack(&mut output,"t0",temp_offset(*destination));
                        }
                        Instruction::IndirectLoad{destination,pointer}=>{
                            load_operand(pointer,"t0",&mut output);writeln!(output,"  lw t0, 0(t0)").unwrap();store_stack(&mut output,"t0",temp_offset(*destination));
                        }
                        Instruction::IndirectStore{pointer,value}=>{
                            load_operand(pointer,"t1",&mut output);load_operand(value,"t0",&mut output);writeln!(output,"  sw t0, 0(t1)").unwrap();
                        }
                        Instruction::Parameter{slot,index}=>{
                            if *index<8 {writeln!(output,"  mv t0, a{index}").unwrap();}
                            else {writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  lw t0, 0(t3)",frame+(index-8)*4).unwrap();}
                            if let Some(reg)=promoted_register(*slot){writeln!(output,"  mv s{reg}, t0").unwrap();}
                            else {writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  sw t0, 0(t3)",local_offset(*slot)).unwrap();}
                        }
                        Instruction::GlobalLoad{destination,global}=>{
                            writeln!(output,"  la t3, {global_prefix}{global}\n  lw t0, 0(t3)").unwrap();store_stack(&mut output,"t0",temp_offset(*destination));
                        }
                        Instruction::GlobalStore{global,value}=>{
                            load_operand(value,"t0",&mut output);writeln!(output,"  la t3, {global_prefix}{global}\n  sw t0, 0(t3)").unwrap();
                        }
                        Instruction::Call{destination,name,arguments}=>{
                            for (id,argument) in arguments.iter().enumerate() {
                                if id<8 {load_operand(argument,&format!("a{id}"),&mut output);}
                                else {load_operand(argument,"t0",&mut output);writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  sw t0, 0(t3)",(id-8)*4).unwrap();}
                            }
                            writeln!(output,"  call {name}").unwrap();
                            if let Some(id)=destination {store_stack(&mut output,"a0",temp_offset(*id));}
                        }
                        Instruction::ReturnVoid=>{
                            for reg in 0..promoted.len(){load_stack(&mut output,&format!("s{reg}"),saved_base+reg*4);}
                            if calls {writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  lw ra, 0(t3)",frame-4).unwrap();}
                            if frame>0 {writeln!(output,"  li t3, {frame}\n  add sp, sp, t3").unwrap();}
                            writeln!(output,"  ret").unwrap();
                        }
                        Instruction::Load { destination, slot }=>{
                            if let Some(reg)=promoted_register(*slot){writeln!(output,"  mv t0, s{reg}").unwrap();}
                            else {writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  lw t0, 0(t3)",local_offset(*slot)).unwrap();}
                            store_stack(&mut output,"t0",temp_offset(*destination));
                        }
                        Instruction::Store { slot, value }=>{
                            load_operand(value,"t0",&mut output);
                            if let Some(reg)=promoted_register(*slot){writeln!(output,"  mv s{reg}, t0").unwrap();}
                            else {writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  sw t0, 0(t3)",local_offset(*slot)).unwrap();}
                        }
                        Instruction::Binary { destination, op, lhs, rhs }=>{
                            load_operand(lhs,"t0",&mut output);load_operand(rhs,"t1",&mut output);
                            let code=match op {
                                Op::Add=>"add t0, t0, t1",Op::Sub=>"sub t0, t0, t1",Op::Mul=>"mul t0, t0, t1",Op::Div=>"div t0, t0, t1",Op::Mod=>"rem t0, t0, t1",
                                Op::Lt=>"slt t0, t0, t1",Op::Gt=>"slt t0, t1, t0",Op::Le=>"slt t0, t1, t0\n  xori t0, t0, 1",Op::Ge=>"slt t0, t0, t1\n  xori t0, t0, 1",
                                Op::Eq=>"xor t0, t0, t1\n  seqz t0, t0",Op::Ne=>"xor t0, t0, t1\n  snez t0, t0",Op::And=>"and t0, t0, t1",Op::Or=>"or t0, t0, t1",
                            };
                            writeln!(output,"  {code}").unwrap();store_stack(&mut output,"t0",temp_offset(*destination));
                        }
                        Instruction::Branch{condition,then_target,else_target}=>{
                            load_operand(condition,"t0",&mut output);
                            writeln!(output,"  beqz t0, .Lbranch_skip_{branch_id}\n  la t3, .L{index}_{then_target}\n  jr t3\n.Lbranch_skip_{branch_id}:\n  la t3, .L{index}_{else_target}\n  jr t3").unwrap();branch_id+=1;
                        }
                        Instruction::Jump(target)=>writeln!(output,"  la t3, .L{index}_{target}\n  jr t3").unwrap(),
                        Instruction::Return(value)=>{
                            load_operand(value,"a0",&mut output);
                            for reg in 0..promoted.len(){load_stack(&mut output,&format!("s{reg}"),saved_base+reg*4);}
                            if calls {writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  lw ra, 0(t3)",frame-4).unwrap();}
                            if frame>0 {writeln!(output,"  li t3, {frame}\n  add sp, sp, t3").unwrap();}
                            writeln!(output,"  ret").unwrap();
                        }
                    }
                }
            }
        }output
    }
}
#[cfg(test)]
mod tests {
 #[test] fn temporary_stack_displacement_boundary() {
  for offset in [0,2044,2048,65536] {
   let mut output=String::new();Operand::Temporary(0).load("t1",&mut output,offset);
   store_stack(&mut output,"t0",offset);
   if offset<=2047 {
    assert_eq!(output,format!("  lw t1, {offset}(sp)\n  sw t0, {offset}(sp)\n"));
   }else {
    assert_eq!(output,format!("  li t3, {offset}\n  add t3, sp, t3\n  lw t1, 0(t3)\n  li t3, {offset}\n  add t3, sp, t3\n  sw t0, 0(t3)\n"));
   }
  }
 }
 use super::*;
 #[test]
 fn lower_logical_booleans(){
  let p=Program::try_from(crate::parse("int main(){return 2&&4;}").unwrap()).unwrap();
  assert!(p.koopa().contains("br 2, %sc_rhs"));assert!(p.koopa().contains("ne 4, 0"));
  assert!(!p.koopa().contains(" = and "));
 }
 #[test]
 fn stack_handles_large_offsets(){
  // Keep 900 values live across a block edge so the spill frame really
  // exceeds the immediate range even after block-local slot reuse.
  let mut entry=vec![Instruction::Allocate{slot:0}];
  let mut tail=Vec::new();
  for id in 0..900 {
   entry.push(Instruction::Binary{destination:id,op:Op::Add,lhs:Operand::Integer(id as i32),rhs:Operand::Integer(1)});
   tail.push(Instruction::Store{slot:0,value:Operand::Temporary(id)});
  }
  entry.push(Instruction::Jump("tail".into()));tail.push(Instruction::Return(Operand::Integer(0)));
  let p=Program{functions:vec![FunctionIr{name:"large".into(),parameters:0,parameter_types:vec![],returns_void:false,
   blocks:vec![BasicBlock{name:"entry".into(),instructions:entry},BasicBlock{name:"tail".into(),instructions:tail}]}],globals:vec![],declarations:vec![]};
  let s=p.riscv();assert!(s.contains("li t3, 3596\n  add t3, sp, t3\n  lw t0, 0(t3)"));
  assert!(s.contains("li t3, 3616\n  sub sp, sp, t3"));
 }
 #[test]
 fn long_expression_reuses_dead_spill_slots(){
  let source=format!("int main(){{return {};}}",vec!["1";800].join("+"));
  let p=Program::try_from(crate::parse(&source).unwrap()).unwrap();
  let s=p.riscv();assert!(s.contains("li t3, 16\n  sub sp, sp, t3"));
  assert!(!s.contains("li t3, 3200"));
 }
 #[test]
 fn backend_uses_ir_names_and_values(){
  let p=Program{functions:vec![FunctionIr{name:"answer".into(),parameters:0,parameter_types:vec![],returns_void:false,blocks:vec![BasicBlock{name:"body".into(),instructions:vec![Instruction::Return(Operand::Integer(i32::MIN))]}]}],globals:vec![],declarations:vec![]};
  assert!(p.koopa().contains("fun @answer"));assert!(p.riscv().contains("answer:\n.L0_body:\n  li a0, -2147483648"));
 }
}
