# Functions

## Declaring a function

`fn` declares a function, with typed parameters and a result type. `export` makes it callable from JavaScript.

```peras build
fn square(x: f64): f64 {
  return x * x;
}

export fn distance(x: f64, y: f64): f64 {
  return (square(x) + square(y)).sqrt();
}
```

A function returns on every path: the checker refuses one that could end without a value ([E0219](../errors/E0219.md)). A function with no result returns `void`.

```peras error E0219
export fn sign(n: i32): i32 {
  if (n > 0) {
    return 1;
  }
}
```

The result type is always written ([E0220](../errors/E0220.md)): a signature is the documentation of a function, and must not change because its body did.

## Calls

Arguments are evaluated from left to right, always, including when the compiler moves a call to optimize it. The number and types of arguments are checked ([E0214](../errors/E0214.md), [E0210](../errors/E0210.md)).

```peras error E0214
fn add(a: i32, b: i32): i32 {
  return a + b;
}

export fn three(): i32 {
  return add(1);
}
```

## Recursion

A function can call itself. The optimizer turns the common recursive shapes into loops, and unrolls the rest a few levels, so recursion costs little:

```peras build
export fn fib(n: i32): i32 {
  if (n < 2) {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

export fn gcd(a: i64, b: i64): i64 {
  if (b == 0) {
    return a;
  }
  return gcd(b, a % b);
}
```

## Parameter conventions

A parameter is read-only by default. Two conventions say more, and the compiler holds the caller to them:

| Convention   | Meaning                                                                                                          |
| ------------ | ---------------------------------------------------------------------------------------------------------------- |
| `x: T`       | the function reads `x`; the caller keeps it                                                                      |
| `x: inout T` | the function may modify the caller's value; the caller writes `&value` and no other alias exists during the call |
| `x: sink T`  | the function takes `x`; the caller can no longer use it                                                          |

```peras build
struct Counter {
  value: i32;
}

fn bump(c: inout Counter): void {
  c.value += 1;
}

export fn twice(): i32 {
  let c = Counter(0);
  bump(&c);
  bump(&c);
  return c.value;
}
```

`inout` works on everything: an array or a string is changed in place, and the function gives its numbers back to the caller, who finds them in its variable after the call. During the call, nothing else in the statement may read that variable ([E0207](../errors/E0207.md)): `n += bump(&n)` is refused, since it would depend on the order of evaluation. The rules are on the [memory](memory.md) page.

From JavaScript, a structure passed `inout` is an object whose fields are updated after the call. A number, an option or a string cannot be passed `inout` from JavaScript, which passes them by value; such a function can still be called from Peras.

## Functions as values

A function is a value of a function type, `(i32) => i32`, written wherever a type is: a parameter, a result, a variable, a field, an element of an array, an option ([decision 0025](../decisions/0025-moved-fields-and-function-values.md)). A function of the module is one by its name, and an arrow function is one wherever it is not given in place to `map` and the others. A value of a function type is called as a function is, and copied freely.

```peras build
fn twice(x: i32): i32 {
  return x * 2;
}

fn apply(f: (i32) => i32, x: i32): i32 {
  return f(x);
}

export fn applied(n: i32): i32 {
  const steps: Array<(i32) => i32> = [twice, (x: i32): i32 => x + 1];
  let total = apply(twice, n);
  for (let i = 0; i < steps.length; i++) {
    total += (steps[i])(n);
  }
  return total;
}
```

- **The arguments are lent**, and the result is the caller's: a function type says no `sink` nor `inout`, so a function with such a parameter is not a value of one ([E0210](../errors/E0210.md)), nor is a generic function.
- **`xs.map(twice)`** gives a function to `map` and the others by its name, which calls it directly, as `(x) => twice(x)` would.
- **An arrow function that captures**, `(x) => x + k`, is a closure ([decision 0026](../decisions/0026-closures.md)). In an owned module, it copies what it captures when it is made: a `const` or a parameter, which cannot change after; a `let` or an `inout` parameter is refused ([E0251](../errors/E0251.md)). A number or a boolean is copied into its environment. A string, an array, a map, a set or a function value is lent when the closure cannot escape: given in place as an argument, or kept in a `const` only called or given as an argument, while what it captures does not move. Otherwise it is copied, and the environment owns the copy ([decision 0027](../decisions/0027-closures-over-the-heap.md)). Either way the program is the same: the compiler chooses. A managed module captures by reference, as JavaScript does.
- **JavaScript** never receives nor gives a function of an owned module: an exported function that takes or gives one is refused ([E0238](../errors/E0238.md)).
- In WebAssembly, a function value is its environment: the position of its function in a table of the module, and the values it captured. A copy counts one more holder, and the last to let go frees it; a function that captures nothing has a static environment, which costs no allocation. A call through a value is an indirect call.

A closure keeps what it captured, and its copies share it:

```peras build
fn makeAdder(k: i32): (i32) => i32 {
  return (x: i32): i32 => x + k;
}

export fn added(k: i32, n: i32): i32 {
  const add = makeAdder(k);
  const same = add;
  return add(n) + same(1);
}
```

## Built-in methods

Numbers have methods, called with a dot: conversions, `abs`, `sqrt`, wrapping and checked arithmetic. The [method reference](../reference/methods.md) lists them all.

```peras build
export fn hypotenuse(a: i32, b: i32): f64 {
  const x = a.toF64();
  const y = b.toF64();
  return (x * x + y * y).sqrt().floor();
}
```

Next: [control flow](control-flow.md).
