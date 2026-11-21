# PKU 编译实践Lv3：表达式（公开自学范围）

验证提交afec76c36532966a6296efbb8a28a4605c7dc6dd的[Actions 44917457107](https://github.com/edidada/pku-sysy-rust/actions/runs/44917457107)completed/success。报告归档与工具镜像固定后的最终HEAD由独立lv3-expr.yml重新验证；最终结果和原始工件另保存在langnote的PKU-Compilers-Lv3-results.json。

按[官方Lv3规范](https://github.com/pku-minic/online-doc/blob/master/docs/lv3-expr/README.md)，实现一元+/-/!、括号、算术/关系/相等/逻辑二元运算，按C优先级与左结合解析。本级不要求短路，有符号溢出等UB不纳入正常值承诺。

最长匹配词法与优先级爬升构建AST，后序降低到共享内存IR。逻辑左右值各自ne0归一化后and/or，避免2&&4按位运算成0。两个后端均遍历同一IR，不把一个后端文本当另一个的实现。

RV32IM每临时值一个栈槽，帧16字节对齐；偏移/帧大小用li加寄存器计算，避免12位立即数限制。比较使用有符号slt和eq/ne组合，除法向零截断、余数用rem。800项长表达式实际覆盖3200字节栈帧。

官方Lv1两模式各7/7、Lv3 Koopa/RISC-V各28/28；Rust七单元与CLI通过。既有234完整32位a0常量检查保留。新增185表达式（25定向、160固定种子生成）由GCC UBSan提供参考，RV32 QEMU比较完整a0、sp恢复和s0哨兵；同185份Koopa再经官方koopac→LLVM llc→clang执行，由独立C harness比较完整i32，两个后端均拒绝同低8位错误负对照。随机每个子表达式限制在996以内，避免除零和有符号溢出；所有输入/参考/IR与结果JSON保留。官方原测试和期望未修改。

运行管道显式set -euo pipefail，避免tee掩盖失败。compiler-dev镜像固定为本次两个job实际相同摘要sha256:5bcecbbe34b6c0874c4fe0ec7ef9b5a77e8733644045dbee260bda20623c4e70。

本地仅静态读写，没有编译/执行实验，也没有新增Python源文件。未向校内OJ提交；当前仅单main返回表达式，不支持变量、赋值、控制流或函数调用，Lv4及后续仍未实现，不能声明完整SysY或整门课程完成。
