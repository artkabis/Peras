# Premiers pas

Peras n’est pas encore publié : on compile le compilateur depuis le dépôt. Il faut [rustup](https://rustup.rs), Node 26 et pnpm 12.

## Compiler le compilateur

```sh
git clone https://github.com/artkabis/Peras.git
cd Peras
cargo build --release -p perasc
./target/release/perasc --version
```

`perasc` est un seul programme : ni toolchain Rust, ni `wasm-bindgen`, rien d’autre n’est nécessaire pour compiler du Peras.

## Votre premier module

Écrivez `hello.peras` :

```peras build
export fn square(n: i32): i32 {
  return n * n;
}
```

Vérifiez-le, puis compilez-le :

```sh
perasc check hello.peras
perasc build hello.peras --out dist
```

`dist/` contient alors trois fichiers :

| Fichier      | Ce que c’est                                              |
| ------------ | --------------------------------------------------------- |
| `hello.wasm` | le module WebAssembly                                     |
| `hello.js`   | un module ES qui le charge et exporte `square`            |
| `hello.d.ts` | les déclarations TypeScript : `square(n: number): number` |

```js
import { square } from "./dist/hello.js";

console.log(square(12)); // 144
```

## Dans une application Vite

Le plugin `vite-plugin-peras` importe directement les fichiers `.peras` :

```ts
// vite.config.ts
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

export default defineConfig({
  plugins: [peras({ perasc: "chemin/vers/perasc", lang: "fr" })],
});
```

```ts
// main.ts
import { square } from "./hello.peras";

document.body.textContent = String(square(12));
```

En développement, le module est recompilé à chaque enregistrement, et une erreur s’affiche dans l’overlay de Vite avec son extrait, sa cause et sa correction. `vite build` échoue à la moindre erreur. Le plugin écrit aussi `hello.d.peras.ts` à côté de votre source, pour que TypeScript connaisse les vraies signatures (activez `allowArbitraryExtensions` dans votre `tsconfig.json`).

L’exemple `examples/fib` du dépôt est une application complète : `pnpm --filter peras-example-fib dev`.

## Quand quelque chose ne va pas

```peras error E0203
export fn square(n: i32): i32 {
  let unused = 3;
  return n * n;
}
```

```text
erreur[E0203] : la variable `unused` n’est jamais lue
  --> hello.peras:2:7
   |
 2 |   let unused = 3;
   |       ^^^^^^ jamais lu
   |
   = pourquoi : Une valeur jamais lue est du code mort, et souvent un bug : une faute de frappe, ou un usage oublié. Un nom qui commence par `_` marque un paramètre comme volontairement inutilisé.
   = correction : supprime-la, ou utilise sa valeur
   = détails : perasc explain E0203

1 erreur : la compilation est bloquée
```

Rien n’est compilé tant que chaque erreur n’est pas corrigée. `--lang en` affiche les mêmes messages en anglais ; `perasc explain E0203` affiche la page de cette erreur.

Ensuite : [valeurs et variables](basics.md).
