# Messages français — traduction de en.ftl, qui fait foi.
# Chaque clé de en.ftl doit exister ici ; sinon la CI échoue.

cli-version = perasc { $version }
cli-usage =
    Utilisation : perasc <commande> [options]

    Commandes :
      check <fichier>... vérifie des fichiers .peras et .apeiron et signale toutes les erreurs
      explain <code>     explique un code d’erreur, par exemple `perasc explain E0001`

    Options :
      --lang <en|fr>     langue des messages (par défaut : PERAS_LANG, puis la locale du système)
      --json             affiche les diagnostics de `check` en JSON sur la sortie standard
      --color <quand>    colore les diagnostics : auto (par défaut), always, never ; NO_COLOR est respecté
      -V, --version      affiche la version
      -h, --help         affiche cette aide
cli-unknown-argument = argument inconnu `{ $argument }`
cli-unknown-command = commande inconnue `{ $command }`
cli-missing-value = `{ $option }` attend une valeur
cli-invalid-value = valeur `{ $value }` invalide pour `{ $option }`
cli-no-files = `check` attend au moins un fichier
cli-missing-code = `explain` attend un code d’erreur, par exemple E0001
cli-unknown-code = code d’erreur inconnu `{ $code }`
cli-not-a-source = `{ $path }` n’est pas un source Peras : l’extension doit être .peras ou .apeiron
cli-not-found = `{ $path }` n’existe pas
cli-not-utf8 = `{ $path }` n’est pas du texte UTF-8
cli-read-error = impossible de lire `{ $path }` : { $reason }
cli-hidden-errors = { $count ->
    [one] 1 autre erreur
   *[other] { $count } autres erreurs
} dans { $path }, non affichées : la limite est de { $limit } par fichier
cli-summary-ok = { $files ->
    [one] 1 fichier vérifié, aucune erreur
   *[other] { $files } fichiers vérifiés, aucune erreur
}
cli-summary-errors = { $errors ->
    [one] 1 erreur : la compilation est bloquée
   *[other] { $errors } erreurs : la compilation est bloquée
}

## Lexer (E0001–E0099)

# E0001
lex-invalid-character = { $count ->
    [one] caractère invalide { $character } ({ $codepoint })
   *[other] { $count } caractères invalides, à partir de { $character } ({ $codepoint })
}
lex-invalid-character-label = { $count ->
    [one] ce caractère n’est pas autorisé dans du code Peras
   *[other] ces caractères ne sont pas autorisés dans du code Peras
}
lex-invalid-character-why = Hors des chaînes et des commentaires, le code Peras n’utilise que des lettres ASCII, des chiffres, `_`, des espaces et les opérateurs du langage.
lex-invalid-character-fix-remove = { $count ->
    [one] supprime ce caractère
   *[other] supprime ces caractères
}
lex-invalid-character-fix-replace = remplace { $character } par `{ $replacement }`, le caractère ASCII auquel il ressemble
lex-invalid-character-fix-identifier = les noms n’utilisent que des lettres ASCII, des chiffres et `_` : renomme-le, par exemple `cafe` au lieu de `café` ; pour afficher du texte, mets-le dans une chaîne
lex-invalid-character-fix-template = les gabarits de chaîne (template literals) ne sont pas encore pris en charge : concatène les chaînes avec `+`

# E0002
lex-unterminated-string = chaîne non terminée
lex-unterminated-string-label = cette chaîne n’est jamais fermée
lex-unterminated-string-end-label = la ligne s’arrête ici, avant le `{ $quote }` fermant
lex-unterminated-string-why = Une chaîne doit se fermer sur la ligne où elle commence ; pour y mettre un retour à la ligne, écris `\n`.
lex-unterminated-string-fix = ferme la chaîne avec `{ $quote }`

# E0003
lex-unterminated-block-comment = commentaire de bloc non terminé
lex-unterminated-block-comment-label = ce commentaire n’est jamais fermé
lex-unterminated-block-comment-why = Un commentaire `/*` court jusqu’au prochain `*/` ; sans lui, tout le reste du fichier est un commentaire.
lex-unterminated-block-comment-fix = ferme le commentaire avec `*/`

# E0004
lex-invalid-escape = séquence d’échappement invalide `{ $escape }`
lex-invalid-escape-unknown-label = `\{ $character }` n’est pas une séquence d’échappement
lex-invalid-escape-braces-label = Peras écrit le point de code entre accolades
lex-invalid-escape-unicode-label = échappement Unicode invalide
lex-invalid-escape-why = Les chaînes n’acceptent que `\n`, `\r`, `\t`, `\0`, `\\`, `\'`, `\"` et `\u{"{"}…{"}"}` avec 1 à 6 chiffres hexadécimaux désignant une valeur scalaire Unicode.
lex-invalid-escape-fix-backslash = pour écrire une barre oblique inverse, double-la : `\\`
lex-invalid-escape-fix-braces = écris `{ $replacement }`
lex-invalid-escape-fix-unicode = écris un point de code de `\u{"{"}0{"}"}` à `\u{"{"}10FFFF{"}"}`, hors substituts `\u{"{"}D800{"}"}` à `\u{"{"}DFFF{"}"}`

# E0005
lex-malformed-number = nombre mal formé `{ $literal }`
lex-number-no-digits-label = aucun chiffre après `{ $prefix }`
lex-number-no-digits-why = Un préfixe `0x`, `0o` ou `0b` doit être suivi d’au moins un chiffre.
lex-number-no-digits-fix = ajoute des chiffres après le préfixe, par exemple `0xFF`
lex-number-invalid-digit-label = `{ $digit }` n’est pas un chiffre { $base ->
    [2] binaire
    [8] octal
   *[other] hexadécimal
}
lex-number-invalid-digit-why = Les nombres binaires (`0b`) utilisent les chiffres 0 et 1, les octaux (`0o`) de 0 à 7, les hexadécimaux (`0x`) de 0 à 9 et de a à f.
lex-number-invalid-digit-fix = supprime ce chiffre, ou change le préfixe pour la bonne base
lex-number-leading-zero-label = zéro en tête
lex-number-leading-zero-why = En JavaScript, `012` désignait autrefois un nombre octal ; Peras interdit cette ambiguïté et écrit les nombres octaux avec `0o`.
lex-number-leading-zero-fix = supprime les zéros en tête
lex-number-leading-zero-fix-ambiguous = écris `{ $octal }` pour un nombre octal, ou `{ $decimal }` pour un nombre décimal
lex-number-underscore-label = `_` doit se trouver entre deux chiffres
lex-number-underscore-why = Les tirets bas séparent des groupes de chiffres, comme dans `1_000_000` ; ils ne peuvent ni commencer ni finir un nombre, ni toucher un `.` ou un préfixe, ni se suivre.
lex-number-underscore-fix = supprime ce `_`
lex-number-exponent-label = l’exposant n’a aucun chiffre
lex-number-exponent-why = Un exposant demande des chiffres après `e`, comme dans `1e3` ou `2.5e-4`.
lex-number-exponent-fix = ajoute des chiffres après `e`, ou supprime le `e`
lex-number-suffix-label = `{ $suffix }` inattendu après le nombre
lex-number-suffix-why = Les nombres littéraux n’ont pas de suffixe d’unité ou de type : le type vient du contexte ou d’une annotation.
lex-number-suffix-fix = supprime `{ $suffix }` ; pour choisir un type, annote la variable, par exemple `let x: f32 = 5.0`
lex-number-radix-fraction-label = seuls les nombres décimaux peuvent avoir une partie fractionnaire
lex-number-radix-fraction-why = Les nombres hexadécimaux, octaux et binaires sont des entiers ; seuls les nombres décimaux ont une partie fractionnaire ou un exposant.
lex-number-radix-fraction-fix = écris le nombre en décimal

# E0006
lex-strict-equality = `{ $operator }` n’existe pas en Peras
lex-strict-equality-label = écris `{ $replacement }`
lex-strict-equality-why = Peras ne convertit jamais les types implicitement : `==` et `!=` comparent donc déjà strictement, comme `===` et `!==` en JavaScript.
lex-strict-equality-fix = remplace `{ $operator }` par `{ $replacement }`

# E0007
lex-bidi-control = caractère invisible de contrôle bidirectionnel { $codepoint } ({ $name })
lex-bidi-control-label = ce caractère invisible change l’ordre dans lequel le code s’affiche
lex-bidi-control-why = Un caractère de contrôle bidirectionnel peut faire paraître le code différent de ce qu’il compile (l’attaque Trojan Source, CVE-2021-42574) : il est donc refusé partout, chaînes et commentaires compris.
lex-bidi-control-fix-remove = supprime ce caractère
lex-bidi-control-fix-escape = écris-le sous forme d’échappement `{ $escape }`, qui reste visible

# E0008
lex-source-too-large = fichier source trop volumineux
lex-source-too-large-label = ce fichier fait { $size } octets
lex-source-too-large-why = Les positions dans le source sont des nombres sur 32 bits : un fichier source doit faire moins de 4 Gio.
lex-source-too-large-fix = découpe ce fichier en plusieurs modules

## Parser (E0100–E0199)

# E0100
parse-expected-expression = valeur attendue, trouvé { $eof ->
    [1] la fin du fichier
   *[other] `{ $found }`
}
parse-expected-expression-label = une valeur est attendue ici
parse-expected-expression-why = Une expression est attendue à cet endroit : un nombre, une chaîne, un nom, un appel comme `f(x)` ou un calcul comme `a + 1`.
parse-expected-expression-fix = écris la valeur manquante, ou supprime ce qui précède cet endroit

# E0101
parse-missing-semicolon = `;` manquant
parse-missing-semicolon-label = l’instruction se termine ici
parse-missing-semicolon-why = Chaque instruction se termine par `;`. Peras ne devine pas où une instruction s’arrête, contrairement à l’insertion automatique de JavaScript, qui peut fusionner deux lignes en une seule instruction.
parse-missing-semicolon-fix = ajoute `;`

# E0102
parse-expected-name = { $what ->
    [function] nom de fonction attendu
    [variable] nom de variable attendu
    [parameter] nom de paramètre attendu
    [field] nom de champ attendu
    [attribute] nom d’attribut attendu
    [struct] nom de structure attendu
   *[other] nom attendu
}, trouvé { $eof ->
    [1] la fin du fichier
   *[other] `{ $found }`
}
parse-expected-name-label = un nom est attendu ici
parse-expected-name-why = Un nom commence par une lettre ou `_`, suivie de lettres, de chiffres ou de `_`. Les mots-clés comme `fn`, `let` ou `match` sont réservés.
parse-expected-name-fix = écris un nom ici
parse-expected-name-fix-keyword = `{ $keyword }` est un mot-clé : choisis un autre nom, par exemple `{ $keyword }_`

# E0103
parse-unclosed = `{ $open }` non fermé
parse-unclosed-open-label = `{ $open }` ouvert ici
parse-unclosed-label = `{ $close }` attendu ici, avant { $eof ->
    [1] la fin du fichier
   *[other] `{ $found }`
}
parse-unclosed-why = Chaque `(`, `[`, `{"{"}` et `<` a besoin de son `)`, `]`, `{"}"}` ou `>` fermant ; l’analyse a atteint la fin de la construction avant.
parse-unclosed-fix = ferme-le avec `{ $close }`

# E0104
parse-unexpected = symbole inattendu : { $eof ->
    [1] la fin du fichier
   *[other] `{ $found }`
}
parse-unexpected-label = ceci ne peut pas apparaître ici
parse-unexpected-expected-label = `{ $expected }` attendu
parse-unexpected-separator-label = `,` ou `{ $close }` attendu
parse-unexpected-why = Chaque construction a une forme fixe, comme `fn nom(paramètre: Type): Type {"{"} … {"}"}` ou `let nom = valeur;`, et ce symbole n’y a pas sa place à cet endroit.
parse-unexpected-fix = vérifie s’il manque un opérateur, un séparateur ou une parenthèse sur cette ligne

# E0105
parse-expected-type = type attendu, trouvé { $eof ->
    [1] la fin du fichier
   *[other] `{ $found }`
}
parse-expected-type-label = un type est attendu ici, par exemple `i32`
parse-expected-type-why = Un type suit `:` dans un paramètre, un type de retour ou une variable annotée : `i32`, `f64`, `bool`, `Array<f32>`, `Option<i32>`…
parse-expected-type-fix = écris un type, par exemple `i32`

# E0106
parse-removed-keyword = `{ $keyword }` n’existe pas en Peras
parse-removed-keyword-label = écris `{ $replacement }`
parse-removed-keyword-why = { $keyword ->
    [var] `var` porte sur toute la fonction et peut être déclaré deux fois ; Peras n’a que `let` et `const`, limités à leur bloc.
   *[other] Peras déclare les fonctions avec `fn`, qui n’a jamais de `this` dynamique.
}
parse-removed-keyword-fix = remplace `{ $keyword }` par `{ $replacement }`

# E0107
parse-number-too-large = le nombre `{ $literal }` est trop grand
parse-number-too-large-int-label = ne tient pas sur 64 bits
parse-number-too-large-float-label = est infini en flottant 64 bits
parse-number-too-large-why = Les entiers les plus larges font 64 bits (`i64` jusqu’à 9223372036854775807, `u64` jusqu’à 18446744073709551615), et un `f64` ne peut dépasser environ 1.8e308.
parse-number-too-large-fix = utilise une valeur plus petite

# E0108
parse-chained-comparison = les comparaisons ne peuvent pas s’enchaîner
parse-chained-comparison-first-label = première comparaison
parse-chained-comparison-label = deuxième comparaison
parse-chained-comparison-why = En JavaScript, `a < b < c` compare le booléen `a < b` à `c`, ce qui n’est presque jamais voulu ; Peras le refuse.
parse-chained-comparison-fix = compare chaque paire et relie-les avec `&&` : `{ $suggestion }`
parse-chained-comparison-fix-mixed = découpe l’expression avec `&&`, ou ajoute des parenthèses pour montrer ce qui est comparé

# E0109
parse-bitwise-mix = `{ $bitwise }` et `{ $comparison }` demandent des parenthèses
parse-bitwise-mix-label = opérateur bit à bit
parse-bitwise-mix-comparison-label = comparaison
parse-bitwise-mix-why = JavaScript lit `a & b == c` comme `a & (b == c)`, car `&`, `|` et `^` sont moins prioritaires que les comparaisons ; c’est rarement voulu, donc Peras demande d’écrire l’ordre.
parse-bitwise-mix-fix = ajoute des parenthèses : `{ $suggestion }`

# E0110
parse-expected-item = déclaration attendue, trouvé { $eof ->
    [1] la fin du fichier
   *[other] `{ $found }`
}
parse-expected-item-label = seules des déclarations peuvent apparaître au niveau du module
parse-expected-item-why = Un module contient des déclarations : `fn`, `struct`, `let` et `const`, éventuellement précédées de `export` ou d’un attribut comme `@frame`. Les instructions vont dans le corps d’une fonction.
parse-expected-item-fix = déplace ce code dans une fonction

# E0111
parse-too-deep = imbrication trop profonde
parse-too-deep-label = plus de { $limit } niveaux de profondeur
parse-too-deep-why = L’analyse s’arrête à { $limit } niveaux d’expressions ou de types imbriqués, pour qu’aucun fichier, même hostile ou généré, ne puisse épuiser la pile du compilateur.
parse-too-deep-fix = découpe l’expression en variables intermédiaires nommées

# E0112
parse-const-without-value = `{ $keyword }` sans valeur
parse-const-without-value-label = `{ $name }` a besoin d’une valeur
parse-const-without-value-why = Un `const` reçoit sa valeur une seule fois, là où il est déclaré, et ne change jamais.
parse-const-without-value-fix = donne-lui une valeur avec `= …`, ou déclare-le avec `let` pour l’affecter plus tard

# E0113
parse-assignment-in-expression = une affectation n’est pas une expression
parse-assignment-in-expression-label = `{ $operator }` affecte ici
parse-assignment-in-expression-why = En Peras, une affectation est une instruction à part entière et n’a pas de valeur : `if (a = b)` ou `x = y = 0` ne peuvent donc pas cacher une erreur.
parse-assignment-in-expression-fix-compare = pour comparer, écris `==`
parse-assignment-in-expression-fix = fais de l’affectation une instruction à part, sur sa propre ligne

# E0114
parse-invalid-target = impossible d’affecter cette expression
parse-invalid-target-label = ni une variable, ni un champ, ni un élément
parse-invalid-target-why = Seuls une variable (`x`), un champ (`p.x`) ou un élément (`a[i]`) peuvent être affectés, incrémentés ou décrémentés.
parse-invalid-target-fix = affecte plutôt une variable, un champ ou un élément

# E0115
parse-condition-parentheses = la condition de `{ $keyword }` demande des parenthèses
parse-condition-parentheses-label = cette condition
parse-condition-parentheses-why = Comme en TypeScript, la condition de `if` et de `while` s’écrit entre parenthèses : `if (x > 0)`.
parse-condition-parentheses-fix = mets la condition entre parenthèses

# E0116
parse-for-in = `for…in` n’existe pas en Peras
parse-for-in-label = écris `of`
parse-for-in-why = En JavaScript, `for…in` parcourt les clés d’un objet, héritées comprises, sous forme de chaînes ; Peras n’a que `for…of`, qui parcourt les valeurs d’une collection.
parse-for-in-fix = remplace `in` par `of`

# E0117
parse-jump-outside-loop = `{ $keyword }` hors d’une boucle
parse-jump-outside-loop-label = aucune boucle autour de ce `{ $keyword }`
parse-jump-outside-loop-why = `{ $keyword }` ne fonctionne qu’à l’intérieur d’un `for` ou d’un `while`, dans la même fonction.
parse-jump-outside-loop-fix = supprime `{ $keyword }`, ou utilise `return` pour quitter la fonction

# E0118
parse-optional-marker = `?:` n’existe pas en Peras
parse-optional-marker-label = marqueur optionnel
parse-optional-marker-why = Peras n’a pas de `undefined` ni de `null` implicites : une valeur qui peut manquer a le type `Option<T>`, et le compilateur oblige à traiter les deux cas.
parse-optional-marker-fix = écris le type `Option<{ $type }>`

# E0119
parse-empty-body = `;` juste après `{ $keyword }` le laisse vide
parse-empty-body-label = ce `;` termine le `{ $keyword }`
parse-empty-body-why = En JavaScript, `if (c); {"{"} … {"}"}` exécute le bloc à chaque fois : le `;` est tout le corps. Peras refuse cette erreur silencieuse ; un corps volontairement vide s’écrit `{"{"}{"}"}`.
parse-empty-body-fix = supprime le `;`, ou écris `{"{"}{"}"}` pour un corps vide

## Rendu des diagnostics

diag-error = erreur
diag-why = pourquoi
diag-fix = correction
diag-details = détails
