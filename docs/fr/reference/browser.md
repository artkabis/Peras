# L’API du navigateur

Ce qu’un module managed (`.apeiron`) peut utiliser du navigateur : un sous-ensemble du DOM, les évènements, un canvas, les minuteurs, `fetch` et le stockage, typés à la main ([décision 0009](../decisions/0009-browser-and-resources.md)). Cette page est vérifiée par le compilateur : un membre absent d’ici, ou listé ici mais inconnu du compilateur, fait échouer les tests. Un module owned (`.peras`) ne peut rien en utiliser ([E0242](../errors/E0242.md)).

Là où le DOM donne `null`, Peras donne `None`. Une propriété marquée _lecture seule_ est fixée par le navigateur lui-même ([E0246](../errors/E0246.md)).

## Globales

| Globale                    | Type ou résultat       | Fait                                                                    |
| -------------------------- | ---------------------- | ----------------------------------------------------------------------- |
| `document`                 | `Document`             | la page                                                                 |
| `storage`                  | `Storage`              | le `localStorage` de la page                                            |
| `fetch(url)`               | `Promise<Response>`    | demande `url` au serveur                                                |
| `sleep(ms)`                | `Promise<void>`        | attend `ms` millisecondes                                               |
| `now()`                    | `f64`                  | les millisecondes depuis le début de la page, comme `performance.now()` |
| `log(text)`                | `void`                 | écrit dans la console                                                   |
| `setTimeout(ms, () => …)`  | `Timer`, une ressource | exécute la fonction fléchée une fois, après `ms` millisecondes          |
| `setInterval(ms, () => …)` | `Timer`, une ressource | exécute la fonction fléchée toutes les `ms` millisecondes               |

```apeiron build
export async fn status(url: String): Promise<String> {
  const response = await fetch(url);
  log(`${url}: ${response.status}`);
  return await response.text();
}
```

## `Document`

| Membre               | Type              | Fait                                       |
| -------------------- | ----------------- | ------------------------------------------ |
| `body`               | `Element`         | le corps de la page, _lecture seule_       |
| `title`              | `String`          | le titre de la page                        |
| `getElementById(id)` | `Option<Element>` | l’élément qui a cet `id`                   |
| `querySelector(css)` | `Option<Element>` | le premier élément que le sélecteur trouve |
| `createElement(tag)` | `Element`         | un nouvel élément, pas encore dans la page |

## `Element`

| Membre                  | Type                      | Fait                                                          |
| ----------------------- | ------------------------- | ------------------------------------------------------------- |
| `textContent`           | `String`                  | son texte                                                     |
| `id`                    | `String`                  | son `id`                                                      |
| `querySelector(css)`    | `Option<Element>`         | le premier élément en lui que le sélecteur trouve             |
| `getAttribute(name)`    | `Option<String>`          | un attribut                                                   |
| `setAttribute(name, v)` | `void`                    | fixe un attribut                                              |
| `append(child)`         | `void`                    | ajoute un élément à sa fin                                    |
| `remove()`              | `void`                    | le retire de la page                                          |
| `addClass(name)`        | `void`                    | ajoute une classe                                             |
| `removeClass(name)`     | `void`                    | retire une classe                                             |
| `toggleClass(name)`     | `bool`                    | ajoute ou retire une classe ; si elle y est ensuite           |
| `focus()`               | `void`                    | lui donne le focus                                            |
| `click()`               | `void`                    | le clique, comme le ferait un utilisateur                     |
| `asInput()`             | `Option<InputElement>`    | lui-même, quand c’est un `<input>`                            |
| `asCanvas()`            | `Option<CanvasElement>`   | lui-même, quand c’est un `<canvas>`                           |
| `on(event, (e) => …)`   | `Listener`, une ressource | exécute la fonction fléchée à chaque `event`, avec un `Event` |

## `InputElement`

| Membre                | Type                      | Fait                |
| --------------------- | ------------------------- | ------------------- |
| `value`               | `String`                  | ce qui y est tapé   |
| `focus()`             | `void`                    | lui donne le focus  |
| `on(event, (e) => …)` | `Listener`, une ressource | comme sur `Element` |

## `CanvasElement`

| Membre                | Type                      | Fait                      |
| --------------------- | ------------------------- | ------------------------- |
| `width`, `height`     | `i32`                     | sa taille, en pixels      |
| `context2d()`         | `Option<Context2d>`       | son contexte de dessin 2D |
| `on(event, (e) => …)` | `Listener`, une ressource | comme sur `Element`       |

## `Context2d`

| Membre                                           | Type     | Fait                                              |
| ------------------------------------------------ | -------- | ------------------------------------------------- |
| `fillStyle`, `strokeStyle`                       | `String` | la couleur des remplissages et des traits, en CSS |
| `lineWidth`                                      | `f64`    | l’épaisseur des traits                            |
| `font`                                           | `String` | la police du texte, en CSS                        |
| `fillRect(x, y, w, h)`, `strokeRect(x, y, w, h)` | `void`   | remplit ou trace un rectangle                     |
| `clearRect(x, y, w, h)`                          | `void`   | efface un rectangle                               |
| `fillText(text, x, y)`                           | `void`   | écrit du texte                                    |
| `beginPath()`, `moveTo(x, y)`, `lineTo(x, y)`    | `void`   | commence et trace un chemin                       |
| `arc(x, y, r, start, end)`                       | `void`   | ajoute un arc au chemin, angles en radians        |
| `fill()`, `stroke()`                             | `void`   | remplit ou trace le chemin                        |

## `Event`

| Membre               | Type             | Fait                                                           |
| -------------------- | ---------------- | -------------------------------------------------------------- |
| `kind`               | `String`         | son type, `"click"` : le `type` de JavaScript, _lecture seule_ |
| `key`                | `Option<String>` | la touche d’un évènement clavier, _lecture seule_              |
| `clientX`, `clientY` | `Option<f64>`    | la position d’un évènement souris, _lecture seule_             |
| `preventDefault()`   | `void`           | annule ce que ferait le navigateur                             |
| `stopPropagation()`  | `void`           | l’empêche d’atteindre les éléments autour                      |

## `Response`

| Membre   | Type              | Fait                                               |
| -------- | ----------------- | -------------------------------------------------- |
| `ok`     | `bool`            | si le statut est entre 200 et 299, _lecture seule_ |
| `status` | `i32`             | le statut HTTP, _lecture seule_                    |
| `text()` | `Promise<String>` | son corps, en texte                                |

## `Storage`

| Membre            | Type             | Fait                        |
| ----------------- | ---------------- | --------------------------- |
| `get(key)`        | `Option<String>` | la valeur gardée pour `key` |
| `set(key, value)` | `void`           | garde `value` pour `key`    |
| `remove(key)`     | `void`           | oublie `key`                |

## Ressources : `Listener`, `Timer`, `Group`

Une ressource appartient à une variable, et est libérée quand cette variable sort de sa portée : un écouteur est retiré, un minuteur arrêté, un groupe libère ce qu’il tient. Elle peut être donnée une fois, en la rendant, en l’ajoutant à un groupe, ou en la libérant plus tôt ([E0245](../errors/E0245.md)). En JavaScript, une ressource a `release()` et `[Symbol.dispose]()`, pour que TypeScript puisse libérer celle qu’un module rend, ou la tenir avec `using`.

| Membre          | Sur       | Fait                                        |
| --------------- | --------- | ------------------------------------------- |
| `Group.new()`   | `Group`   | un groupe vide                              |
| `add(resource)` | `Group`   | prend la ressource : libérée avec le groupe |
| `release()`     | les trois | la libère maintenant                        |

```apeiron build
fn clicked(e: Event): void {
  log(e.kind);
}

export fn mount(root: Element): Group {
  let group = Group.new();
  const button = document.createElement("button");
  button.textContent = "Enregistrer";
  root.append(button);
  const listener = button.on("click", (e) => clicked(e));
  group.add(listener);
  return group;
}
```

En debug, le défaut de `perasc build` et du serveur de développement de Vite, une ressource ramassée par le ramasse-miettes sans avoir été libérée est signalée dans la console, avec le fichier et la ligne qui l’ont créée.
