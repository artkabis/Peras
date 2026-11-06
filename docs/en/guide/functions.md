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

```peras
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

> `inout` and `sink` are checked, not compiled yet; they arrive with the ownership checks ([memory](memory.md)).

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
