//! Conservative spill-slot reuse: cross-block values retain dedicated slots.
use super::{FunctionIr, Instruction, Operand, PointerBase};

fn definition(i: &Instruction) -> Option<usize> {
    match i {
        Instruction::Binary { destination, .. }
        | Instruction::Load { destination, .. }
        | Instruction::GlobalLoad { destination, .. }
        | Instruction::ElementPointer { destination, .. }
        | Instruction::IndirectLoad { destination, .. } => Some(*destination),
        Instruction::Call { destination, .. } => *destination,
        _ => None,
    }
}

pub(super) fn uses(i: &Instruction) -> Vec<usize> {
    let mut result = Vec::new();
    let mut operand = |v: &Operand| {
        if let Operand::Temporary(id) = v { result.push(*id); }
    };
    match i {
        Instruction::Binary { lhs, rhs, .. } => { operand(lhs); operand(rhs); }
        Instruction::Store { value, .. } | Instruction::GlobalStore { value, .. }
        | Instruction::Return(value) => operand(value),
        Instruction::Branch { condition, .. } => operand(condition),
        Instruction::Call { arguments, .. } => { for value in arguments { operand(value); } }
        Instruction::IndirectLoad { pointer, .. } => operand(pointer),
        Instruction::IndirectStore { pointer, value } => { operand(pointer); operand(value); }
        Instruction::ElementPointer { base, index, .. } => {
            operand(index);
            if let PointerBase::Temporary(id) = base { result.push(*id); }
        }
        _ => {}
    }
    result
}

pub(super) fn slots(function: &FunctionIr) -> Vec<usize> {
    let count = function.blocks.iter().flat_map(|b| &b.instructions)
        .filter_map(definition).max().map_or(0, |id| id + 1);
    let mut definitions = vec![None; count];
    let mut last = vec![0; count];
    let mut shared = vec![false; count];
    for (block, body) in function.blocks.iter().enumerate() {
        for (position, instruction) in body.instructions.iter().enumerate() {
            if let Some(id) = definition(instruction) {
                definitions[id] = Some((block, position));
                last[id] = position;
            }
        }
    }
    for (block, body) in function.blocks.iter().enumerate() {
        for (position, instruction) in body.instructions.iter().enumerate() {
            for id in uses(instruction) {
                match definitions[id] {
                    Some((origin, start)) if origin == block && position > start => {
                        last[id] = last[id].max(position);
                    }
                    _ => shared[id] = true,
                }
            }
        }
    }
    let mut result = vec![0; count];
    let mut reserved = 0;
    for id in 0..count {
        if shared[id] || definitions[id].is_none() {
            result[id] = reserved;
            reserved += 1;
        }
    }
    // Block-local intervals cannot survive the terminator. Reuse the same
    // pool in every block; never overlap an operand's last use with a def.
    for block in 0..function.blocks.len() {
        let mut intervals: Vec<_> = (0..count).filter_map(|id| {
            definitions[id].filter(|(owner, _)| *owner == block && !shared[id])
                .map(|(_, start)| (start, last[id], id))
        }).collect();
        intervals.sort_unstable();
        let mut ends = Vec::new();
        for (start, end, id) in intervals {
            let slot = match ends.iter().position(|last_end| *last_end < start) {
                Some(slot) => slot,
                None => { ends.push(0); ends.len() - 1 }
            };
            ends[slot] = end;
            result[id] = reserved + slot;
        }
    }
    result
}

/// t4..t6 are not used by instruction selection. Only keep block-local
/// intervals that do not cross a call; caller-saved registers cannot hold
/// values through either generated functions or the external SysY runtime.
pub(super) fn registers(function: &FunctionIr) -> Vec<Option<usize>> {
    let count = function.blocks.iter().flat_map(|b| &b.instructions)
        .filter_map(definition).max().map_or(0, |id| id + 1);
    let mut definitions = vec![None; count];
    let mut last = vec![0; count];
    let mut shared = vec![false; count];
    for (block, body) in function.blocks.iter().enumerate() {
        for (position, instruction) in body.instructions.iter().enumerate() {
            if let Some(id) = definition(instruction) {
                definitions[id] = Some((block, position));
                last[id] = position;
            }
        }
    }
    for (block, body) in function.blocks.iter().enumerate() {
        for (position, instruction) in body.instructions.iter().enumerate() {
            for id in uses(instruction) {
                match definitions[id] {
                    Some((owner, start)) if owner == block && position > start => {
                        last[id] = last[id].max(position);
                    }
                    _ => shared[id] = true,
                }
            }
        }
    }
    let mut result = vec![None; count];
    for (block, body) in function.blocks.iter().enumerate() {
        let calls: Vec<_> = body.instructions.iter().enumerate()
            .filter_map(|(p, i)| matches!(i, Instruction::Call { .. }).then_some(p)).collect();
        let mut intervals: Vec<_> = (0..count).filter_map(|id| {
            definitions[id].filter(|(owner, _)| *owner == block && !shared[id])
                .filter(|(_, start)| !calls.iter().any(|p| *start < *p && *p <= last[id]))
                .map(|(_, start)| (start, last[id], id))
        }).collect();
        intervals.sort_unstable();
        let mut ends = [None; 3];
        for (start, end, id) in intervals {
            if let Some(reg) = ends.iter().position(|p| p.is_none_or(|p| p < start)) {
                ends[reg] = Some(end);
                result[id] = Some(reg + 4);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BasicBlock, Op};
    #[test]
    fn registers_preserve_calls_cross_block_values_and_pressure() {
        let t = Operand::Temporary;
        let bin = |destination, lhs, rhs| Instruction::Binary {destination, op: Op::Add, lhs, rhs};
        let f = FunctionIr {
            name: "register_test".into(), parameters: 0, parameter_types: vec![], returns_void: false,
            blocks: vec![BasicBlock { name: "entry".into(), instructions: vec![
                bin(0, Operand::Integer(1), Operand::Integer(2)),
                Instruction::Call { destination: Some(1), name: "getint".into(), arguments: vec![] },
                bin(2, t(0), t(1)),
                bin(3, Operand::Integer(3), Operand::Integer(4)),
                bin(4, Operand::Integer(5), Operand::Integer(6)),
                bin(5, Operand::Integer(7), Operand::Integer(8)),
                bin(6, t(3), t(4)), bin(7, t(5), t(6)),
                Instruction::Jump("next".into()),
            ]}, BasicBlock { name: "next".into(), instructions: vec![
                bin(8, t(2), t(7)), Instruction::Return(t(8)),
            ]}],
        };
        let map = registers(&f);
        assert_eq!(map[0], None, "live through call");
        assert!(map[1].is_some(), "call result is produced after clobbers");
        assert_eq!(map[2], None, "live across block edge");
        assert_eq!(map[7], None, "live across block edge");
        assert_ne!(map[3], map[4]);
        assert_ne!(map[3], map[5]);
        assert_ne!(map[4], map[5]);
        assert_eq!(map[6], None, "all three inputs still live at definition");
        assert!(map[8].is_some(), "register pool resets in next block");
    }
    #[test]
    fn preserves_cross_block_values_and_overlapping_operands() {
        let t = Operand::Temporary;
        let bin = |id, lhs, rhs| Instruction::Binary {
            destination: id, op: Op::Add, lhs, rhs,
        };
        let f = FunctionIr {
            name: "spill_test".into(), parameters: 0, parameter_types: vec![], returns_void: false,
            blocks: vec![
                BasicBlock { name: "entry".into(), instructions: vec![
                    bin(0, Operand::Integer(1), Operand::Integer(2)),
                    bin(1, Operand::Integer(3), Operand::Integer(4)),
                    bin(2, t(0), t(1)),
                    bin(3, Operand::Integer(5), Operand::Integer(6)),
                    Instruction::Jump("next".into()),
                ] },
                BasicBlock { name: "next".into(), instructions: vec![
                    bin(4, t(2), t(3)), Instruction::Return(t(4)),
                ] },
            ],
        };
        let map = slots(&f);
        assert_ne!(map[0], map[1]);
        assert_ne!(map[2], map[3]);
        assert_ne!(map[2], map[4]);
        assert_ne!(map[3], map[4]);
        assert_eq!(map[0], map[4]);
    }
}
