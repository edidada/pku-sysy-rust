//! Memory representation of the Lv1 subset of Koopa IR.
//! Both text and target backends walk the same lowered program.
use crate::Function;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand { Integer(i32) }
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction { Return(Operand) }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock { pub name: String, pub instructions: Vec<Instruction> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionIr { pub name: String, pub blocks: Vec<BasicBlock> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program { pub functions: Vec<FunctionIr> }

impl From<Function> for Program {
    fn from(ast: Function) -> Self {
        Self { functions: vec![FunctionIr { name: ast.name, blocks: vec![BasicBlock {
            name: "entry".into(), instructions: vec![Instruction::Return(Operand::Integer(ast.return_value))],
        }] }] }
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
                        Instruction::Return(Operand::Integer(value)) => writeln!(output,"  ret {value}").unwrap(),
                    }
                }
            }
            writeln!(output,"}}").unwrap();
        }
        output
    }
    pub fn riscv(&self) -> String {
        let mut output=String::from("  .text\n");
        for (index,function) in self.functions.iter().enumerate() {
            writeln!(output,"  .globl {}\n{}:",function.name,function.name).unwrap();
            for block in &function.blocks {
                writeln!(output,".L{index}_{}:",block.name).unwrap();
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::Return(Operand::Integer(value)) => writeln!(output,"  li a0, {value}\n  ret").unwrap(),
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
    fn lowering_preserves_structure() {
        let program=Program::from(Function{name:"main".into(),return_value:42});
        assert_eq!(program.functions[0].blocks[0].instructions,vec![Instruction::Return(Operand::Integer(42))]);
        assert!(program.koopa().contains("ret 42"));
        assert!(program.riscv().contains("li a0, 42"));
    }
    #[test]
    fn backend_reads_ir_not_source_or_fixed_main() {
        let program=Program{functions:vec![FunctionIr{name:"answer".into(),blocks:vec![BasicBlock{name:"body".into(),instructions:vec![Instruction::Return(Operand::Integer(i32::MIN))]}]}]};
        assert!(program.koopa().contains("fun @answer"));
        assert!(program.riscv().contains("answer:\n.L0_body:\n  li a0, -2147483648"));
    }
}
