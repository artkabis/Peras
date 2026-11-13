# Modules

Un programme d’une certaine taille se découpe en fichiers. Un module owned (`.peras`) importe ce que d’autres modules owned exportent : des fonctions, des structures et des énumérations ([décision 0022](../decisions/0022-owned-modules.md)).

## Exporter et importer

`export` marque ce qu’un autre fichier peut importer. Ici `shapes.peras` exporte une structure, une énumération et deux fonctions, et garde `half` pour lui :

```peras file shapes.peras
export struct Size {
  width: f64;
  height: f64;
}

export enum Shape {
  Circle(f64),
  Box(Size),
}

fn half(x: f64): f64 {
  return x / 2.0;
}

export fn area(s: Size): f64 {
  return s.width * s.height;
}

export fn inner(s: Size): Size {
  return Size(half(s.width), half(s.height));
}
```

Un autre fichier les importe par leur nom, depuis le chemin du fichier, relatif à lui-même :

```peras main
import { Size, Shape, area, inner } from "./shapes.peras";

fn half(x: f64): f64 {
  return x * 0.5;
}

export fn surface(shapes: Array<Shape>): f64 {
  let total = 0.0;
  for (const shape of shapes) {
    total += match (shape) {
      Shape.Circle(r) => 3.14159 * r * r,
      Shape.Box(size) => area(inner(size)),
    };
  }
  return half(total);
}
```

Les deux fichiers ont un `half` privé : un nom non exporté appartient au fichier.

- **Le chemin** commence par `./`, nomme un fichier `.peras`, extension comprise, et reste dans le dossier du fichier qui importe ou en dessous : `./shapes.peras`, `./lib/shapes.peras`.
- **Une structure ou une énumération importée** est celle du fichier qui la déclare. Importée deux fois, par deux fichiers, elle est un seul type.
- **Un nom se prend une fois.** Un nom importé que le fichier déclare aussi, ou importe d’un autre fichier, est refusé ([E0244](../errors/E0244.md)).

## Un build, un module

`perasc build main.peras` suit les imports. Chaque fichier atteint est compilé dans le même module WebAssembly, avec une seule mémoire et un seul allocateur :

- une valeur passée d’un fichier à l’autre n’est pas copiée, et la propriété est vérifiée d’un fichier à l’autre comme au sein d’un seul ;
- les optimisations du compilateur voient les fonctions de chaque fichier, et le moteur WebAssembly peut inliner un appel d’un fichier à l’autre comme au sein d’un seul ;
- JavaScript n’appelle que ce qu’exporte le fichier construit. Construit seul, `shapes.peras` exporte ses propres fonctions.

## Les cycles sont refusés

Chaque fichier est vérifié contre ce qu’exportent les fichiers qu’il importe, donc ceux-ci sont vérifiés d’abord. Un fichier qui dépend de lui-même, directement ou par d’autres, est refusé, avec le chemin du cycle ([E0250](../errors/E0250.md)). Déplacez ce que les fichiers du cycle partagent dans un troisième fichier, que chacun d’eux importe.

## Pas encore

- **Les fonctions, structures et énumérations génériques** ne s’importent pas encore ([E0244](../errors/E0244.md)) : un générique est refait pour chaque type avec lequel il sert, depuis sa source. C’est l’étape suivante de la [décision 0022](../decisions/0022-owned-modules.md).
- **Renommer un import**, `import { a as b }`, n’existe pas.
- **Les méthodes d’une structure importée** ne sont pas importées avec elle : appelez des fonctions à la place.
- **Une fonction qu’aucun export n’atteint** est tout de même compilée dans le module : un fichier importé apporte toutes ses fonctions.

## Outillage

- `perasc build --json` liste les fichiers dont le module est fait, sous `inputs`.
- Le plugin Vite les surveille : changer un fichier importé reconstruit ce qui l’importe.
- Une erreur dans un fichier importé s’affiche dans ce fichier.
- Un module managed (`.apeiron`) qui importe un module owned n’a besoin de rien de plus : ce module est construit avec les fichiers qu’il importe ([modules managed](managed.md)).
