//! Memory IR shared by the Koopa text and RV32IM backends.
use crate::{Function,Expr,UnaryOp,BinaryOp};
use std::fmt::Write;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum Operand { Integer(i32), Temporary(usize) }
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Op { Add,Sub,Mul,Div,Mod,Lt,Gt,Le,Ge,Eq,Ne,And,Or }
impl Op {
    fn koopa(self)->&'static str {match self {Self::Add=>"add",Self::Sub=>"sub",Self::Mul=>"mul",Self::Div=>"div",Self::Mod=>"mod",Self::Lt=>"lt",Self::Gt=>"gt",Self::Le=>"le",Self::Ge=>"ge",Self::Eq=>"eq",Self::Ne=>"ne",Self::And=>"and",Self::Or=>"or"}}
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum Instruction { Binary{destination:usize,op:Op,lhs:Operand,rhs:Operand}, Return(Operand) }
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct BasicBlock {pub name:String,pub instructions:Vec<Instruction>}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct FunctionIr {pub name:String,pub blocks:Vec<BasicBlock>}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Program {pub functions:Vec<FunctionIr>}
struct Builder {instructions:Vec<Instruction>,next:usize}
impl Builder {
    fn binary(&mut self,op:Op,lhs:Operand,rhs:Operand)->Operand {
        let destination=self.next;self.next+=1;
        self.instructions.push(Instruction::Binary{destination,op,lhs,rhs});Operand::Temporary(destination)
    }
    fn lower(&mut self,expr:Expr)->Operand {
        match expr {
            Expr::Number(value)=>Operand::Integer(value),
            Expr::Unary(op,value)=>{let rhs=self.lower(*value);match op {UnaryOp::Plus=>rhs,UnaryOp::Minus=>self.binary(Op::Sub,Operand::Integer(0),rhs),UnaryOp::Not=>self.binary(Op::Eq,rhs,Operand::Integer(0))}},
            Expr::Binary(op,lhs,rhs)=>{
                let lhs=self.lower(*lhs);let rhs=self.lower(*rhs);
                if matches!(op,BinaryOp::And|BinaryOp::Or) {
                    let lhs=self.binary(Op::Ne,lhs,Operand::Integer(0));
                    let rhs=self.binary(Op::Ne,rhs,Operand::Integer(0));
                    self.binary(if op==BinaryOp::And {Op::And}else{Op::Or},lhs,rhs)
                } else {
                    let op=match op {BinaryOp::Add=>Op::Add,BinaryOp::Sub=>Op::Sub,BinaryOp::Mul=>Op::Mul,BinaryOp::Div=>Op::Div,BinaryOp::Mod=>Op::Mod,BinaryOp::Lt=>Op::Lt,BinaryOp::Gt=>Op::Gt,BinaryOp::Le=>Op::Le,BinaryOp::Ge=>Op::Ge,BinaryOp::Eq=>Op::Eq,BinaryOp::Ne=>Op::Ne,_=>unreachable!()};
                    self.binary(op,lhs,rhs)
                }
            }
        }
    }
}
impl From<Function> for Program {
    fn from(ast:Function)->Self {
        let mut builder=Builder{instructions:Vec::new(),next:0};let value=builder.lower(ast.expression);
        builder.instructions.push(Instruction::Return(value));
        Self{functions:vec![FunctionIr{name:ast.name,blocks:vec![BasicBlock{name:"entry".into(),instructions:builder.instructions}]}]}
    }
}
impl Operand {
    fn text(&self)->String {match self {Self::Integer(v)=>v.to_string(),Self::Temporary(id)=>format!("%t{id}")}}
    fn load(&self,register:&str,output:&mut String) {
        match self {
            Self::Integer(value)=>writeln!(output,"  li {register}, {value}").unwrap(),
            Self::Temporary(id)=>{writeln!(output,"  li t3, {}\n  add t3, sp, t3\n  lw {register}, 0(t3)",id*4).unwrap();}
        }
    }
}
impl Program {
    pub fn koopa(&self)->String {
        let mut output=String::new();
        for function in &self.functions {
            writeln!(output,"fun @{}(): i32 {{",function.name).unwrap();
            for block in &function.blocks {
                writeln!(output,"%{}:",block.name).unwrap();
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::Binary{destination,op,lhs,rhs}=>writeln!(output,"  %t{destination} = {} {}, {}",op.koopa(),lhs.text(),rhs.text()).unwrap(),
                        Instruction::Return(value)=>writeln!(output,"  ret {}",value.text()).unwrap(),
                    }
                }
            }
            writeln!(output,"}}").unwrap();
        }
        output
    }
    pub fn riscv(&self)->String {
        let mut output=String::from("  .text\n");
        for (index,function) in self.functions.iter().enumerate() {
            let slots=function.blocks.iter().flat_map(|b|&b.instructions).filter_map(|i|match i {Instruction::Binary{destination,..}=>Some(destination+1),_=>None}).max().unwrap_or(0);
            let frame=(slots*4).div_ceil(16)*16;
            writeln!(output,"  .globl {}\n{}:",function.name,function.name).unwrap();
            if frame>0 {writeln!(output,"  li t3, {frame}\n  sub sp, sp, t3").unwrap();}
            for block in &function.blocks {
                writeln!(output,".L{index}_{}:",block.name).unwrap();
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::Binary{destination,op,lhs,rhs}=>{
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
        }
        output
    }
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn lower_logical_booleans(){
  let p=Program::from(crate::parse("int main(){return 2&&4;}").unwrap());
  assert!(p.koopa().contains("%t0 = ne 2, 0"));assert!(p.koopa().contains("%t2 = and %t0, %t1"));
 }
 #[test]
 fn stack_handles_large_offsets(){
  let source=format!("int main(){{return {};}}",vec!["1";800].join("+"));
  let p=Program::from(crate::parse(&source).unwrap());
  let s=p.riscv();assert!(s.contains("li t3, 3200"));assert!(s.contains("li t3, 3192"));
 }
 #[test]
 fn backend_uses_ir_names_and_values(){
  let p=Program{functions:vec![FunctionIr{name:"answer".into(),blocks:vec![BasicBlock{name:"body".into(),instructions:vec![Instruction::Return(Operand::Integer(i32::MIN))]}]}]};
  assert!(p.koopa().contains("fun @answer"));assert!(p.riscv().contains("answer:\n.L0_body:\n  li a0, -2147483648"));
 }
}
