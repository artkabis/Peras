# Fonctions

## Déclarer une fonction

`fn` déclare une fonction, avec des paramètres typés et un type de résultat. `export` la rend appelable depuis JavaScript.

```peras build
fn square(x: f64): f64 {
  return x * x;
}

export fn distance(x: f64, y: f64): f64 {
  return (square(x) + square(y)).sqrt();
}
```

Une fonction renvoie une valeur sur chaque chemin : le vérificateur refuse celle qui pourrait se terminer sans valeur ([E0219](../errors/E0219.md)). Une fonction sans résultat renvoie `void`.

```peras error E0219
export fn sign(n: i32): i32 {
  if (n > 0) {
    return 1;
  }
}
```

Le type du résultat est toujours écrit ([E0220](../errors/E0220.md)) : une signature est la documentation d’une fonction, et ne doit pas changer parce que son corps a changé.

## Appels

Les arguments sont évalués de gauche à droite, toujours, y compris quand le compilateur déplace un appel pour optimiser. Le nombre et le type des arguments sont vérifiés ([E0214](../errors/E0214.md), [E0210](../errors/E0210.md)).

```peras error E0214
fn add(a: i32, b: i32): i32 {
  return a + b;
}

export fn three(): i32 {
  return add(1);
}
```

## Récursion

Une fonction peut s’appeler elle-même. L’optimiseur transforme les formes récursives courantes en boucles, et déroule les autres sur quelques niveaux, si bien que la récursion coûte peu :

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

## Conventions de passage

Un paramètre est en lecture seule par défaut. Deux conventions en disent plus, et le compilateur y tient l’appelant :

| Convention   | Sens                                                                                                                    |
| ------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `x: T`       | la fonction lit `x` ; l’appelant le garde                                                                               |
| `x: inout T` | la fonction peut modifier la valeur de l’appelant ; l’appelant écrit `&valeur`, et aucun alias n’existe pendant l’appel |
| `x: sink T`  | la fonction prend `x` ; l’appelant ne peut plus l’utiliser                                                              |

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

`inout` marche sur tout : un tableau ou une chaîne est modifié sur place, et la fonction rend ses nombres à l’appelant, qui les retrouve dans sa variable après l’appel. Pendant l’appel, rien d’autre dans l’instruction ne peut lire cette variable ([E0207](../errors/E0207.md)) : `n += bump(&n)` est refusé, puisque le résultat dépendrait de l’ordre d’évaluation. Les règles sont sur la page [mémoire](memory.md).

Depuis JavaScript, une structure passée `inout` est un objet dont les champs sont mis à jour après l’appel. Un nombre, une option ou une chaîne ne peuvent pas être passés `inout` depuis JavaScript, qui les passe par valeur ; une telle fonction reste appelable depuis Peras.

## Les fonctions comme valeurs

Une fonction est une valeur d’un type fonction, `(i32) => i32`, qui s’écrit partout où un type s’écrit : un paramètre, un résultat, une variable, un champ, un élément de tableau, une option ([décision 0025](../decisions/0025-moved-fields-and-function-values.md)). Une fonction du module en est une par son nom ; une fonction fléchée en est une quand elle ne capture rien. Une valeur d’un type fonction s’appelle comme une fonction.

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

- **Les arguments sont prêtés**, et le résultat appartient à l’appelant : un type fonction ne dit ni `sink` ni `inout`, si bien qu’une fonction avec un tel paramètre n’en est pas une valeur ([E0210](../errors/E0210.md)), pas plus qu’une fonction générique.
- **`xs.map(twice)`** donne une fonction à `map` et aux autres par son nom, qui l’appelle directement, comme le ferait `(x) => twice(x)`.
- **Une fonction fléchée qui capture une variable**, `(x) => x + k`, n’est une valeur que dans un module managed ; dans un module owned, elle s’écrit sur place, comme argument de `map` et des autres ([E0900](../errors/E0900.md)).
- **JavaScript** ne reçoit ni ne donne jamais une fonction d’un module owned : une fonction exportée qui en prend ou en rend une est refusée ([E0238](../errors/E0238.md)).
- En WebAssembly, une valeur fonction est la position de la fonction dans une table du module, et un appel à travers elle un appel indirect, qui coûte à peu près ce que coûte un appel direct.

## Méthodes intégrées

Les nombres ont des méthodes, appelées avec un point : conversions, `abs`, `sqrt`, arithmétique qui reboucle ou qui vérifie. La [référence des méthodes](../reference/methods.md) les liste toutes.

```peras build
export fn hypotenuse(a: i32, b: i32): f64 {
  const x = a.toF64();
  const y = b.toF64();
  return (x * x + y * y).sqrt().floor();
}
```

Ensuite : [le contrôle du flux](control-flow.md).
