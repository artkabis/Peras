# The browser API

What a managed module (`.apeiron`) can use of the browser: a subset of the DOM, events, a canvas, timers, `fetch` and storage, typed by hand ([decision 0009](../decisions/0009-browser-and-resources.md)). This page is checked against the compiler: a member missing here, or listed here but unknown to the compiler, fails the tests. An owned module (`.peras`) cannot use any of it ([E0242](../errors/E0242.md)).

Where the DOM gives `null`, Peras gives `None`. A property marked _read only_ is set by the browser itself ([E0246](../errors/E0246.md)).

## Globals

| Global                     | Type or result      | Does                                                        |
| -------------------------- | ------------------- | ----------------------------------------------------------- |
| `document`                 | `Document`          | the page                                                    |
| `storage`                  | `Storage`           | the page's `localStorage`                                   |
| `fetch(url)`               | `Promise<Response>` | asks the server for `url`                                   |
| `sleep(ms)`                | `Promise<void>`     | waits `ms` milliseconds                                     |
| `now()`                    | `f64`               | milliseconds since the page started, as `performance.now()` |
| `log(text)`                | `void`              | writes to the console                                       |
| `setTimeout(ms, () => …)`  | `Timer`, a resource | runs the arrow function once, after `ms` milliseconds       |
| `setInterval(ms, () => …)` | `Timer`, a resource | runs the arrow function every `ms` milliseconds             |

```apeiron build
export async fn status(url: String): Promise<String> {
  const response = await fetch(url);
  log(`${url}: ${response.status}`);
  return await response.text();
}
```

## `Document`

| Member               | Type              | Does                                   |
| -------------------- | ----------------- | -------------------------------------- |
| `body`               | `Element`         | the page's body, _read only_           |
| `title`              | `String`          | the page's title                       |
| `getElementById(id)` | `Option<Element>` | the element with that `id`             |
| `querySelector(css)` | `Option<Element>` | the first element the selector matches |
| `createElement(tag)` | `Element`         | a new element, not in the page yet     |

## `Element`

| Member                  | Type                    | Does                                                      |
| ----------------------- | ----------------------- | --------------------------------------------------------- |
| `textContent`           | `String`                | its text                                                  |
| `id`                    | `String`                | its `id`                                                  |
| `querySelector(css)`    | `Option<Element>`       | the first element inside it the selector matches          |
| `getAttribute(name)`    | `Option<String>`        | an attribute                                              |
| `setAttribute(name, v)` | `void`                  | sets an attribute                                         |
| `append(child)`         | `void`                  | adds an element at its end                                |
| `remove()`              | `void`                  | takes it out of the page                                  |
| `addClass(name)`        | `void`                  | adds a class                                              |
| `removeClass(name)`     | `void`                  | removes a class                                           |
| `toggleClass(name)`     | `bool`                  | adds or removes a class; whether it is there after        |
| `focus()`               | `void`                  | gives it the focus                                        |
| `click()`               | `void`                  | clicks it, as a user would                                |
| `asInput()`             | `Option<InputElement>`  | itself, when it is an `<input>`                           |
| `asCanvas()`            | `Option<CanvasElement>` | itself, when it is a `<canvas>`                           |
| `on(event, (e) => …)`   | `Listener`, a resource  | runs the arrow function on each `event`, given an `Event` |

## `InputElement`

| Member                | Type                   | Does                |
| --------------------- | ---------------------- | ------------------- |
| `value`               | `String`               | what is typed in it |
| `focus()`             | `void`                 | gives it the focus  |
| `on(event, (e) => …)` | `Listener`, a resource | as on `Element`     |

## `CanvasElement`

| Member                | Type                   | Does                   |
| --------------------- | ---------------------- | ---------------------- |
| `width`, `height`     | `i32`                  | its size, in pixels    |
| `context2d()`         | `Option<Context2d>`    | its 2D drawing context |
| `on(event, (e) => …)` | `Listener`, a resource | as on `Element`        |

## `Context2d`

| Member                                           | Type     | Does                                       |
| ------------------------------------------------ | -------- | ------------------------------------------ |
| `fillStyle`, `strokeStyle`                       | `String` | the colour of fills and strokes, as in CSS |
| `lineWidth`                                      | `f64`    | the width of strokes                       |
| `font`                                           | `String` | the font of text, as in CSS                |
| `fillRect(x, y, w, h)`, `strokeRect(x, y, w, h)` | `void`   | fills or strokes a rectangle               |
| `clearRect(x, y, w, h)`                          | `void`   | clears a rectangle                         |
| `fillText(text, x, y)`                           | `void`   | writes text                                |
| `beginPath()`, `moveTo(x, y)`, `lineTo(x, y)`    | `void`   | starts and draws a path                    |
| `arc(x, y, r, start, end)`                       | `void`   | adds an arc to the path, angles in radians |
| `fill()`, `stroke()`                             | `void`   | fills or strokes the path                  |

## `Event`

| Member               | Type             | Does                                                  |
| -------------------- | ---------------- | ----------------------------------------------------- |
| `kind`               | `String`         | its type, `"click"`: JavaScript's `type`, _read only_ |
| `key`                | `Option<String>` | the key of a keyboard event, _read only_              |
| `clientX`, `clientY` | `Option<f64>`    | the position of a mouse event, _read only_            |
| `preventDefault()`   | `void`           | cancels what the browser would do                     |
| `stopPropagation()`  | `void`           | stops it reaching the elements around                 |

## `Response`

| Member   | Type              | Does                                          |
| -------- | ----------------- | --------------------------------------------- |
| `ok`     | `bool`            | whether the status is 200 to 299, _read only_ |
| `status` | `i32`             | the HTTP status, _read only_                  |
| `text()` | `Promise<String>` | its body, as text                             |

## `Storage`

| Member            | Type             | Does                     |
| ----------------- | ---------------- | ------------------------ |
| `get(key)`        | `Option<String>` | the value kept for `key` |
| `set(key, value)` | `void`           | keeps `value` for `key`  |
| `remove(key)`     | `void`           | forgets `key`            |

## Resources: `Listener`, `Timer`, `Group`

A resource belongs to one variable, and is released when that variable goes out of scope: a listener is removed, a timer cleared, a group releases what it holds. It can be given away once, by returning it, adding it to a group, or releasing it early ([E0245](../errors/E0245.md)). In JavaScript, a resource has `release()` and `[Symbol.dispose]()`, so that TypeScript can release one a module returns, or hold it with `using`.

| Member          | On        | Does                                        |
| --------------- | --------- | ------------------------------------------- |
| `Group.new()`   | `Group`   | an empty group                              |
| `add(resource)` | `Group`   | takes the resource: released with the group |
| `release()`     | all three | releases it now                             |

```apeiron build
fn clicked(e: Event): void {
  log(e.kind);
}

export fn mount(root: Element): Group {
  let group = Group.new();
  const button = document.createElement("button");
  button.textContent = "Save";
  root.append(button);
  const listener = button.on("click", (e) => clicked(e));
  group.add(listener);
  return group;
}
```

In debug, the default of `perasc build` and Vite's dev server, a resource collected by the garbage collector without having been released is reported in the console, with the file and line that made it.
