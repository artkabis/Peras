# 0009 : Le navigateur, typé, et des ressources libérées avec leur propriétaire

_Version anglaise, qui fait foi : [../../en/decisions/0009-browser-and-resources.md](../../en/decisions/0009-browser-and-resources.md)._

## Contexte

L’étape 1 du mode managed ([décision 0008](0008-managed-mode.md)) compile les modules `.apeiron` en JavaScript, mais ils ne pouvaient pas atteindre le navigateur : un module managed recevait des valeurs et en rendait, et TypeScript appelait le DOM. La [spécification](../spec.md#mode-managed) prévoit deux choses pour l’étape 2 : des API du navigateur typées, et la possession pour les **ressources** seulement, les écouteurs, minuteurs et requêtes dont les fuites sont les fuites habituelles d’une application web, une référence restée attachée plutôt qu’une libération manquante.

## Ce que d’autres ont fait

- **TypeScript** type tout le DOM à partir de `lib.dom.d.ts` : complet, avec des `any`, des unions et des surcharges que le vérificateur accepte largement.
- **Elm et ReScript** lient à la main une partie choisie du navigateur, chaque liaison écrite et vérifiée une fois.
- **La gestion explicite des ressources** (TC39, dans V8 depuis 2025) ajoute `using` et `Symbol.dispose` à JavaScript : une ressource liée par `using` est libérée à la fin de son bloc. Rien ne vérifie qu’une ressource est liée du tout.
- **`AbortController`** : un signal donné à `addEventListener` et à `fetch` retire et annule tout ce à quoi il a été donné.
- **`FinalizationRegistry`** dit à un programme qu’un objet a été ramassé : des outils s’en servent pour trouver les fuites en développement.

## Décision

1. **Un sous-ensemble du navigateur, typé à la main.** `Document`, `Element`, `InputElement`, `CanvasElement`, `Context2d`, `Event`, `Response` et `Storage`, les globales `document`, `storage`, `fetch`, `sleep`, `now`, `log`, `setTimeout` et `setInterval`, chaque membre listé une fois dans `perasc-check` ([référence du navigateur](../reference/browser.md)). Là où le DOM donne `null`, Peras donne `None` ; `asInput()` et `asCanvas()` sont des conversions vérifiées. Les propriétés que fixe le navigateur ne peuvent qu’être lues ([E0246](../errors/E0246.md)). Un module owned ne peut pas utiliser le navigateur ([E0242](../errors/E0242.md)). D’autres API viennent quand un programme en a besoin, chacune ajoutée de la même façon.
2. **Les ressources ont un seul propriétaire.** Un `Listener`, un `Timer` et un `Group` appartiennent à une variable, et sont libérés quand elle sort de sa portée, par n’importe quelle sortie. Une ressource peut être donnée une fois : rendue, ajoutée à un `Group`, ou libérée par `release()`. Le vérificateur refuse une ressource que rien ne garde, utilisée après avoir été donnée, donnée dans une boucle, gardée par une structure, un tableau, une option, une table ou une fonction fléchée, donnée par une fonction qui ne faisait que l’emprunter, ou remplacée ([E0245](../errors/E0245.md)).
3. **Libérées comme JavaScript les libère.** Un écouteur est ajouté avec le signal de son propre `AbortController`, et libéré en l’annulant ; un minuteur est arrêté. La suite du bloc du propriétaire s’exécute dans un `try` dont le `finally` libère la ressource, sauf si un drapeau dit qu’elle a été donnée. Une ressource est un objet avec `release()` et `[Symbol.dispose]()`, pour que TypeScript libère celle qu’un module rend, ou la tienne avec `using`.
4. **Les fuites signalées en debug.** Avec l’allocateur de débogage, le défaut de `perasc build` et du serveur de développement de Vite, chaque ressource est inscrite dans un `FinalizationRegistry` : une ressource ramassée sans avoir été libérée est signalée dans la console avec le fichier et la ligne qui l’ont créée. Un build de production n’emporte pas de détecteur.

## Non retenu

- **Lire `lib.dom.d.ts`.** Complet d’un coup, et le chemin vers les déclarations de npm (phase 5), mais ses `any`, ses unions et ses surcharges feraient entrer la souplesse de TypeScript dans les vérifications de Peras.
- **La libération à la main seulement**, avec un avertissement quand `release()` n’est jamais appelé : plus simple, et les fuites restent possibles sur chaque chemin qui l’oublie.
- **Les fonctions fléchées à instructions, et les fermetures comme valeurs.** Un gestionnaire est une fonction fléchée dont le corps est une expression, en général un appel : assez pour cette étape, et cela empêche les gestionnaires de capturer des ressources.

## État

**Construit.**

- **Vérifications** : les types et les membres du navigateur dans `perasc-check` (`browser.rs`, `host.rs`), puis une passe sur chaque fonction d’un module managed qui suit chaque variable ressource sur chaque chemin, les branches d’un `if` réunies, le corps d’une boucle donné au plus une fois (`resources.rs`). Chaque cas d’E0245 donne une erreur dans les tests UI.
- **Générateur** : l’API du navigateur dans `perasc-codegen-js` (`browser.rs`), et le runtime des ressources (`$Resource`, `$on`, `$timer`, `$group`), écrit dans un module seulement s’il s’en sert.
- **Dans Chromium**, depuis le serveur de développement de Vite : un module managed construit une page, écoute les clics et la saisie, dessine sur un canvas et bat la mesure ; libéré, ses écouteurs sont retirés et son minuteur arrêté, vérifié en cliquant et en tapant à nouveau ; `fetch` lit un fichier et un statut d’erreur ; `storage` garde et oublie une valeur ; un écouteur rendu à JavaScript puis abandonné est signalé par le détecteur avec sa ligne. Trois exécutions de suite.
- Les exemples managed de la documentation, ceux de la référence avant tout, sont compilés par les tests, et la référence liste exactement l’API que connaît le compilateur.

Pas encore : davantage du navigateur (formulaires, `requestAnimationFrame`, WebSockets, workers), les tableaux de ressources, une requête comme ressource annulée avec son propriétaire, et le signalement des fuites dans l’overlay de Vite.

## Conséquences

- Une interface web peut s’écrire en Peras de bout en bout, son calcul dans des modules owned.
- Un gestionnaire ne peut pas garder une ressource : celui qui doit en arrêter une autre s’écrit comme une fonction qui reçoit ce dont elle a besoin.
