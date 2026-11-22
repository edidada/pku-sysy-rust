# PKU Lv5：语句块与作用域（公开自学范围）

实现提交7504191525c9015a74d09cee09e5a528c2823e99的[Actions 44920295381](https://github.com/edidada/pku-sysy-rust/actions/runs/44920295381)completed/success。报告归档后的最终HEAD由独立lv5-block-n-scope.yml再次验证；最终运行和原始证据哈希保存在langnote的PKU-Compilers-Lv5-results.json。

[官方Lv5规范](https://github.com/pku-minic/online-doc/blob/master/docs/lv5-block-n-scope/README.md)，文档blob4aad5b2d9304876d7004b3503596afda2b4a378b。从已验Lv4 3f69c28实现递归Block、普通表达式语句和空语句；标识符后窥视单等号区分赋值与相等比较等表达式。

作用域栈从内向外查找，同层重名拒绝，退出块恢复外层；常量初始化先以PendingConstant占位，阻止误读同名外层常量。可写性取最近绑定，内层常量不能错误地写到外层变量；内层变量可遮蔽外层常量，退出后外层常量属性恢复。局部槽整函数唯一，兄弟/嵌套声明不会相互覆盖存储。

首return在任何嵌套层都会记录全函数终止位置，后续源码继续语义检查但不生成终止后的IR。表达式语句求值后丢弃结果，空语句不生成存储。此处尚无条件控制流，后续Lv6必须改为按基本块管理终止，不能沿用单个全函数截断点代替CFG。

官方Lv5 Koopa/RISC-V各7/7，Lv1各7/7、Lv3各28/28、Lv4各14/14回归。Rust十四单元/CLI通过。新增182 GCC UBSan参考程序分别执行真实RV32/QEMU和官方Koopa→LLVM/native，完整i32逐项一致，26命名入口；包括128层嵌套、800兄弟块、常量/变量互相遮蔽、退出恢复、表达式/空语句和嵌套return。RV32核验sp与全部12保存寄存器，两链均拒绝同低字节错误负对照。20非法源码在两模式核验诊断与旧输出文件保留，共40检查。旧366声明/折叠、185表达式及234常量回归保留，官方测试和期望未修改。

工具镜像固定sha256:5bcecbbe34b6c0874c4fe0ec7ef9b5a77e8733644045dbee260bda20623c4e70，tee管道保留pipefail。全部编译/实验仅在Actions，本地只做静态检查，没有新增Python源文件。Ruby/Java/cs61b-fa26继续跳过。Lv6条件/短路、Lv7循环和后续函数/全局/数组未实现；这是AI辅助公开自学，不提交校内OJ，也不代表整门课程完成。
