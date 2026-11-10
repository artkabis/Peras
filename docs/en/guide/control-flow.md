# Control flow

## `if` and `else`

The condition is a `bool`, in parentheses; the branches are blocks or single statements.

```peras build
export fn clamp(x: i32, low: i32, high: i32): i32 {
  if (x < low) {
    return low;
  } else if (x > high) {
    return high;
  }
  return x;
}
```

Peras has no ternary operator `a ? b : c`: an `if` says the same, and reads the same in every language.

## `while`

```peras build
export fn digits(n: i64): i32 {
  let count = 1;
  let rest = n / 10;
  while (rest != 0) {
    count++;
    rest = rest / 10;
  }
  return count;
}
```

## `for`

The three-part loop of C and JavaScript: an initialization, a condition, an update.

```peras build
export fn sum_to(n: i32): i64 {
  let total: i64 = 0;
  for (let i = 1; i <= n; i++) {
    total += i.toI64();
  }
  return total;
}
```

## `parallel for`

A counted loop whose turns do not see each other can run them on several threads at once ([decision 0007](../decisions/0007-threads-over-shared-memory.md)). Each turn may read anything the loop does not change, and write only the elements at its own index:

```peras build
export fn brighten(pixels: Array<i32>, out: inout Array<i32>, gain: i32): void {
  parallel for (let i = 0; i < pixels.length; i++) {
    out[i] = pixels[i] * gain;
  }
}
```

The compiler proves it: a turn that assigns a variable declared outside the loop, reads at another index an array the loop writes, calls a function, makes or releases a string or an array, or leaves the loop with `break` or `return` is refused ([E0241](../errors/E0241.md)). The results are then those of the loop run in order, bit for bit. Built with `--threads on`, the turns run on every core of a page that can share memory, and on one thread elsewhere: see [Peras and JavaScript](javascript.md#threads). Built without, the loop runs as any other. <span class="status compiled">compiled</span>

## `for…of`

Walks the elements of an array:

```peras build
export fn total(prices: Array<f64>): f64 {
  let sum = 0.0;
  for (const price of prices) {
    sum += price;
  }
  return sum;
}
```

The loop reads the length again at each turn. During the loop, the array cannot grow, shrink or be replaced ([E0207](../errors/E0207.md)); its elements can change.

## `break` and `continue`

`break` leaves the innermost loop, `continue` goes to its next turn (after the update of a `for`).

```peras build
export fn first_multiple(of: i32, above: i32): i32 {
  let n = above + 1;
  while (true) {
    if (n % of == 0) {
      break;
    }
    n++;
  }
  return n;
}
```

```peras build
export fn odd_sum(n: i32): i32 {
  let sum = 0;
  for (let i = 0; i < n; i++) {
    if (i % 2 == 0) {
      continue;
    }
    sum += i;
  }
  return sum;
}
```

## Increments and compound assignments

`x++`, `x--`, `+=`, `-=`, `*=`, `/=`, `%=` are statements, not expressions: `a = b++` does not exist, so the order of side effects is never in doubt.

Next: [structures](structures.md).
