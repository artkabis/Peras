# Mémoire

En mode possédé, Peras gère la mémoire sans ramasse-miettes et sans que vous écriviez `free` : chaque valeur a un seul propriétaire, et elle est libérée quand la portée de son propriétaire se termine.

> Cette page décrit le modèle. Les nombres, les booléens et les structures n’en ont pas besoin : ils vivent dans les variables locales de WebAssembly, pas sur le tas. Les vérifications de possession arrivent avec les tableaux, le premier type sur le tas ; les fondations du compilateur pour elles sont déjà construites et testées (voir plus bas).

## Un propriétaire par valeur

- Chaque valeur a un seul propriétaire : une variable, un champ ou un paramètre.
- Affecter une valeur qui vit sur le tas la **déplace** : l’ancien nom n’est plus utilisable.
- Aucune référence n’est jamais stockée dans une structure ou un tableau, si bien qu’il n’y a jamais de durée de vie à écrire, contrairement à Rust.
- Le partage est explicite, avec `Rc<T>`, visible dans le type.

## Trois façons de passer une valeur

| Convention          | La fonction…                | L’appelant…                                            |
| ------------------- | --------------------------- | ------------------------------------------------------ |
| `x: T` (par défaut) | lit `x`                     | le garde, inchangé                                     |
| `x: inout T`        | peut modifier `x` sur place | écrit `&valeur` ; aucun alias n’existe pendant l’appel |
| `x: sink T`         | prend `x`, et le libère     | ne peut plus l’utiliser                                |

```peras
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

export fn process(data: Array<f32>): i32 {
  let buf = Buffer(data);
  gain(&buf, 0.8);
  return consume(buf);
}
```

## Où vivent les valeurs

| Valeur            | Emplacement                                                   |
| ----------------- | ------------------------------------------------------------- |
| nombres, `bool`   | les variables locales de WebAssembly : aucune adresse mémoire |
| structures        | leurs champs, comme variables locales                         |
| `Array`, `String` | le tas, dans la mémoire linéaire du module                    |

## Ce qui vous protège

Libérer la mémoire à la main est là où les programmes C se trompent : libérer deux fois, utiliser une mémoire après l’avoir libérée, oublier de libérer. En Peras, le compilateur insère chaque libération, et trois vérifications surveillent le compilateur lui-même :

1. **Un vérificateur de libérations.** Avant de générer le moindre code, il prouve que chaque valeur sur le tas est libérée exactement une fois sur chaque chemin : `return`, `break` et `continue` compris. Un programme qui libérerait deux fois ou fuirait n’est pas compilé ; c’est un bug de `perasc`, signalé comme tel.
2. **Un allocateur de débogage.** En développement, la mémoire libérée est empoisonnée et mise en quarantaine, chaque bloc a une zone rouge, et les fuites sont comptées. Un bug mémoire lève une `PerasMemoryError` qui nomme où le bloc a été alloué et libéré :

   ```text
   PerasMemoryError: le bloc alloué à parse (alloc 1) est libéré une seconde fois, à parse (release 2)
   ```

3. **Un test différentiel.** Deux générateurs de code indépendants doivent s’accorder sur les résultats et sur le nombre de blocs encore vivants.

Les détails, et les mesures, sont dans la [décision 0001](../decisions/0001-webassembly-backend.md).

Ensuite : [lire les diagnostics](diagnostics.md).
