# La recherche derrière Peras

_Version anglaise, qui fait foi : [../en/research.md](../en/research.md)._

Les choix de conception de Peras reposent sur des travaux publiés et relus par des pairs. Cette page les recense : pour chaque choix, l’article, ce qu’il montre, ce qu’il ne montre pas, et où Peras l’applique. Elle recense aussi les travaux qui vont contre certaines parties du projet, car ils en fixent les limites autant que les autres en fixent la conception.

## Comment lire cette page

- **Un article étaye un choix ; il ne prouve pas que Peras a raison.** Chaque article a étudié son propre sujet : Rust, JavaScript, un allocateur pour systèmes temps réel. Ce que Peras affirme de lui-même repose sur ses propres tests et bancs d’essai, liés depuis chaque entrée.
- **Chaque référence a été vérifiée** auprès de la notice de l’éditeur ou de la page des auteurs : auteurs, titre, conférence ou revue, année, DOI quand il existe. Chaque résultat cité ici vient du résumé de l’article ou de sa présentation publiée. Aucun chiffre n’est donné qui n’y ait été lu.
- **Les travaux qui vont contre le projet figurent aussi ici**, dans [l’adoption et ses risques](#ladoption-et-ses-risques).

## La sûreté mémoire sans ramasse-miettes

**La possession peut être rendue sûre.** Jung, Jourdan, Krebbers et Dreyer, _RustBelt: Securing the Foundations of the Rust Programming Language_, POPL 2018 ([DOI 10.1145/3158154](https://doi.org/10.1145/3158154)).

- **Montre** : la première preuve formelle, vérifiée par machine, de la sûreté d’un sous-ensemble réaliste de Rust, et les conditions qu’une bibliothèque utilisant `unsafe` doit remplir pour rester sûre.
- **Ne montre pas** : quoi que ce soit sur le vérificateur de Peras. Peras suit la sémantique de valeurs mutables, pas les références de Rust, et n’a pas encore de preuve formelle. Ses vérifications sont éprouvées par des tests UI, un vérificateur de l’IR qui contrôle chaque libération, l’allocateur de débogage et une cible de fuzz.
- **Dans Peras** : `inout`, `sink` et les vérifications de possession ([mémoire](guide/memory.md), [architecture](architecture.md)).

**Des valeurs sans références peuvent se compiler en code efficace.** Racordon, Shabalin, Zheng, Abrahams et Saeta, _Implementation Strategies for Mutable Value Semantics_, Journal of Object Technology 21(2), 2022.

- **Montre** : des stratégies pour compiler la sémantique de valeurs mutables, étudiée sur un noyau de Swift, en code natif efficace : les valeurs de taille fixe sur la pile, où les optimisations ordinaires s’appliquent, et la copie à l’écriture pour les conteneurs.
- **Ne montre pas** : la vitesse de Peras, qui est mesurée à part ([bancs d’essai](../../bench/README.fr.md)).
- **Dans Peras** : la raison pour laquelle aucune durée de vie ne s’écrit ([spécification](spec.md)).

## Les vérifications statiques attrapent des bugs

Gao, Bird et Barr, _To Type or Not to Type: Quantifying Detectable Bugs in JavaScript_, ICSE 2017 ([DOI 10.1109/ICSE.2017.75](https://doi.org/10.1109/ICSE.2017.75)).

- **Montre** : sur des bugs publics de projets JavaScript, TypeScript 2.0 et Flow en auraient chacun signalé environ 15 % avant leur publication, entre 11,5 % et 18,5 % à 95 % de confiance.
- **Ne montre pas** : que des règles plus strictes, comme celles de Peras, remboursent leur coût d’apprentissage. Aucune étude ne le mesure pour Peras.
- **Dans Peras** : aucune conversion implicite, pas de `any`, un `match` exhaustif, vérifiés dans les deux modes.

## WebAssembly comme cible

**Pourquoi WebAssembly.** Haas, Rossberg, Schuff, Titzer, Holman, Gohman, Wagner, Zakai et Bastien, _Bringing the Web up to Speed with WebAssembly_, PLDI 2017 ([DOI 10.1145/3062341.3062363](https://doi.org/10.1145/3062341.3062363)).

- **Montre** : la motivation, la conception et la sémantique formelle de WebAssembly, et une première expérience de ses implémentations. JavaScript y est décrit comme une cible de compilation inégale.
- **Dans Peras** : les modules owned se compilent en WebAssembly ([décision 0001](decisions/0001-webassembly-backend.md)).

**WebAssembly n’est pas toujours plus rapide.** Jangda, Powers, Berger et Guha, _Not So Fast: Analyzing the Performance of WebAssembly vs. Native Code_, USENIX ATC 2019.

- **Montre** : sur les programmes de SPEC CPU, WebAssembly tournait en moyenne 45 % (Firefox) à 55 % (Chrome) plus lentement que le code natif. Les résultats antérieurs proches de la parité venaient de petits noyaux scientifiques.
- **Dans Peras** : la vitesse se mesure, elle ne se suppose pas. Une fonction passe de `.apeiron` à `.peras` après une mesure. `scripts/bench-managed.mjs` montre pourquoi : un module owned est 1,7 à 1,9 fois plus rapide que le JavaScript écrit à la main pour compter des mots, et 6 fois plus lent sur une boucle dont il doit copier les tableaux JavaScript à l’aller et au retour ([banc des modules managed](../../bench/README.fr.md#modules-managed-benchmanaged)).

## L’allocateur

Masmano, Ripoll, Crespo et Real, _TLSF: A New Dynamic Memory Allocator for Real-Time Systems_, ECRTS 2004 ([DOI 10.1109/EMRTS.2004.1311009](https://doi.org/10.1109/EMRTS.2004.1311009)).

- **Montre** : Two-Level Segregated Fit alloue et libère en temps constant, borné pour les systèmes temps réel.
- **Dans Peras** : les modules de production utilisent TLSF ([décision 0002](decisions/0002-tlsf-allocator.md)).

## Les tables de hachage

**Des clés choisies pour entrer en collision.** Crosby et Wallach, _Denial of Service via Algorithmic Complexity Attacks_, USENIX Security 2003.

- **Montre** : une nouvelle classe de dénis de service à faible débit, par des entrées qui font tomber les structures de données de programmes courants dans leur pire cas.

**Un hachage à clé pour les entrées courtes.** Aumasson et Bernstein, _SipHash: a Fast Short-Input PRF_, INDOCRYPT 2012 (LNCS 7668, p. 489–508).

- **Montre** : une famille de fonctions pseudo-aléatoires rapides sur les entrées courtes, conçue pour les tables de hachage protégées contre l’inondation de hachage.
- **Dans Peras** : les tables et les ensembles hachent avec SipHash-1-3 sous une clé tirée pour chaque instance ([décision 0005](decisions/0005-maps-and-sets.md)).

## Les flottants en texte

Adams, _Ryū: Fast Float-to-String Conversion_, PLDI 2018 ([DOI 10.1145/3192366.3192369](https://doi.org/10.1145/3192366.3192369)).

- **Montre** : un algorithme, avec une preuve de correction, qui écrit le texte décimal le plus court dont on relit le même flottant.
- **Dans Peras** : `toString()` sur les flottants écrit ce même texte le plus court, aujourd’hui en le demandant au `String()` de JavaScript. Ryū est la voie prévue pour l’écrire en WebAssembly si un programme en a besoin plus vite ([journal des modifications](../../CHANGELOG.fr.md)).

## Les diagnostics

Becker, Denny, Pettit, Bouchard, Bouvier, Harrington, Kamil, Karkare, McDonald, Osera, Pearce et Prather, _Compiler Error Messages Considered Unhelpful: The Landscape of Text-Based Programming Error Message Research_, ITiCSE Working Group Reports 2019 ([DOI 10.1145/3344429.3372508](https://doi.org/10.1145/3344429.3372508)).

- **Montre** : une revue de plus de 300 travaux sur les messages d’erreur de programmation, depuis 1965. Son titre en donne la conclusion.
- **Dans Peras** : chaque diagnostic donne sa cause et sa correction, en anglais et en français. Chaque code a une page dont l’exemple et la correction sont vérifiés par les tests ([codes d’erreur](errors/index.md)).

## L’adoption et ses risques

Ces travaux vont contre certaines parties du projet, ou bornent là où il convient.

**Les bibliothèques décident de l’adoption.** Meyerovich et Rabkin, _Empirical Analysis of Programming Language Adoption_, OOPSLA 2013 ([DOI 10.1145/2509136.2509515](https://doi.org/10.1145/2509136.2509515)).

- **Montre** : sur environ 213 000 projets et des sondages de développeurs, les bibliothèques open source, le code existant et l’expérience pèsent le plus dans le choix d’un langage. Les qualités propres d’un langage pèsent moins.
- **Pour Peras** : un module ne peut pas utiliser npm aujourd’hui. Peras convient donc sous forme de modules appelés depuis une application TypeScript, qui garde npm et ses frameworks. L’utilisation typée des bibliothèques JavaScript par leurs `.d.ts` est prévue ([feuille de route, phase 5](reference/roadmap.md)).

**Ce qu’on sait déjà gêne.** Shrestha, Botta, Barik et Parnin, _Here We Go Again: Why Is It Difficult for Developers to Learn Another Programming Language?_, ICSE 2020 ([DOI 10.1145/3377811.3380352](https://doi.org/10.1145/3377811.3380352)).

- **Montre** : sur des questions Stack Overflow et des entretiens avec des programmeurs professionnels, la connaissance d’un langage connu interfère avec l’apprentissage d’un nouveau. Cette interférence est répandue.
- **Pour Peras** : sa syntaxe est celle de TypeScript et ses règles non, ce qui est exactement le cas que décrit cet article. Des diagnostics qui nomment l’habitude TypeScript et donnent la forme Peras sont prévus : le ternaire `a ? b : c`, les littéraux d’objet, `any`.

**Ce que coûte l’adoption d’un langage sûr.** Fulton, Chan, Votipka, Hicks et Mazurek, _Benefits and Drawbacks of Adopting a Secure Programming Language: Rust as a Case Study_, SOUPS 2021.

- **Montre** : dans des entretiens avec 16 développeurs et un sondage auprès de 178, Rust est vu comme atteignant ses objectifs de sécurité et de performance. Ses inconvénients sont une courbe d’apprentissage raide, des bibliothèques limitées et des inquiétudes pour le recrutement. Les développeurs disent aussi qu’il a amélioré leur façon de raisonner sur le code sûr, au-delà de Rust.
- **Pour Peras** : les mêmes inconvénients sont à attendre ; ils sont listés dans [avant d’utiliser Peras en production](reference/roadmap.md#avant-dutiliser-peras-en-production).

**Les assistants IA connaissent mal un langage jeune.** Cassano et al., _MultiPL-E: A Scalable and Polyglot Approach to Benchmarking Neural Code Generation_, IEEE TSE 49(7), 2023 ([DOI 10.1109/TSE.2023.3267446](https://doi.org/10.1109/TSE.2023.3267446)), et Cassano et al., _Knowledge Transfer from High-Resource to Low-Resource Programming Languages for Code LLMs_, OOPSLA 2024 ([arXiv 2308.09895](https://arxiv.org/abs/2308.09895)).

- **Montrent** : les modèles de code réussissent sur les langages fréquents dans leurs données d’entraînement, et peinent sur les langages rares. Des données d’entraînement traduites depuis Python et validées par des tests ont doublé leurs résultats en Racket et en OCaml.
- **Pour Peras** : un assistant écrit du TypeScript que Peras refuse. Les diagnostics du compilateur, qui donnent la correction, et cette documentation, dont chaque exemple est compilé par les tests, sont ce dont il peut apprendre. Un jeu de programmes Peras validés suivrait la méthode de cet article.
