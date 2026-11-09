# Mémoire

En mode possédé, Peras gère la mémoire sans ramasse-miettes et sans que vous écriviez `free` : chaque valeur a un seul propriétaire, et elle est libérée quand la portée de son propriétaire se termine.

> Les nombres, les booléens, les structures qui en contiennent et les options n’ont besoin de rien de tout cela : ils vivent dans les variables locales de WebAssembly, pas sur le tas. Les tableaux et les chaînes vivent sur le tas, et le compilateur leur applique tout ce que décrit cette page.

## Un propriétaire par valeur

- Chaque valeur sur le tas a un seul propriétaire : une variable, un champ ou un paramètre `sink`.
- Lire une valeur l’emprunte. L’affecter, la renvoyer, ou la passer à un paramètre `sink`, à une structure ou à `push` la **déplace** : l’ancien nom ne contient plus rien tant qu’il ne reçoit pas une nouvelle valeur ([E0204](../errors/E0204.md)).
- Seul le propriétaire peut déplacer une valeur : un paramètre qui n’est pas `sink`, un champ ou un élément ne font que l’emprunter ([E0209](../errors/E0209.md)). Déplacez une copie, `xs.copy()`, pour garder les deux.
- Aucune référence n’est jamais stockée dans une structure ou un tableau, si bien qu’il n’y a jamais de durée de vie à écrire, contrairement à Rust.
- Le partage sera explicite, avec `Rc<T>`, visible dans le type.

Les vérifications suivent chaque chemin : une valeur déplacée d’un côté d’un `if` compte comme déplacée après lui ; dans une boucle, une valeur déplacée pendant un tour l’est encore au tour suivant, sauf si le tour lui donne une nouvelle valeur ou sort avec `break`.

## Trois façons de passer une valeur

| Convention          | La fonction…                | L’appelant…                                            |
| ------------------- | --------------------------- | ------------------------------------------------------ |
| `x: T` (par défaut) | lit `x`                     | le garde, inchangé                                     |
| `x: inout T`        | peut modifier `x` sur place | écrit `&valeur` ; aucun alias n’existe pendant l’appel |
| `x: sink T`         | prend `x`, et le libère     | ne peut plus l’utiliser                                |

`&` marque un argument `inout`, et lui seul ([E0206](../errors/E0206.md)). Aucun autre argument de l’appel ne peut atteindre le même emplacement, et rien d’autre dans l’instruction ne peut le lire ([E0207](../errors/E0207.md)) ; deux champs différents d’une structure sont deux emplacements différents. Une boucle `for…of` ne laisse pas son tableau changer de taille.

```peras build
struct Buffer {
  samples: Array<f32>;
}

fn gain(buf: inout Buffer, factor: f32): void {
  for (let i = 0; i < buf.samples.length; i++) {
    buf.samples[i] = buf.samples[i] * factor;
  }
}

fn consume(buf: sink Buffer): i32 {
  return buf.samples.length;
}

export fn process(data: sink Array<f32>): i32 {
  let buf = Buffer(data);
  gain(&buf, 0.8);
  return consume(buf);
}
```

## Quand la mémoire est libérée

Le compilateur insère chaque libération ; vous n’en écrivez jamais :

- à la fin du bloc qui déclare une variable, si elle possède encore sa valeur ;
- quand une variable reçoit une nouvelle valeur, pour l’ancienne ;
- avant `return`, pour tout ce que la fonction possède encore ; avant `break` et `continue`, pour ce que le corps de la boucle a déclaré ;
- là où des chemins se rejoignent : une valeur déplacée sur l’un d’eux est libérée sur les autres, aussitôt.

```peras build
fn total(xs: sink Array<i32>): i32 {
  return xs.length;
}

export fn count(n: i32): i32 {
  let xs: Array<i32> = [];
  let sum = 0;
  for (let i = 0; i < n; i++) {
    xs.push(i);
    if (i == 10) {
      sum += total(xs);
      break;
    }
  }
  return sum;
}
```

Ici, `xs` est libéré par `total` sur le chemin qui sort par `break`, et dès la fin de la boucle sur celui où elle va jusqu’au bout.

## Où vivent les valeurs

| Valeur              | Emplacement                                                   |
| ------------------- | ------------------------------------------------------------- |
| nombres, `bool`     | les variables locales de WebAssembly : aucune adresse mémoire |
| structures, options | leurs champs, comme variables locales                         |
| `Array`, `String`   | le tas, dans la mémoire linéaire du module                    |

## Ce qui vous protège

Libérer la mémoire à la main est là où les programmes C se trompent : libérer deux fois, utiliser une mémoire après l’avoir libérée, oublier de libérer. En Peras, le compilateur insère chaque libération, et trois vérifications surveillent le compilateur lui-même :

1. **Un vérificateur de libérations.** Avant de générer le moindre code, il prouve que chaque valeur sur le tas est libérée exactement une fois sur chaque chemin : `return`, `break` et `continue` compris. Un programme qui libérerait deux fois ou fuirait n’est pas compilé ; c’est un bug de `perasc`, signalé comme tel.
2. **Un allocateur de débogage.** En développement, la mémoire libérée est empoisonnée et mise en quarantaine, chaque bloc a une zone rouge, et les fuites sont comptées. Un bug mémoire lève une `PerasMemoryError` qui nomme où le bloc a été alloué et libéré :

   ```text
   PerasMemoryError: le bloc alloué à parse (alloc 1) est libéré une seconde fois, à parse (release 2)
   ```

3. **Des tests sur chaque chemin.** Les programmes de test s’exécutent dans un interpréteur du code intermédiaire du compilateur, et en WebAssembly sur l’allocateur de débogage : les deux doivent donner les résultats attendus et ne laisser aucun bloc vivant.

En production, `vite build` et `perasc build --allocator tlsf` lient à la place un allocateur TLSF : chaque allocation et libération en temps borné, les blocs libérés fusionnés aussitôt, 2,7 fois plus rapide sur un programme qui alloue beaucoup ([décision 0002](../decisions/0002-tlsf-allocator.md)).

Les détails, et les mesures, sont dans la [décision 0001](../decisions/0001-webassembly-backend.md).

Ensuite : [lire les diagnostics](diagnostics.md).
