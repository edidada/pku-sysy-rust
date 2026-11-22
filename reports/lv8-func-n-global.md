# Lv8 函数和全局变量（进行中，未验收）

从Lv7最终8ce1d52建立edidada/lv8-func-n-global。首步扩展编译单元AST、全局声明与函数顺序、int/void、任意数量标量形参/实参、嵌套调用和裸return语法。parse_compilation_unit负责对象片段语法，validate_entry另行表达完整SysY恰有一个无参int main约束；兼容旧单函数parse入口。

当前IR明确拒绝尚未实现的调用/参数/void，compile尚走单函数入口；多函数及全局语义、递归、库函数、超过8参ABI、运行时双链均待实现。新工作流当前仅验证前端里程碑和旧单元/CLI，绿色也不代表Lv8官方验收。接下来必须接入完整编译单元降低、全局/签名符号表、call/return IR及实际ABI，扩展该独立工作流为完整官方Lv8和旧回归，再动态验收。

完整程序入口契约尚未接入CLI，须保留旧命名单函数对象测试的显式接口，不能悄悄删去回归。库函数必须实际官方运行时验证。全部编译实验仅Actions，本地只静态读写，无新增Python源文件；Ruby/Java/cs61b-fa26跳过。
