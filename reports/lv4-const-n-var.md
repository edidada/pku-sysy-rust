# PKU Lv4：常量、变量与赋值

依据https://github.com/pku-minic/online-doc/blob/master/docs/lv4-const-n-var/README.md，在已验收Lv3 cd24f08上独立实现平坦块的声明序列、逗号多定义、变量引用和赋值。常量编译期求值，符号表区分常量与变量，拒绝重名、先用后定义、常量赋值和非常量的常量初始化。变量声明分配存储，初始化与赋值分别产生store，引用产生load。常量表达式溢出/除零给出诊断，未初始化读取不承诺确定值。

两个后端消费共享alloc/load/store/二元/return IR。RV32局部槽与临时槽不重叠，帧16字节对齐且支持大偏移。首return后的源码仍作语义检查，但不在终止指令后生成IR。

独立lv4-const-n-var.yml仅在Actions编译/运行：保留Lv1/Lv3官方回归和全部旧测试，新增官方Lv4两模式；181 GCC UBSan参考程序分别走RV32/QEMU与官方Koopa/LLVM链，包括800局部变量。比较完整i32、sp及12个callee-saved寄存器，同低字节错误负例必须拒绝；另13非法程序两模式检查诊断与输出文件保留。结果以远端最终提交终态及原始工件为准，当前待验收。

这是AI辅助公开自学，不提交校内OJ。不支持嵌套块、分支、循环、函数调用或数组，Lv5及后续仍未实现。无本地编译/实验，无新增Python源文件；Ruby/Java及cs61b-fa26继续跳过。
