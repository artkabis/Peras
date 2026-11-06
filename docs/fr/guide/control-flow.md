# Contrôle du flux

## `if` et `else`

La condition est un `bool`, entre parenthèses ; les branches sont des blocs ou des instructions seules.

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

Peras n’a pas d’opérateur ternaire `a ? b : c` : un `if` dit la même chose, et se lit de la même façon dans tous les langages.

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

La boucle en trois parties de C et de JavaScript : une initialisation, une condition, une mise à jour.

```peras build
export fn sum_to(n: i32): i64 {
  let total: i64 = 0;
  for (let i = 1; i <= n; i++) {
    total += i.toI64();
  }
  return total;
}
```

## `for…of`

Parcourt les éléments d’un tableau :

```peras
export fn total(prices: Array<f64>): f64 {
  let sum = 0.0;
  for (const price of prices) {
    sum += price;
  }
  return sum;
}
```

> `for…of` est vérifié, pas encore compilé : il arrive avec les tableaux.

## `break` et `continue`

`break` quitte la boucle la plus intérieure, `continue` passe à son tour suivant (après la mise à jour d’un `for`).

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

## Incréments et affectations composées

`x++`, `x--`, `+=`, `-=`, `*=`, `/=`, `%=` sont des instructions, pas des expressions : `a = b++` n’existe pas, si bien que l’ordre des effets n’est jamais en doute.

Ensuite : [les structures](structures.md).
