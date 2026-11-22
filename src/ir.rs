//! Memory IR shared by the Koopa and RV32IM backends.
use crate::{BinaryOp, BlockItem, Expr, Function, UnaryOp};
use std::{collections::HashMap, fmt::Write};
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
pub struct FunctionIr { pub name: String, pub blocks: Vec<BasicBlock> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program { pub functions: Vec<FunctionIr> }
#[derive(Clone, Copy)]
enum Binding { PendingConstant, Constant(i32), Variable(usize) }
struct Builder {
    blocks: Vec<BasicBlock>, active: Option<usize>, allocations: Vec<Instruction>,
    next: usize, next_slot: usize, scopes: Vec<HashMap<String, Binding>>, has_return: bool, loops: Vec<(usize,usize)>,
}
impl Builder {
    fn lookup(&self,name:&str)->Option<Binding> {self.scopes.iter().rev().find_map(|scope|scope.get(name)).copied()}
    fn current(&mut self)->&mut HashMap<String,Binding> {self.scopes.last_mut().expect("function scope exists")}
    fn emit(&mut self,instruction:Instruction) {
        if let Some(block)=self.active {
            if matches!(instruction,Instruction::Allocate{..}) {self.allocations.push(instruction);}
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
            Expr::Number(_)=>Ok(()),
            Expr::Variable(name)=>match self.lookup(name) {
                Some(Binding::Constant(_))=>Ok(()),Some(Binding::Variable(_)) if !constant=>Ok(()),
                _=>Err(format!("undefined, initializing or nonconstant identifier: {name}")),
            },
            Expr::Unary(_,value)=>self.validate(value,constant),
            Expr::Binary(_,lhs,rhs)=>{self.validate(lhs,constant)?;self.validate(rhs,constant)},
        }
    }
    fn constant(&self, expr: &Expr) -> Result<i32, String> {
        let bad=||"undefined or overflowing constant arithmetic".to_owned();
        match expr {
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
            Expr::Number(value)=>Operand::Integer(value),
            Expr::Variable(name)=>match self.lookup(&name) {
                Some(Binding::Constant(value))=>Operand::Integer(value),
                Some(Binding::Variable(slot))=>self.load_slot(slot),
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
            BlockItem::Expression(Some(expr))=>{self.lower(expr)?;},BlockItem::Expression(None)=>{},
            BlockItem::Declaration{constant,definitions}=>{
                for definition in definitions {
                    if self.current().contains_key(&definition.name){return Err(format!("duplicate identifier: {}",definition.name));}
                    if constant {
                        self.current().insert(definition.name.clone(),Binding::PendingConstant);
                        let expr=definition.initializer.as_ref().ok_or("missing constant initializer")?;
                        self.validate(expr,true)?;let value=self.constant(expr)?;
                        self.current().insert(definition.name,Binding::Constant(value));
                    } else {
                        let slot=self.allocate();self.current().insert(definition.name,Binding::Variable(slot));
                        if let Some(expr)=definition.initializer{let value=self.lower(expr)?;self.emit(Instruction::Store{slot,value});}
                    }
                }
            }
            BlockItem::Assignment{name,value}=>{
                let slot=match self.lookup(&name){Some(Binding::Variable(slot))=>slot,_=>return Err(format!("undefined or constant assignment: {name}"))};
                let value=self.lower(value)?;self.emit(Instruction::Store{slot,value});
            }
            BlockItem::Return(expr)=>{
                let value=self.lower(expr)?;self.emit(Instruction::Return(value));self.active=None;self.has_return=true;
            }
        }Ok(())
    }
}
impl TryFrom<Function> for Program {
    type Error=String;
    fn try_from(ast:Function)->Result<Self,String> {
        let mut builder=Builder{blocks:vec![BasicBlock{name:"entry".into(),instructions:Vec::new()}],active:Some(0),allocations:Vec::new(),next:0,next_slot:0,scopes:vec![HashMap::new()],has_return:false,loops:Vec::new()};
        for item in ast.items{builder.item(item)?;}
        if !builder.has_return{return Err("function requires a return statement".into());}
        // Falling off an int function is undefined; keep its reachable block well formed.
        if builder.active.is_some(){builder.emit(Instruction::Return(Operand::Integer(0)));}
        let mut entry=std::mem::take(&mut builder.allocations);entry.append(&mut builder.blocks[0].instructions);builder.blocks[0].instructions=entry;
        builder.blocks.retain(|block|!block.instructions.is_empty());
        let names:std::collections::HashSet<_>=builder.blocks.iter().map(|b|b.name.as_str()).collect();
        for block in &builder.blocks {
            for (index,instruction) in block.instructions.iter().enumerate() {
                let terminal=matches!(instruction,Instruction::Return(_)|Instruction::Branch{..}|Instruction::Jump(_));
                if terminal!=(index+1==block.instructions.len()){return Err("invalid basic block terminator".into());}
                match instruction {
                    Instruction::Jump(target) if !names.contains(target.as_str())=>return Err("missing jump target".into()),
                    Instruction::Branch{then_target,else_target,..} if !names.contains(then_target.as_str())||!names.contains(else_target.as_str())=>return Err("missing branch target".into()),
                    _=>{},
                }
            }
        }
        Ok(Self{functions:vec![FunctionIr{name:ast.name,blocks:builder.blocks}]})
    }
}
impl Operand {
    fn text(&self) -> String {match self {Self::Integer(v)=>v.to_string(),Self::Temporary(id)=>format!("%t{id}")}}
    fn load(&self, register: &str, output: &mut String) {
        match self {
            Self::Integer(value)=>writeln!(output,"  li {register}, {value}").unwrap(),
            Self::Temporary(id)=>writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  lw {register}, 0(t3)",id*4).unwrap(),
        }
    }
}
impl Program {
    pub fn koopa(&self) -> String {
        let mut output=String::new();
        for function in &self.functions {
            writeln!(output,"fun @{}(): i32 {{",function.name).unwrap();
            for block in &function.blocks {
                writeln!(output,"%{}:",block.name).unwrap();
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::Allocate { slot }=>writeln!(output,"  %v{slot} = alloc i32").unwrap(),
                        Instruction::Load { destination, slot }=>writeln!(output,"  %t{destination} = load %v{slot}").unwrap(),
                        Instruction::Store { slot, value }=>writeln!(output,"  store {}, %v{slot}",value.text()).unwrap(),
                        Instruction::Binary { destination, op, lhs, rhs }=>writeln!(output,"  %t{destination} = {} {}, {}",op.koopa(),lhs.text(),rhs.text()).unwrap(),
                        Instruction::Branch{condition,then_target,else_target}=>writeln!(output,"  br {}, %{}, %{}",condition.text(),then_target,else_target).unwrap(),
                        Instruction::Jump(target)=>writeln!(output,"  jump %{target}").unwrap(),
                        Instruction::Return(value)=>writeln!(output,"  ret {}",value.text()).unwrap(),
                    }
                }
            }writeln!(output,"}}").unwrap();
        }output
    }
    pub fn riscv(&self) -> String {
        let mut output=String::from("  .text\n");
        let mut branch_id=0;
        for (index, function) in self.functions.iter().enumerate() {
            let instructions=||function.blocks.iter().flat_map(|b|&b.instructions);
            let temporaries=instructions().filter_map(|i|match i {Instruction::Binary { destination, .. }|Instruction::Load { destination, .. }=>Some(destination+1),_=>None}).max().unwrap_or(0);
            let locals=instructions().filter_map(|i|match i {Instruction::Allocate { slot }=>Some(slot+1),_=>None}).max().unwrap_or(0);
            let frame=((temporaries+locals)*4).div_ceil(16)*16;
            writeln!(output,"  .globl {}\n{}:",function.name,function.name).unwrap();
            if frame>0 {writeln!(output,"  li t3, {frame}\n  sub sp, sp, t3").unwrap();}
            for block in &function.blocks {
                writeln!(output,".L{index}_{}:",block.name).unwrap();
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::Allocate { .. }=>{},
                        Instruction::Load { destination, slot }=>{
                            writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  lw t0, 0(t3)\n  li t3, {}\n  add t3, sp, t3\n  sw t0, 0(t3)",(temporaries+slot)*4,destination*4).unwrap();
                        }
                        Instruction::Store { slot, value }=>{
                            value.load("t0",&mut output);
                            writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  sw t0, 0(t3)",(temporaries+slot)*4).unwrap();
                        }
                        Instruction::Binary { destination, op, lhs, rhs }=>{
                            lhs.load("t0",&mut output);rhs.load("t1",&mut output);
                            let code=match op {
                                Op::Add=>"add t0, t0, t1",Op::Sub=>"sub t0, t0, t1",Op::Mul=>"mul t0, t0, t1",Op::Div=>"div t0, t0, t1",Op::Mod=>"rem t0, t0, t1",
                                Op::Lt=>"slt t0, t0, t1",Op::Gt=>"slt t0, t1, t0",Op::Le=>"slt t0, t1, t0\n  xori t0, t0, 1",Op::Ge=>"slt t0, t0, t1\n  xori t0, t0, 1",
                                Op::Eq=>"xor t0, t0, t1\n  seqz t0, t0",Op::Ne=>"xor t0, t0, t1\n  snez t0, t0",Op::And=>"and t0, t0, t1",Op::Or=>"or t0, t0, t1",
                            };
                            writeln!(output,"  {code}\n  li t3, {}\n  add t3, sp, t3\n  sw t0, 0(t3)",destination*4).unwrap();
                        }
                        Instruction::Branch{condition,then_target,else_target}=>{
                            condition.load("t0",&mut output);
                            writeln!(output,"  beqz t0, .Lbranch_skip_{branch_id}\n  la t3, .L{index}_{then_target}\n  jr t3\n.Lbranch_skip_{branch_id}:\n  la t3, .L{index}_{else_target}\n  jr t3").unwrap();branch_id+=1;
                        }
                        Instruction::Jump(target)=>writeln!(output,"  la t3, .L{index}_{target}\n  jr t3").unwrap(),
                        Instruction::Return(value)=>{
                            value.load("a0",&mut output);
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
 use super::*;
 #[test]
 fn lower_logical_booleans(){
  let p=Program::try_from(crate::parse("int main(){return 2&&4;}").unwrap()).unwrap();
  assert!(p.koopa().contains("br 2, %sc_rhs"));assert!(p.koopa().contains("ne 4, 0"));
  assert!(!p.koopa().contains(" = and "));
 }
 #[test]
 fn stack_handles_large_offsets(){
  let source=format!("int main(){{return {};}}",vec!["1";800].join("+"));
  let p=Program::try_from(crate::parse(&source).unwrap()).unwrap();
  let s=p.riscv();assert!(s.contains("li t3, 3200"));assert!(s.contains("li t3, 3192"));
 }
 #[test]
 fn backend_uses_ir_names_and_values(){
  let p=Program{functions:vec![FunctionIr{name:"answer".into(),blocks:vec![BasicBlock{name:"body".into(),instructions:vec![Instruction::Return(Operand::Integer(i32::MIN))]}]}]};
  assert!(p.koopa().contains("fun @answer"));assert!(p.riscv().contains("answer:\n.L0_body:\n  li a0, -2147483648"));
 }
}
