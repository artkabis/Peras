# Formatage

`perasc fmt` met en page un fichier Peras comme Prettier met en page TypeScript, à une largeur de 100 ([décision 0041](../decisions/0041-formatter.md)). Chaque fichier se ressemble, quel que soit son auteur : il n’y a rien à régler.

Un fichier écrit à la hâte :

```text
struct Point { x: f64, y: f64 }

// The perimeter of a path, closed or not.
export fn perimeter(points: Array<Point>, closedPath: bool, scale: f64, offset: f64, round: bool): f64 {
  let total = 0.0;
  for (let i = 1; i < points.length; i++) { total += distance(points[i - 1], points[i]); }
  if (closedPath && points.length > 2) { total += distance(points[points.length - 1], points[0]); }
  const value = total * scale + offset;
  if (round) { return Math.round(value); }
  return value;
}

fn distance(a: Point, b: Point): f64 {
    const dx = a.x-b.x;
    const dy = a.y-b.y;
    return Math.sqrt(dx*dx + dy*dy);
}
```

après `perasc fmt shapes.peras` :

```peras build
struct Point {
  x: f64,
  y: f64,
}

// The perimeter of a path, closed or not.
export fn perimeter(
  points: Array<Point>,
  closedPath: bool,
  scale: f64,
  offset: f64,
  round: bool,
): f64 {
  let total = 0.0;
  for (let i = 1; i < points.length; i++) {
    total += distance(points[i - 1], points[i]);
  }
  if (closedPath && points.length > 2) {
    total += distance(points[points.length - 1], points[0]);
  }
  const value = total * scale + offset;
  if (round) {
    return Math.round(value);
  }
  return value;
}

fn distance(a: Point, b: Point): f64 {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}
```

## Ce qu’il fait

- Un bloc met chaque instruction sur sa ligne, indentée de deux espaces ; une ligne vide entre deux instructions est gardée, une suite de lignes vides ramenée à une.
- Une liste, les paramètres d’une fonction, les arguments d’un appel, les éléments d’un tableau, les bras d’un `match`, reste sur une ligne si elle tient dans 100 colonnes, et met chaque élément sur sa ligne sinon, avec une virgule finale. Une fonction donnée en dernier, `xs.map((x) => { … })`, reste sur la ligne de son appel.
- Une chaîne de trois appels ou plus, `xs.filter(…).map(…).join(…)`, se coupe avant chaque `.` si elle ne tient pas ; une longue condition après chaque `&&` et `||` ; une longue somme après chaque `+` et `-`.
- Le corps d’une `struct` ou d’un `enum` met un champ par ligne, et garde les lignes vides entre eux.
- Les commentaires restent où ils sont, sur leur ligne ou en fin de la ligne qu’ils suivent. Un gabarit n’est jamais coupé dans son `${…}`, une chaîne non plus.
- Les littéraux gardent le texte avec lequel ils ont été écrits : `1_000`, `0xff`, `1.50`.

## L’utiliser

```sh
perasc fmt src/*.peras src/*.apeiron          # formate chaque fichier sur place
perasc fmt --check src/*.peras src/*.apeiron  # nomme les fichiers qu’il changerait, statut 1 s’il y en a
```

`--check` est celui d’une CI. Un fichier avec une erreur de syntaxe est laissé tel quel, et nommé : `perasc check` montre ses erreurs. Le formatage ne change rien à ce que fait un programme : un fichier donne le même module, octet pour octet, avant et après, ce que le dépôt vérifie sur chacun de ses fichiers Peras.
