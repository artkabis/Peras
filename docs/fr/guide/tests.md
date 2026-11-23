# Tests

Un test s’écrit dans le fichier qu’il teste, à côté du code, en Peras ([décision 0040](../decisions/0040-tests.md)) :

```peras build
fn digit(c: String): i32 {
  return "0123456789".indexOf(c);
}

export fn parsePort(text: String): Result<u16, String> {
  if (text.length == 0) {
    return Err("empty");
  }
  let n = 0;
  for (const c of text.split("")) {
    const d = digit(c);
    if (d < 0) {
      return Err(`not a digit: ${c}`);
    }
    n = n * 10 + d;
    if (n > 65535) {
      return Err("too large");
    }
  }
  return Ok(n.toU16());
}

test "a port is read" {
  expect(parsePort("80").unwrapOr(0)).toBe(80);
  expect(parsePort("65535")).toBeOk();
}

test "a bad text is refused" {
  expect(parsePort("65536")).toBeErr();
  expect(parsePort("")).toBeErr();
}

test "a digit is found" {
  expect(digit("7")).toBe(7);
  assert(digit("x") < 0);
}
```

Un bloc `test "nom" { … }` atteint chaque fonction de son fichier, exportée ou non : `digit` est testée alors que JavaScript ne la voit jamais.

## Ce que dit un test

- `expect(a).toBe(b)` et `expect(a).toEqual(b)` échouent quand `a != b`, en disant où et les deux valeurs : `port.peras:24: expected 80, got 0`.
- `expect(r).toBeOk()` et `expect(r).toBeErr()` testent un `Result`.
- `assert(cond)` échoue quand `cond` est faux, et `assertEqual(a, b)` quand `a != b`, en disant où et ce qui était affirmé.

Un test s’arrête à son premier échec. Une valeur qu’un gabarit ne sait pas encore écrire, un tableau ou une structure, ne peut pas être montrée dans un échec : comparez-la, `expect(xs == [1, 2]).toBe(true)`, en attendant (constat F227).

## Les lancer

`vitest-peras` lance les tests des fichiers `.peras` et `.apeiron` avec Vitest :

```ts
// vitest.config.ts
import perasTests from "vitest-peras";

export default {
  plugins: [perasTests()],
  test: { include: ["src/**/*.peras", "src/**/*.apeiron"] },
};
```

Chaque bloc est un test de Vitest, lancé dans le mode du module : les tests d’un module possédé tournent en WebAssembly, sur l’allocateur de débogage, qui attrape chaque débordement et chaque fuite ; ceux d’un module géré en JavaScript. `perasc build --tests` est ce qu’appelle le plugin : chaque bloc une fonction exportée, les noms des tests à côté du module.

## Ce qu’ils coûtent

Rien au module livré : un build sans `--tests` lit le fichier comme si ses tests n’y étaient pas, et donne les mêmes octets. `perasc check` les lit de la même façon, si bien qu’une erreur dans un test est signalée par le build de test, pas par `check`.
