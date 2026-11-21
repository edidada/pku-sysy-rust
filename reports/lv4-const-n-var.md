# PKU Lv4：常量、变量与赋值（公开自学范围）

实现验证提交2caf901579f70b8ddfa6540f6388bb90954c7048的[Actions 44919193317](https://github.com/edidada/pku-sysy-rust/actions/runs/44919193317)completed/success。报告归档后的最终HEAD由独立lv4-const-n-var.yml再次验证；最终运行/证据哈希与源码快照保存在langnote的PKU-Compilers-Lv4-results.json。

[官方Lv4规范](https://github.com/pku-minic/online-doc/blob/master/docs/lv4-const-n-var/README.md)文档blob为0ab12f56163b9c745f030a25d059d3a108a31c6c。在已验Lv3 cd24f08上实现平坦块的声明序列、逗号多定义、标识符表达式及赋值。函数名按FuncDef的IDENT解析，支持任意合法单函数名；源程序内函数调用仍属后续等级，不冒称已支持。

符号表区分常量和变量：常量初始化只能引用先前常量并在编译期求值；变量分配存储，引用load、初始化/赋值store。拒绝同块重名、先用后定义和常量赋值，允许局部标识符main。常量算术用检查运算诊断溢出/除零；运行期未定义行为不承诺结果，未初始化变量不被擅自赋零。

两后端消费共享alloc/load/store/二元/return IR。RV32局部槽与临时槽不重叠，帧16字节对齐，通过li和寄存器寻址处理大偏移。首return之后仍作源码语义检查，但不生成终止指令之后的IR。

验证包括官方Lv4 Koopa/RISC-V各14/14（Lv1各7/7、Lv3各28/28回归），Rust十一单元/CLI，旧234常量和185表达式回归；新增366程序由GCC UBSan独立给参考，分别执行RV32/QEMU与官方Koopa→LLVM/native，比较完整i32。181声明/赋值程序覆盖多定义次序、main标识符、return之后语句和800局部变量；另185表达式放入常量声明，要求IR无运行期算术/存取并直接ret正确字面量。每7例采用answer_N命名入口；RV32还核验sp及12个callee-saved寄存器，两个执行链均拒绝同低字节错误负例。13非法程序在两输出模式核验诊断及旧输出文件保留。原官方测试未改。

审计过程：f052ac4/run44918430211已通过初版181声明与14官方用例；9f8c918/run44918812396补185常量折叠后通过；最终语法审计纠正继承的main固定函数名限制，旧错误负例改为真正非法的关键字函数名并加正例，2caf901全部通过。Docker工具镜像固定为sha256:5bcecbbe34b6c0874c4fe0ec7ef9b5a77e8733644045dbee260bda20623c4e70，所有tee管道保留pipefail。

这是AI辅助公开自学，不提交校内OJ。不支持嵌套块、分支、循环、源程序函数调用或数组，Lv5及后续仍未实现。本地只做静态语法/格式检查，没有编译或运行实验，没有新增Python源文件；Ruby/Java及cs61b-fa26继续跳过。
