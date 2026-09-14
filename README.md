# Monkey：用 Rust 学习实现解释器

这是一个用 Rust 编写的 Monkey 语言解释器学习项目。代码参考 Thorsten Ball 的 [《Writing An Interpreter In Go》](https://interpreterbook.com/)，宏系统部分参考作者的补充章节 [《The Lost Chapter: A Macro System For Monkey》](https://interpreterbook.com/lost/)。原书使用 Go，本项目在跟随书中实现思路的过程中，使用 Rust 重新实现并调整部分设计。

## 项目目的

**这个项目的主要目的是学习 Rust，而不是构建生产级解释器。**

通过实现词法分析、语法分析、求值和宏展开，练习 Rust 的所有权与借用、枚举与模式匹配、闭包、错误处理和单元测试，并理解解释器各个部分如何协作。

本项目不是原书代码的逐行翻译。例如，AST 使用 `Program`、`Statement` 和 `Expression` 等具体类型及枚举组织，环境通过 `Rc<RefCell<Environment>>` 共享，AST 改写使用可变引用进行原地遍历。实现会随着学习过程继续调整，不保证与原书的所有行为完全一致。

## 项目结构

主要代码位于 `src/`，单元测试放在相应模块的 `#[cfg(test)]` 中。

| 文件 | 职责 |
| --- | --- |
| `main.rs` | 程序入口，选择交互或标准输入模式，处理退出信号 |
| `lib.rs` | 模块声明及库入口 |
| `token.rs` | Token 类型和关键字定义 |
| `lexer.rs` | 词法分析，将源码转换为 Token |
| `parser.rs` | 语法分析，使用 Pratt 解析构建 AST |
| `ast.rs` | AST 节点定义、格式化以及原地遍历与改写 |
| `object.rs` | 运行时对象，包括整数、函数、数组、哈希、Quote 和宏 |
| `environment.rs` | 名称绑定、嵌套作用域以及函数和宏捕获的环境 |
| `evaluator.rs` | AST 求值，以及 `quote` / `unquote` 的处理 |
| `builtins.rs` | 内置函数 |
| `macro_expansion.rs` | 收集并移除宏定义，将宏调用展开为 AST |
| `repl.rs` | 读取输入、串联解释流程，提供多行输入和行编辑功能 |

REPL 中的主要处理流程：

```text
源码 → Lexer → Parser → AST → 收集宏定义 → 展开宏调用 → 求值 → 输出
```

宏环境和普通运行时环境分别保存，宏展开处理的是语法树，普通求值处理的是展开后的程序。

## 运行与测试

安装支持 Rust 2024 edition 的 Rust 工具链后，在项目根目录运行：

```sh
cargo run
```

进入 REPL 后，可以输入 Monkey 代码，例如：

```text
let add = fn(left, right) { left + right; };
add(2, 3);
```

运行单元测试：

```sh
cargo test
```

## 参考资料

- [Writing An Interpreter In Go — 书籍官网](https://interpreterbook.com/)
- [The Lost Chapter: A Macro System For Monkey — 宏系统补充章节](https://interpreterbook.com/lost/)

Monkey 语言及原始解释器的设计来自原书；本仓库用于记录 Rust 实践和学习过程。
