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

## `parallel for`

Une boucle comptée dont les tours ne se voient pas peut les faire tourner sur plusieurs fils à la fois ([décision 0007](../decisions/0007-threads-over-shared-memory.md)). Chaque tour peut lire tout ce que la boucle ne change pas, et n’écrire que les éléments à son propre indice :

```peras build
export fn brighten(pixels: Array<i32>, out: inout Array<i32>, gain: i32): void {
  parallel for (let i = 0; i < pixels.length; i++) {
    out[i] = pixels[i] * gain;
  }
}
```

Un tour peut appeler une fonction qui ne change rien : qui ne crée ni ne libère de chaîne ou de tableau, n’écrit aucun élément ni aucune variable de module, et n’appelle aucune valeur fonction, pas plus qu’aucune fonction qu’elle appelle. Et il peut écrire une part d’un tableau à lui : dans un tour compté par `b`, `let band = xs.part(b, k)` est les éléments `b * k` à `(b + 1) * k` de `xs`, que le tour lit et écrit comme `band[j]` ; les parts de deux tours ne se chevauchent jamais ([décision 0031](../decisions/0031-useful-parallel-loops.md)). Une image se découpe ainsi en bandes de lignes :

```peras build
fn shade(x: f64, y: f64): f64 {
  return (x * x + y * y).sqrt();
}

export fn draw(pixels: inout Array<f64>, width: i32, height: i32, bands: i32): void {
  const rows = height / bands;
  parallel for (let b = 0; b < bands; b++) {
    let band = pixels.part(b, rows * width);
    for (let r = 0; r < rows; r++) {
      for (let x = 0; x < width; x++) {
        band[r * width + x] = shade(x.toF64(), (b * rows + r).toF64());
      }
    }
  }
}
```

Le compilateur le prouve : un tour qui affecte une variable déclarée hors de la boucle, écrit ailleurs qu’à son indice ou dans ses parts, lit à un autre indice un tableau que la boucle écrit, appelle une fonction qui change quelque chose, crée ou libère une chaîne ou un tableau, ou sort de la boucle par `break` ou `return` est refusé, avec sa raison ([E0241](../errors/E0241.md)). Les résultats sont alors ceux de la boucle exécutée dans l’ordre, bit pour bit. Compilés avec `--threads on`, les tours tournent sur tous les cœurs d’une page qui peut partager sa mémoire, et sur un seul fil ailleurs : voir [Peras et JavaScript](javascript.md#threads). Sans, la boucle tourne comme une autre. <span class="status compiled">compilé</span>

## `for…of`

Parcourt les éléments d’un tableau :

```peras build
export fn total(prices: Array<f64>): f64 {
  let sum = 0.0;
  for (const price of prices) {
    sum += price;
  }
  return sum;
}
```

La boucle relit la longueur à chaque tour. Pendant la boucle, le tableau ne peut ni grandir, ni rétrécir, ni être remplacé ([E0207](../errors/E0207.md)) ; ses éléments peuvent changer.

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
