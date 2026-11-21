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
    Return(Operand),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock { pub name: String, pub instructions: Vec<Instruction> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionIr { pub name: String, pub blocks: Vec<BasicBlock> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program { pub functions: Vec<FunctionIr> }
#[derive(Clone, Copy)]
enum Binding { Constant(i32), Variable(usize) }
struct Builder {
    instructions: Vec<Instruction>, next: usize, next_slot: usize,
    symbols: HashMap<String, Binding>,
}
impl Builder {
    fn binary(&mut self, op: Op, lhs: Operand, rhs: Operand) -> Operand {
        let destination=self.next; self.next+=1;
        self.instructions.push(Instruction::Binary { destination, op, lhs, rhs });
        Operand::Temporary(destination)
    }
    fn constant(&self, expr: &Expr) -> Result<i32, String> {
        let bad=||"undefined or overflowing constant arithmetic".to_owned();
        match expr {
            Expr::Number(value)=>Ok(*value),
            Expr::Variable(name)=>match self.symbols.get(name) {
                Some(Binding::Constant(value))=>Ok(*value),
                _=>Err(format!("constant expression needs an earlier constant: {name}")),
            },
            Expr::Unary(op, value)=>{
                let value=self.constant(value)?;
                match op { UnaryOp::Plus=>Ok(value),UnaryOp::Minus=>value.checked_neg().ok_or_else(bad),UnaryOp::Not=>Ok(i32::from(value==0)) }
            }
            Expr::Binary(op, lhs, rhs)=>{
                let a=self.constant(lhs)?;let b=self.constant(rhs)?;
                match op {
                    BinaryOp::Add=>a.checked_add(b).ok_or_else(bad),BinaryOp::Sub=>a.checked_sub(b).ok_or_else(bad),
                    BinaryOp::Mul=>a.checked_mul(b).ok_or_else(bad),BinaryOp::Div=>a.checked_div(b).ok_or_else(bad),BinaryOp::Mod=>a.checked_rem(b).ok_or_else(bad),
                    BinaryOp::Lt=>Ok(i32::from(a<b)),BinaryOp::Gt=>Ok(i32::from(a>b)),BinaryOp::Le=>Ok(i32::from(a<=b)),BinaryOp::Ge=>Ok(i32::from(a>=b)),
                    BinaryOp::Eq=>Ok(i32::from(a==b)),BinaryOp::Ne=>Ok(i32::from(a!=b)),BinaryOp::And=>Ok(i32::from(a!=0 && b!=0)),BinaryOp::Or=>Ok(i32::from(a!=0 || b!=0)),
                }
            }
        }
    }
    fn lower(&mut self, expr: Expr) -> Result<Operand, String> {
        Ok(match expr {
            Expr::Number(value)=>Operand::Integer(value),
            Expr::Variable(name)=>match self.symbols.get(&name).copied() {
                Some(Binding::Constant(value))=>Operand::Integer(value),
                Some(Binding::Variable(slot))=>{
                    let destination=self.next;self.next+=1;
                    self.instructions.push(Instruction::Load { destination, slot });Operand::Temporary(destination)
                }
                None=>return Err(format!("undefined identifier: {name}")),
            },
            Expr::Unary(op, value)=>{
                let rhs=self.lower(*value)?;
                match op {UnaryOp::Plus=>rhs,UnaryOp::Minus=>self.binary(Op::Sub,Operand::Integer(0),rhs),UnaryOp::Not=>self.binary(Op::Eq,rhs,Operand::Integer(0))}
            }
            Expr::Binary(op, lhs, rhs)=>{
                let lhs=self.lower(*lhs)?;let rhs=self.lower(*rhs)?;
                if matches!(op,BinaryOp::And|BinaryOp::Or) {
                    let lhs=self.binary(Op::Ne,lhs,Operand::Integer(0));
                    let rhs=self.binary(Op::Ne,rhs,Operand::Integer(0));
                    self.binary(if op==BinaryOp::And {Op::And} else {Op::Or},lhs,rhs)
                } else {
                    let op=match op {
                        BinaryOp::Add=>Op::Add,BinaryOp::Sub=>Op::Sub,BinaryOp::Mul=>Op::Mul,BinaryOp::Div=>Op::Div,BinaryOp::Mod=>Op::Mod,
                        BinaryOp::Lt=>Op::Lt,BinaryOp::Gt=>Op::Gt,BinaryOp::Le=>Op::Le,BinaryOp::Ge=>Op::Ge,BinaryOp::Eq=>Op::Eq,BinaryOp::Ne=>Op::Ne,
                        _=>unreachable!(),
                    };self.binary(op,lhs,rhs)
                }
            }
        })
    }
    fn item(&mut self, item: BlockItem) -> Result<(), String> {
        match item {
            BlockItem::Declaration { constant, definitions }=>{
                for definition in definitions {
                    if self.symbols.contains_key(&definition.name) {return Err(format!("duplicate identifier: {}",definition.name));}
                    if constant {
                        let value=self.constant(definition.initializer.as_ref().ok_or("missing constant initializer")?)?;
                        self.symbols.insert(definition.name,Binding::Constant(value));
                    } else {
                        let slot=self.next_slot;self.next_slot+=1;
                        self.symbols.insert(definition.name,Binding::Variable(slot));
                        self.instructions.push(Instruction::Allocate { slot });
                        if let Some(expr)=definition.initializer {
                            let value=self.lower(expr)?;self.instructions.push(Instruction::Store { slot, value });
                        }
                    }
                }
            }
            BlockItem::Assignment { name, value }=>{
                let slot=match self.symbols.get(&name) {
                    Some(Binding::Variable(slot))=>*slot,
                    Some(Binding::Constant(_))=>return Err(format!("cannot assign to constant: {name}")),
                    None=>return Err(format!("undefined identifier: {name}")),
                };
                let value=self.lower(value)?;self.instructions.push(Instruction::Store { slot, value });
            }
            BlockItem::Return(expr)=>{
                let value=self.lower(expr)?;self.instructions.push(Instruction::Return(value));
            }
        }Ok(())
    }
}
impl TryFrom<Function> for Program {
    type Error=String;
    fn try_from(ast: Function) -> Result<Self, String> {
        let mut builder=Builder { instructions:Vec::new(),next:0,next_slot:0,symbols:HashMap::new() };
        let mut terminator=None;
        for item in ast.items {
            // Validate even unreachable source items, but do not emit after a terminator.
            builder.item(item)?;
            if terminator.is_none() && matches!(builder.instructions.last(),Some(Instruction::Return(_))) {
                terminator=Some(builder.instructions.len());
            }
        }
        builder.instructions.truncate(terminator.ok_or("main requires a return statement")?);
        Ok(Self { functions:vec![FunctionIr { name:ast.name,blocks:vec![BasicBlock { name:"entry".into(),instructions:builder.instructions }] }] })
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
                        Instruction::Return(value)=>writeln!(output,"  ret {}",value.text()).unwrap(),
                    }
                }
            }writeln!(output,"}}").unwrap();
        }output
    }
    pub fn riscv(&self) -> String {
        let mut output=String::from("  .text\n");
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
  assert!(p.koopa().contains("%t0 = ne 2, 0"));assert!(p.koopa().contains("%t2 = and %t0, %t1"));
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
