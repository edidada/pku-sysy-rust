# PKU Lv2：内存IR与目标代码生成

从已验收Lv1分支继续，按官方Lv2文档建立自有内存Koopa子集：Program→FunctionIr→BasicBlock→Return(Integer)。AST先降低到IR，Koopa文本和RISC-V后端共同遍历该IR；不再直接从AST拼接汇编。语言范围仍为一个main返回一个非负32位整数。

官方文档允许自行定义内存IR表示：https://github.com/pku-minic/online-doc/blob/master/docs/lv2-code-gen/processing-ir.md 。官方要求 https://github.com/pku-minic/online-doc/blob/master/docs/lv2-code-gen/testing.md 为`autotest -riscv -s lv1`，同时回归Koopa模式。

独立工作流lv2-code-gen.yml执行Rust单元/CLI、官方两模式以及三进制边界/确定性散列值的真实RV32汇编链接/QEMU。运行时_start将a0与32位预期值比较后再exit，避免仅低8位退出码掩盖错误。无本地编译运行，无新增Python源文件，不提交校内OJ。当前远端待验；后续表达式/变量等等级未实现。
