# 北大编译实践 Lv1 main 实验报告

官方起点 pku-minic/sysy-cargo-template。模板原来忽略输入并始终返回0；本分支 edidada/lv1-main 按官方 Lv1 规范实现词法分析、AST 和生成，输入不同常量得到相应返回值。语法规约来源：https://github.com/pku-minic/online-doc/blob/master/docs/lv1-main/README.md。

手写字节扫描器处理空白、行注释和块注释；注释作为 token 分隔符，不能拼接关键字或数字。整数支持十进制、八进制和十六进制，严格限制0..2147483647并拒绝非法数字及未闭合注释。语法要求单一 main 和单一 return，构造 Function AST 后输出 Koopa IR 或 RISC-V 汇编。错误输入不先打开输出文件，避免覆盖现有结果。去掉只用于原演示映射的 lazy_static 依赖。

独立 lv1-main.yml 执行 Rust 单元测试（4096个数的三种进制交叉核对）、真实 CLI 文件测试，并使用课程官方 maxxing/compiler-dev 的 autotest -koopa -s lv1。编译和执行只在 GitHub Actions，本地未编译；不向校内 OJ 提交。此分支只代表 Lv1，不声称已实现完整 SysY。

官方 autotest 测试失败后仍可能返回0，因此额外去掉 ANSI 色码并核对最终 PASSED (N/N)，同时要求 N>0，避免把进程正常退出误认为全部测试通过。
