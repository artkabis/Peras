# Keywords

These words cannot name a variable, a function or a type.

## In use

| Keyword              | Purpose                                                                           |
| -------------------- | --------------------------------------------------------------------------------- |
| `fn`                 | declares a function                                                               |
| `struct`             | declares a structure                                                              |
| `enum`, `match`      | declare an enumeration, take a value apart ([enumerations](../guide/enums.md))    |
| `let`, `const`       | declare a variable, a constant                                                    |
| `export`             | makes a function or a structure visible from JavaScript                           |
| `return`             | leaves a function, with its result                                                |
| `if`, `else`         | chooses between two branches                                                      |
| `while`, `for`, `of` | loops; `of` is contextual, in `for…of` only                                       |
| `parallel`           | contextual, before `for`: [`parallel for`](../guide/control-flow.md#parallel-for) |
| `break`, `continue`  | leave a loop, go to its next turn                                                 |
| `true`, `false`      | the two `bool` values                                                             |
| `inout`, `sink`      | parameter conventions ([functions](../guide/functions.md#parameter-conventions))  |
| `impl`, `interface`  | methods, interfaces ([methods and interfaces](../guide/methods.md))               |
| `type`               | type aliases ([types](types.md#type-aliases))                                     |

## Reserved

Reserved for features that are designed but not built yet; using them as a name is an error today, so that code written now keeps working when they arrive.

| Keyword          | For                                                     |
| ---------------- | ------------------------------------------------------- |
| `trait`          | kept free: Peras calls them interfaces                  |
| `class`          | refused on purpose: Peras uses structures and functions |
| `import`         | modules                                                 |
| `async`, `await` | asynchronous code, in managed mode                      |
