# PKU Lv6：条件控制流与短路（公开自学范围）

实现提交42a31f7df0d29236bfba4e3a7399dc9e54c08bea的[Actions 45022404677](https://github.com/edidada/pku-sysy-rust/actions/runs/45022404677)completed/success。报告归档后的最终HEAD由独立lv6-if.yml再次验证；最终运行与原始证据哈希另存langnote的PKU-Compilers-Lv6-results.json。

按[官方Lv6规范](https://github.com/pku-minic/online-doc/blob/master/docs/lv6-if/README.md)及[短路章节](https://github.com/pku-minic/online-doc/blob/master/docs/lv6-if/short-circuit.md)实现。文档blob分别为43201f731ed8d29c0e6be396007b295e00a4890d、0af5a2d0f776394dafd1c538e6c1dce3eb89983a。递归Stmt解析使else匹配最近未匹配if，条件按零/非零判断；不带块的分支不能直接放声明。

Builder改为多基本块与当前块，每块末尾必须为br/jump/ret且目标存在。分别处理单路return、两路return和汇合，不再使用全函数首return截断。所有alloc入口提升保证指针支配使用点，符号表仍按词法作用域检查两个分支及未执行RHS，死代码错误不会因不执行而漏掉。

&&/||通过sc_rhs/sc_end块、结果槽默认0/1及RHS归一化实现短路；常量求值也短路，但预先验证全部标识符及常量性，因此1||(1/0)可折叠而1||未定义标识符仍报错。RV32使用近距离条件分支配合la/jr转移远目标，所有return恢复统一16字节对齐帧。两个2000语句大分支分别走真假两路，证明大偏移栈与远跳转实际可执行。

官方Lv6 Koopa/RISC-V各8/8；Lv1各7/7、Lv3各28/28、Lv4各14/14、Lv5各7/7回归，Rust十六单元/CLI通过。186 GCC UBSan参考程序在真实RV32/QEMU和官方Koopa→LLVM/native逐项比较完整i32，含27命名入口；RV32核验sp及12保存寄存器，保留同低字节错误负对照。20非法源码在两模式验证诊断及旧输出保留，共40检查；全部旧366声明/折叠、182作用域、185表达式和234常量回归保留。

短路路径另用12原始程序及18个RHS位置作动态对照。先验证原始程序，再仅在测试工件对应RHS入口插入RV32 ebreak或Koopa call @abort()。两条链逐项得到10执行/8跳过：应执行分别以SIGTRAP(-5)/SIGABRT(-6)终止，应跳过正常返回并保持完整结果。不以0&&(1/0)的结果代替路径证明，也不向生产编译器加入探针/文件名专用行为；生产源程序函数调用仍未实现。预期异常关闭core dump。

初版8c052a1/run45022083485的官方和186双链通过，探针确实收到SIGTRAP但脚本多要求stderr文字，因该QEMU静默而失败；42a31f7改核验信号退出状态后全部通过，未删减路径条件。失败日志将和最终工件一起归档。工具镜像按固定摘要，全部tee保留pipefail。

本地只静态检查，未编译运行实验，无新增Python源文件。Ruby/Java/cs61b-fa26跳过。Lv7循环、Lv8函数/全局和Lv9数组仍未实现；这是AI辅助公开自学，未提交校内OJ，不代表整门课程或全目标完成。
