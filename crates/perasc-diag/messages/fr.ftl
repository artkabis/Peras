# Messages français — traduction de en.ftl, qui fait foi.
# Chaque clé de en.ftl doit exister ici ; sinon la CI échoue.

cli-version = perasc { $version }
cli-usage =
    Utilisation : perasc <commande> [options]

    Commandes :
      check <fichier>... vérifie des fichiers .peras et .apeiron et signale toutes les erreurs
      build <fichier>    compile un fichier .peras en WebAssembly, ou un fichier .apeiron en JavaScript
      explain <code>     explique un code d’erreur, par exemple `perasc explain E0001`

    Options :
      --lang <en|fr>     langue des messages (par défaut : PERAS_LANG, puis la locale du système)
      --json             affiche les diagnostics de `check` et `build` en JSON sur la sortie standard
      --backend <nom>    générateur de `build` : direct (par défaut, aucun autre outil) ou rust (cargo, wasm-bindgen)
      --out <dossier>    dossier de sortie de `build` (par défaut : dist)
      --no-optimize      saute les optimisations de l’IR dans `build` (backend direct)
      --allocator <nom>  le build : debug (défaut, chaque bug mémoire et chaque débordement d’entier attrapés) ou tlsf (production, temps borné, les entiers bouclent)
      --simd <on|off>    SIMD de WebAssembly dans les boucles et le runtime : on (par défaut, plus rapide) ou off (se charge sans lui, comme Safari avant 16.4)
      --threads <on|off> boucles `parallel for` sur plusieurs fils, là où la page est isolée entre origines : off (par défaut) ou on
      --link <on|off>    le runtime lié dans le .wasm du module, ses appels internes à un seul module (décision 0016) : on (par défaut) ou off
      --imports <mode>   imports de fichiers .peras dans un module .apeiron : built (par défaut, chacun compilé à côté) ou source (gardés pour un bundler comme Vite)
      --color <quand>    colore les diagnostics : auto (par défaut), always, never ; NO_COLOR est respecté
      -V, --version      affiche la version
      -h, --help         affiche cette aide
cli-unknown-argument = argument inconnu `{ $argument }`
cli-unknown-command = commande inconnue `{ $command }`
cli-missing-value = `{ $option }` attend une valeur
cli-invalid-value = valeur `{ $value }` invalide pour `{ $option }`
cli-no-files = `check` attend au moins un fichier
cli-no-build-file = `build` attend un fichier .peras ou .apeiron
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
cli-build-ok = { $file } compilé dans { $out } ({ $bytes } octets, { $ms } ms)
cli-write-error = impossible d’écrire la sortie : { $reason }
cli-tool-missing = `{ $tool }` est nécessaire au backend Rust mais n’est pas installé : installe-le, ou utilise `--backend direct`
cli-tool-failed = `{ $tool }` a échoué sur du code généré par perasc : c’est un bug de perasc, signale-le avec la sortie ci-dessous
cli-internal-error = le vérificateur d’IR a refusé le code produit par perasc : c’est un bug de perasc, signale-le avec les lignes ci-dessous
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
    [enum] nom d’énumération attendu
    [variant] nom de variante attendu
    [type-parameter] nom de paramètre de type attendu
    [constraint] nom de contrainte attendu
    [type] nom de type attendu
    [interface] nom d’interface attendu
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

## Noms et types (E0200–E0299)

check-did-you-mean = voulais-tu dire `{ $name }` ?

# E0200
check-undefined = `{ $name }` n’est pas défini
check-undefined-label = introuvable dans cette portée
check-undefined-why = Un nom doit être déclaré avant d’être utilisé, dans cette portée ou une portée englobante : avec `let`, `const`, `fn` ou `struct`, ou comme paramètre.
check-undefined-fix = déclare-le, ou vérifie son orthographe

# E0201
check-shadowing = `{ $name }` masque une autre déclaration
check-shadowing-label = ce `{ $name }` masque l’autre
check-shadowing-previous-label = déclaré d’abord ici
check-shadowing-why = Peras refuse le masquage : deux variables de même nom dans des portées imbriquées rendent facile de lire ou de modifier la mauvaise.
check-shadowing-fix = donne un autre nom à ce `{ $name }`

# E0202
check-duplicate = `{ $name }` est déclaré deux fois
check-duplicate-label = déclaré à nouveau ici
check-duplicate-previous-label = déclaré d’abord ici
check-duplicate-why = Un nom se déclare une fois par portée : fonctions, structures et constantes du module partagent un même espace de noms, comme les champs d’une structure et les paramètres d’une fonction.
check-duplicate-fix = renomme ou supprime l’un des deux `{ $name }`

# E0203
check-unused = { $what ->
    [parameter] le paramètre `{ $name }` n’est jamais lu
   *[other] la variable `{ $name }` n’est jamais lue
}
check-unused-label = jamais lu
check-unused-why = Une valeur jamais lue est du code mort, et souvent un bug : une faute de frappe, ou un usage oublié. Un nom qui commence par `_` marque un paramètre comme volontairement inutilisé.
check-unused-fix-parameter = s’il est inutilisé volontairement, nomme-le `_{ $name }`
check-unused-fix-variable = supprime-la, ou utilise sa valeur

# E0204
check-moved = `{ $name }` est utilisé après avoir été déplacé
check-moved-label = utilisé ici, après le déplacement
check-moved-given-label = `{ $name }` reçoit sa valeur ici
check-moved-here-label = déplacé ici
check-moved-loop-label = déplacé ici au tour précédent de la boucle
check-moved-why = Une valeur sur le tas, un tableau, une chaîne ou une structure qui en contient, a un seul propriétaire. L’affecter, la renvoyer ou la passer à un paramètre `sink` la déplace : l’ancien nom ne contient plus rien tant qu’il ne reçoit pas une nouvelle valeur.
check-moved-fix = donne une nouvelle valeur à la variable avant de l’utiliser à nouveau
check-fix-copy = déplace plutôt une copie, avec `.copy()`

# E0205
check-immutable = impossible de modifier { $kind ->
    [parameter] le paramètre
   *[other] la constante
} `{ $name }`
check-immutable-label = modifié ici
check-immutable-declared-label = { $kind ->
    [parameter] paramètre en lecture seule
   *[other] déclaré avec `const`
}
check-immutable-why = Un `const` ne change jamais, et un paramètre est en lecture seule sauf s’il est passé `inout`, ce qui montre à l’appelant que la fonction le modifie.
check-immutable-fix-const = déclare `{ $name }` avec `let`
check-immutable-fix-parameter = passe `{ $name }` en `inout`

# E0206
check-inout-marker = { $kind ->
    [missing] un argument `inout` s’écrit avec `&`
   *[other] `&` ne marque que les arguments `inout`
}
check-inout-marker-label = { $kind ->
    [missing] passé à un paramètre `inout` sans `&`
   *[other] ce paramètre n’est pas `inout`
}
check-inout-marker-why = `&` à l’appel montre que la fonction peut modifier la variable. Il est exigé pour un paramètre `inout`, et refusé partout ailleurs, pour qu’il veuille toujours dire cela.
check-inout-marker-fix-add = écris `&` devant l’argument
check-inout-marker-fix-remove = retire le `&`

# E0207
check-alias = `{ $name }` est passé deux fois à `{ $callee }`, et l’un des deux le modifie
check-alias-label = atteint de nouveau la même variable
check-alias-other-label = atteinte une première fois ici
check-alias-why = Un paramètre `inout` modifie sa variable sur place. Un autre argument qui atteindrait la même variable la verrait changer pendant l’appel : Peras le refuse.
check-alias-fix = passe deux variables différentes
check-alias-loop = `{ $name }` change pendant qu’une boucle le parcourt
check-alias-loop-label = modifié ici
check-alias-loop-iterated-label = la boucle le parcourt
check-alias-loop-fix = parcours une copie, `{ $name }.copy()`, pour modifier l’original

# E0209
check-move-out = impossible de déplacer { $what ->
    [parameter] le paramètre `{ $name }`, qui ne fait qu’emprunter sa valeur
    [field] un champ hors de sa structure
    [element] un élément hors de son tableau
    [binding] une valeur hors du `match` ou de la fonction fléchée qui ne fait que la prêter
   *[other] une valeur du niveau du module
}
check-move-out-label = déplacé ici
check-move-out-why = Seul le propriétaire d’une valeur peut la déplacer. Un paramètre qui n’est pas `sink` emprunte la valeur de l’appelant, et un champ ou un élément appartient à ce qui le contient.
check-move-out-fix = déplace une copie, avec `.copy()`, ou déplace le propriétaire en entier
check-move-out-fix-sink = déclare `{ $name }` en `sink` : la fonction prend alors la valeur

# E0210
check-mismatch = types incompatibles : `{ $expected }` attendu, `{ $found }` trouvé
check-mismatch-label = ceci est `{ $found }`
check-mismatch-why = Peras ne convertit jamais une valeur implicitement, pas même entre nombres : `i32 + f64` est une erreur, car une conversion silencieuse peut perdre de la précision ou déborder.
check-mismatch-fix-convert = convertis-le explicitement avec `.{ $method }()`
check-mismatch-fix = utilise une valeur de type `{ $expected }`

# E0211
check-integer-for-float = entier littéral là où `{ $expected }` est attendu
check-integer-for-float-label = `{ $literal }` est un entier
check-integer-for-float-why = Un entier littéral ne devient jamais un flottant implicitement ; le point décimal rend l’intention visible.
check-integer-for-float-fix = écris `{ $literal }`

# E0212
check-out-of-range = `{ $literal }` ne tient pas dans `{ $type }`
check-out-of-range-label = `{ $type }` contient les valeurs de { $min } à { $max }
check-out-of-range-why = Un littéral est vérifié par rapport à son type dès la compilation, pour qu’il ne déborde ni ne boucle jamais silencieusement.
check-out-of-range-fix = utilise une valeur dans les bornes, ou un type plus large

# E0213
check-unknown-type = type `{ $name }` inconnu
check-unknown-type-label = n’est pas un type
check-unknown-type-why = Un type est soit intégré (`i32`, `f64`, `bool`, `String`, `Array<T>`, `Option<T>`…), soit une `struct` déclarée dans le module.
check-unknown-type-fix = déclare une `struct` de ce nom, ou utilise un type intégré

# E0214
check-argument-count = `{ $name }` prend { $expected ->
    [one] 1 argument
   *[other] { $expected } arguments
}, { $found } trouvé(s)
check-argument-count-label = { $found ->
    [one] 1 argument donné
   *[other] { $found } arguments donnés
}
check-argument-count-declared-label = { $expected ->
    [one] prend 1 argument
   *[other] prend { $expected } arguments
}
check-argument-count-why = Chaque paramètre reçoit exactement un argument, dans l’ordre : Peras n’a ni paramètres optionnels ni `undefined` implicite.
check-argument-count-fix = passe exactement { $expected ->
    [one] 1 argument
   *[other] { $expected } arguments
}

# E0215
check-unknown-member = `{ $type }` n’a ni champ ni méthode `{ $member }`
check-unknown-member-label = n’est pas un membre de `{ $type }`
check-unknown-member-why = Les champs d’une structure viennent de sa déclaration ; les types intégrés ont un ensemble fixe de membres, comme `length` ou `toF64()`.
check-unknown-member-fix = vérifie le nom du champ ou de la méthode

# E0216
check-type-argument-count = `{ $name }` prend { $expected } argument(s) de type, { $found } trouvé(s)
check-type-argument-count-label = { $expected } attendu(s)
check-type-argument-count-why = Un type ou une fonction prend autant d’arguments de type qu’il a de paramètres de type : un pour `Array` et `Option`, comme dans `Array<f32>`, deux pour `Result`, le nombre déclaré pour un générique, et aucun pour les autres.
check-type-argument-count-fix = écris `{ $name }` avec { $expected } argument(s) de type

# E0217
check-condition = condition de type `{ $found }`, `bool` attendu
check-condition-label = ceci est `{ $found }`, pas `bool`
check-condition-why = Peras n’a pas de « truthiness » : `0`, `""` et `None` ne valent pas faux. Une condition est un `bool`, pour que son sens soit toujours explicite.
check-condition-fix-compare = compare explicitement : `{ $comparison }`
check-condition-fix = écris une comparaison qui donne un `bool`

# E0218
check-return = { $kind ->
    [value] cette fonction ne renvoie pas de valeur
   *[other] valeur de retour de type `{ $expected }` manquante
}
check-return-label = { $kind ->
    [value] valeur renvoyée ici
   *[other] `return` sans valeur
}
check-return-declared-label = la fonction renvoie `{ $expected }`
check-return-why = Une fonction renvoie exactement ce que dit son type de retour : une valeur de ce type à chaque `return`, ou rien quand il est `void`.
check-return-fix = { $kind ->
    [value] supprime la valeur, ou déclare un type de retour
   *[other] renvoie une valeur du type déclaré
}

# E0219
check-missing-return = `{ $name }` peut se terminer sans renvoyer de valeur
check-missing-return-label = la fin de la fonction peut être atteinte ici
check-missing-return-declared-label = renvoie `{ $expected }`
check-missing-return-why = Une fonction avec un type de retour renvoie une valeur sur chaque chemin ; atteindre la fin de son corps ne renverrait rien.
check-missing-return-fix = termine chaque chemin par un `return` de type `{ $expected }`

# E0220
check-missing-return-type = `{ $name }` n’a pas de type de retour
check-missing-return-type-label = { $reason ->
    [exported] une fonction exportée déclare son type de retour
   *[other] renvoie une valeur sans déclarer son type
}
check-missing-return-type-why = Les signatures publiques sont toujours écrites, pour que l’interface d’un module ne dépende jamais de ce que son corps renvoie ; une fonction qui renvoie une valeur déclare son type.
check-missing-return-type-fix = ajoute le type de retour `{ $type }`

# E0221
check-operator = `{ $operator }` ne s’applique pas à `{ $type }`
check-operator-label = ceci est `{ $type }`
check-operator-why = Chaque opérateur s’applique à des types précis : l’arithmétique aux nombres, `+` aussi aux chaînes, `&`, `|`, `^`, `~`, `<<` et `>>` aux entiers, `!`, `&&` et `||` aux `bool`, le `-` unaire aux nombres signés, `[]` aux tableaux.
check-operator-fix = convertis la valeur, ou utilise une autre opération

## Génération de code (E0900–E0999)

# E0222
check-match-missing = ce `match` sur `{ $ty }` ne couvre pas { $missing }
check-match-missing-label = ajoute un bras pour { $missing }
check-match-missing-match-label = toute valeur doit atteindre un bras
check-match-missing-why = Un `match` doit traiter chaque valeur qu’il peut recevoir : chaque variante d’une énumération ou d’une option, `true` et `false`, et pour les nombres ou les chaînes un `_` final. Oublier un cas est précisément le bug qu’un `match` exhaustif sert à attraper.
check-match-missing-fix = ajoute les bras manquants, ou un bras final `_ => …` volontairement

# E0223
check-unknown-variant = `{ $ty }` n’a pas de variante `{ $name }`
check-unknown-variant-label = variante inconnue
check-unknown-variant-why = Les variantes d’une énumération sont celles que sa déclaration liste ; une option a `None` et `Some`.
check-unknown-variant-fix = utilise l’une des variantes que liste la déclaration

# E0224
check-unreachable-arm = ce bras ne peut jamais être atteint
check-unreachable-arm-label = les bras précédents couvrent déjà ces valeurs
check-unreachable-arm-why = Les bras sont essayés dans l’ordre : après `_`, un nom, ou une variante déjà couverte, il ne reste aucune valeur pour celui-ci. C’est du code mort, ou des bras dans le mauvais ordre.
check-unreachable-arm-fix = retire le bras, ou place-le avant celui qui le couvre

# E0225
check-try-return = `?` sur { $kind ->
    [option] une option
   *[other] un résultat
} dans une fonction qui renvoie `{ $ret }`
check-try-return-label = renvoie tout de suite { $kind ->
    [option] `None`
   *[other] l’`Err`
}
check-try-return-why = `?` donne la valeur à l’intérieur, ou renvoie aussitôt ce qu’il ne peut pas donner : `None` pour une option, l’erreur pour un résultat. La fonction doit pouvoir renvoyer cela : une option, ou un résultat avec le même type d’erreur.
check-try-return-fix = { $kind ->
    [option] renvoie une `Option`, ou traite `None` avec `match` ou `unwrapOr`
   *[other] renvoie un `Result` avec ce type d’erreur, ou traite l’erreur avec `match`
}

# E0226
check-unknown-constraint = contrainte inconnue `{ $name }`
check-unknown-constraint-label = pas une contrainte
check-unknown-constraint-why = Un paramètre de type est contraint par ce que ses types doivent savoir faire : `Eq` (`==`), `Ord` (comparaisons), `Numeric` (arithmétique, pour entiers et flottants), `Integer`, `Float`, ou `Copy` (copié, pas déplacé).
check-unknown-constraint-fix = utilise `Eq`, `Ord`, `Numeric`, `Integer`, `Float` ou `Copy`

# E0227
check-constraint = `{ $ty }` n’est pas `{ $bound }`
check-constraint-label = `{ $param }` est `{ $ty }` ici
check-constraint-declared = `{ $param }` doit être `{ $bound }`
check-constraint-why = Une fonction ou un type générique fonctionne pour tout type qui respecte les contraintes de ses paramètres, et seulement pour ceux-là : son corps s’appuie dessus, donc un type qui n’en respecte pas une le ferait échouer.
check-constraint-fix = donne à `{ $param }` un type qui est `{ $bound }`

# E0228
check-missing-constraint = `{ $op }` sur `{ $param }`, qui peut ne pas le permettre
check-missing-constraint-label = `{ $op }` demande `{ $bound }`
check-missing-constraint-why = Un corps générique est vérifié une fois, pour tous les types que ses paramètres peuvent représenter. Sans contrainte, un paramètre de type peut être n’importe quel type, et tous les types ne permettent pas toutes les opérations ; la contrainte affirme que si, et chaque appel est vérifié par rapport à elle.
check-missing-constraint-fix = contrains le paramètre : `{ $param }: { $bound }`

# E0229
check-cannot-infer = impossible de savoir quel type représente `{ $param }` de `{ $name }`
check-cannot-infer-label = `{ $param }` est inconnu ici
check-cannot-infer-declared = `{ $param }` déclaré ici
check-cannot-infer-why = Les arguments de type d’une fonction générique viennent de ses arguments, ou du type que l’appel doit avoir. Ici, rien ne dit lequel.
check-cannot-infer-fix = donne un type au résultat, comme `let xs: Array<i32> = …`, ou passe une valeur de type `{ $param }`

# E0230
check-generic-export = la fonction exportée `{ $name }` est générique
check-generic-export-label = exportée, avec des paramètres de type
check-generic-export-why = Le code générique est compilé une fois par jeu d’arguments de type que le module utilise. JavaScript appelle une fonction exportée avec n’importe quelle valeur : il n’y a pas de copie unique à exporter.
check-generic-export-fix = exporte une fonction non générique qui appelle `{ $name }` avec les types voulus

# E0231
check-endless-instances = `{ $name }` demanderait des copies sans fin
check-endless-instances-label = instancié avec des types toujours plus grands
check-endless-instances-why = Chaque jeu d’arguments de type donne une copie du code générique. Ici, une copie en demande une autre avec un type plus grand, comme quand `f<T>` appelle `f<Array<T>>`, ou qu’une structure se contient avec un argument de type plus grand : cela ne finit pas.
check-endless-instances-fix = utilise les mêmes paramètres de type dans l’appel ou le champ récursif

# E0232
check-alias-cycle = l’alias de type `{ $name }` se contient lui-même
check-alias-cycle-label = renvoie à lui-même, directement ou par d’autres alias
check-alias-cycle-why = Un alias est un autre nom pour un type : le remplacer par ce qu’il nomme doit finir. Un type qui se contient demande une structure ou une énumération, dont le compilateur sait calculer la taille.
check-alias-cycle-fix = fais-en une structure ou une énumération

# E0233
check-not-constant = la valeur d’une constante de module doit être connue à la compilation
check-not-constant-label = calculé quand le programme s’exécute
check-not-constant-why = Une `const` de module est écrite dans le code partout où elle sert : sa valeur est faite de littéraux, d’autres constantes, d’opérateurs, de tableaux, de tuples, de structures, d’options et de variantes. Un appel ne s’exécute que quand le programme tourne.
check-not-constant-fix = calcule-la dans une fonction, ou écris sa valeur

# E0234
check-no-methods = `{ $ty }` ne peut pas avoir de méthodes ici
check-no-methods-label = pas une structure ni une énumération de ce module
check-no-methods-why = Un `impl` donne des méthodes à une structure ou une énumération déclarée dans le module. Les autres types ont les méthodes prédéfinies de la référence des méthodes.
check-no-methods-fix = enveloppe la valeur dans une structure du module, et donne-lui les méthodes

# E0235
check-impl-missing = l’implémentation de `{ $interface }` pour `{ $ty }` n’a pas { $missing }
check-impl-missing-label = { $missing } manquant
check-impl-extra = `{ $name }` n’est pas une méthode de `{ $interface }`
check-impl-extra-label = `{ $interface }` ne déclare pas cette méthode
check-impl-mismatch = une implémentation qui ne correspond pas à son interface
check-impl-mismatch-why = Un `impl Interface for Type` fournit exactement les méthodes que l’interface déclare : le code qui s’appuie sur l’interface appelle chacune d’elles, et rien d’autre n’a sa place là.
check-impl-missing-fix = ajoute les méthodes manquantes, avec les signatures que déclare `{ $interface }`
check-impl-extra-fix = déplace `{ $name }` dans un `impl` du type lui-même

# E0236
check-signature = `{ $name }` ne correspond pas à `{ $interface }`
check-signature-label = attendu `{ $expected }`
check-signature-declared = déclarée dans `{ $interface }` ici
check-interface-self = la méthode d’interface `{ $name }` ne prend pas `self`
check-interface-self-label = `self` doit venir en premier
check-signature-why = Une méthode d’interface est appelée sur une valeur de n’importe quel type qui l’implémente : elle prend `self` en premier, et chaque implémentation a exactement ses paramètres, leurs conventions et son type de retour.
check-signature-fix = écris-la `{ $expected }`
check-interface-self-fix = écris `fn { $name }(self, …)`

# E0237
check-interface-as-type = l’interface `{ $name }` utilisée comme un type
check-interface-as-type-label = une interface, pas un type
check-interface-as-type-why = Une interface est une contrainte : elle dit ce qu’un type sait faire, pas quel type a une valeur. Le code qui marche pour tout type qui l’implémente est générique, et compilé une fois par type, sans coût à l’exécution.
check-interface-as-type-fix = utilise un paramètre de type : `fn f<T: { $name }>(x: T)`

# E0238
check-arrow = une fonction fléchée n’est une valeur que dans un module managed
check-arrow-label = pas l’argument d’une méthode qui prend une fonction
check-arrow-expected = cette méthode prend une fonction fléchée
check-arrow-expected-label = attendu `(x) => …`
check-arrow-why = Dans un module owned, une fonction fléchée est compilée sur place, comme le corps d’une boucle, par les méthodes de tableaux qui en prennent une : `map`, `filter`, `reduce`, `find`, `findIndex`, `some` et `every`. Dans un module managed, une fonction fléchée est aussi une valeur : gardée dans une variable, passée à une fonction dont le paramètre a un type de fonction, `(Event) => void`, et appelée ; elle voit les variables autour d’elle, et change celles déclarées avec `let`. Ses paramètres ont alors leur type écrit, puisque rien d’autre ne le donne, et un type de fonction ne s’écrit que pour une variable, un paramètre ou un résultat, là où le vérificateur peut la suivre. Une fonction exportée ne peut pas encore en prendre ni en rendre : les valeurs que rendrait une fonction de JavaScript ne seraient pas vérifiées.
check-arrow-fix = passe-la à `map`, `filter`, `reduce`, `find`, `findIndex`, `some` ou `every`, ou écris une fonction avec `fn`
check-arrow-param = le type de `{ $name }` n’est pas écrit
check-arrow-param-label = un paramètre d’une fonction fléchée utilisée comme valeur
check-arrow-param-fix = écrivez son type : `({ $name }: Event) => …`
check-arrow-type = un type de fonction ne peut pas s’écrire ici
check-arrow-type-label = à l’intérieur d’un autre type
check-arrow-type-fix = n’écrivez un type de fonction que pour une variable, un paramètre ou un résultat
check-arrow-export = la fonction exportée `{ $name }` prend ou rend une fonction
check-arrow-export-label = un type de fonction, dans une fonction exportée
check-arrow-export-fix = gardez la fonction dans le module, et exportez-en une qui prend des données
check-arrow-expected-fix = écris la fonction sur place : `xs.map((x) => x * 2)`
check-template = un gabarit ne sait pas encore écrire un `{ $type }` en texte
check-template-label = un `{ $type }`
check-template-why = Un gabarit écrit chacune de ses valeurs en texte : une chaîne telle quelle, un nombre ou un booléen comme l’écrit `toString()`, comme le fait JavaScript. Une structure, un tableau ou une option n’a pas encore de texte à elle : quels champs, dans quel ordre, c’est au programme de le dire.
check-template-fix = mettez-y ses champs, une valeur chacun
check-map-key = un `{ $type }` ne peut pas encore être une clé de table ni un élément d’ensemble
check-map-key-label = une clé `{ $type }`
check-map-key-why = Une table retrouve ses clés par leur hachage et leur égalité, écrits dans son runtime pour les entiers, les booléens et les chaînes. Une structure, une énumération ou un flottant demande une règle pour les deux : un flottant, parce que `NaN` n’est pas égal à lui-même et que `-0` est égal à `+0`.
check-map-key-fix = utilisez un entier, un booléen ou une chaîne comme clé : un identifiant, ou une clé construite en texte
check-parallel = cette boucle ne peut pas tourner en parallèle : { $reason ->
    [shape] une boucle parallèle s’écrit `parallel for (let i = début; i < fin; i++)`
    [outer] elle affecte `{ $name }`, déclarée hors de la boucle
    [index] elle écrit un élément à un autre indice que celui de son compteur
    [reads] elle lit, à un autre indice que celui de son compteur, un tableau qu’elle écrit
    [calls] elle appelle une fonction
    [heap] elle crée, agrandit, déplace ou libère une chaîne, un tableau ou une table
    [exits] elle sort de la boucle par `break` ou `return`
   *[other] ses tours pourraient se voir
}
check-parallel-label = des tours qui peuvent tourner en même temps
check-parallel-why = Les tours d’une boucle parallèle tournent sur plusieurs fils à la fois, dans n’importe quel ordre. Chaque tour peut lire tout ce que la boucle ne change pas, et n’écrire que les éléments à son propre indice, pour qu’aucun tour n’en voie un autre : les résultats sont alors ceux de la boucle exécutée dans l’ordre, bit pour bit.
check-parallel-fix = ne gardez dans chaque tour que ce qui lit les entrées et écrit `xs[i]` ; calculez une somme ou ce qui demande de la mémoire dans une boucle ordinaire après

# E0242
check-managed-only = { $what ->
    [async] une fonction `async`
    [await] `await`
    [await-sync] `await` hors d’une fonction `async`
    [number] le type `number`
    [promise] le type `Promise`
    [import] un `import`
    [browser] l’API du navigateur
    [extern] un bloc `extern`
    [arrow-block] une fonction fléchée à instructions
    [function-type] un type de fonction
   *[other] ceci
} { $what ->
    [await-sync] ne peut pas attendre
   *[other] appartient aux modules managed (`.apeiron`)
}
check-managed-only-label = { $what ->
    [await-sync] attend dans une fonction qui n’attend pas
   *[other] seulement dans un module `.apeiron`
}
check-managed-only-why = Un module owned (`.peras`) se compile en WebAssembly, où rien n’attend : il calcule et rend. Un module managed (`.apeiron`) se compile en JavaScript, pour ce qui parle au navigateur : il attend avec `async` et `await`, compte avec `number`, et appelle les modules owned en les important. `await` attend une promesse ; seule une fonction qui rend elle-même une promesse, une fonction `async`, peut donc l’utiliser.
check-managed-only-fix = { $what ->
    [await-sync] marquez la fonction `async`, et son résultat `Promise<T>`
    [number] utilisez `f64`, que `number` nomme dans un module managed
    [browser] utilisez le navigateur dans un module `.apeiron`, et donnez les données à ce module
   *[other] déplacez ce code dans un module `.apeiron`, qui peut importer celui-ci
}

# E0243
check-owned-only = { $what ->
    [inout] `inout` sur une valeur que JavaScript copie
    [inout-assign] remplacer un paramètre `inout`
    [sink] `sink`
    [parallel] `parallel for`
   *[other] ceci
} appartient aux modules owned (`.peras`)
check-owned-only-label = seulement dans un module `.peras`
check-owned-only-why = Les valeurs d’un module managed appartiennent au ramasse-miettes de JavaScript : rien n’est déplacé, rien n’est prêté. Un tableau, une table, un ensemble ou une structure est partagé avec l’appelant : un `inout` peut donc être changé, et l’appelant le voit ; un nombre, un booléen, une chaîne ou une option est copié, et un `inout` ne pourrait pas atteindre son appelant, pas plus qu’un paramètre remplacé par une autre valeur. Donner une valeur avec `sink`, et les boucles qui partagent le travail entre fils sur une mémoire commune, sont propres aux modules owned, compilés en WebAssembly.
check-owned-only-fix = { $what ->
    [parallel] écrivez la boucle dans un module `.peras` et importez sa fonction
    [inout-assign] changez ses éléments ou ses champs, ou rendez la nouvelle valeur
   *[other] retirez la convention : rendez la nouvelle valeur au lieu de changer l’argument
}

# E0244
check-import = { $reason ->
    [module] impossible d’importer depuis `{ $name }`
    [name] `{ $name }` n’est pas une fonction, une structure ou une énumération exportée de ce module
    [signature] `{ $name }` prend ou rend une valeur qui ne passe pas encore d’un module à l’autre
    [clash] le module importé a un type `{ $name }`, et ce module en déclare un aussi
   *[other] impossible d’importer `{ $name }`
}
check-import-label = { $reason ->
    [module] pas un module owned qui se compile
    [name] non exporté ici
    [signature] importée ici
    [clash] importé ici
   *[other] ici
}
check-import-why = Un module managed importe des fonctions, des structures et des énumérations de modules owned (`.peras`), dans le même dossier ou en dessous, et appelle les fonctions à travers des conversions vérifiées : les nombres, les booléens, les chaînes, les structures et énumérations du module owned, et les tableaux, options, résultats, tables et ensembles de ceux-ci passent de l’un à l’autre. Une structure générique, une fonction ou un type du navigateur ne passe pas. Une structure ou une énumération importée est celle du module owned, sous son nom : ce module ne peut pas en déclarer une autre de ce nom.
check-import-fix = { $reason ->
    [module] importez un fichier `.peras` par son chemin relatif, comme `"./maths.peras"`, et corrigez d’abord ses erreurs
    [name] marquez-la `export` dans son module, ou vérifiez son orthographe
    [signature] donnez à la fonction des valeurs qui passent : des nombres, des chaînes, des structures et énumérations non génériques, et des tableaux de ceux-ci
    [clash] renommez le type propre de ce module, ou utilisez celui qui est importé
   *[other] vérifiez l’import
}

# E0245
check-resource = { $reason ->
    [dropped] cette ressource est libérée dès qu’elle est créée
    [moved] `{ $name }` est utilisé après avoir été donné
    [loop] `{ $name }` est donné dans une boucle
    [capture] `{ $name }` est une ressource, et une fonction fléchée ne peut pas la garder
    [holder] une ressource ne peut pas être gardée dans une structure, un tableau, une option ou une table
    [lent] `{ $name }` est prêté à cette fonction, qui ne peut pas le donner
    [reassign] `{ $name }` est prêté à cette fonction, qui ne peut pas le remplacer
    [global] `{ $name }` est une variable de module : rien ne libérerait la ressource qu’elle tient
   *[other] cette ressource ne peut pas être utilisée ici
}
check-resource-label = { $reason ->
    [dropped] créée ici, gardée par rien
    [moved] utilisé ici
    [loop] donné à chaque tour
    [capture] gardé par la fonction fléchée
    [holder] gardée ici
    [lent] donné ici
    [reassign] remplacé ici
    [global] gardée aussi longtemps que la page
   *[other] ici
}
check-resource-why = Un écouteur, un minuteur et un groupe sont des ressources : chacun appartient à une variable, et est libéré quand cette variable sort de sa portée, pour que rien ne continue d’écouter ou de battre une fois son propriétaire parti. Une ressource peut être donnée une fois : rendue, ajoutée à un `Group`, placée dans un `Slot`, ou libérée par `release()` ; ensuite, sa variable ne tient plus rien. Remplacer la ressource d’une variable `let` libère celle qu’elle tenait. Une ressource que rien ne garde serait libérée aussitôt, et une ressource gardée par une structure, un tableau ou une fonction fléchée n’aurait pas de propriétaire pour la libérer.
check-resource-fix = { $reason ->
    [dropped] gardez-la dans une variable, `const listener = …`, ajoutez-la à un `Group`, ou rendez-la
    [moved] donnez-la en dernier, ou gardez-la dans un `Group` et utilisez le groupe
    [loop] ajoutez chacune à un `Group` déclaré avant la boucle
    [capture] gardez-la dans un `Slot` d’un groupe, qu’une fonction fléchée peut utiliser : `const pending = group.slot();`, puis `pending.set(…)`
    [holder] gardez-la dans une variable ou un `Group`
    [lent] rendez-la depuis la fonction qui l’a créée, ou ajoutez-la à un `Group` là-bas
    [reassign] rendez la nouvelle ressource, ou gardez la ressource dans un `Slot` d’un groupe : `slot.set(…)` la remplace
    [global] gardez-la dans une fonction qui la rend, ou dans un `Group` que l’une rend
   *[other] gardez la ressource dans une variable
}

# E0246
check-read-only = `{ $member }` de `{ $type }` ne peut qu’être lu
check-read-only-label = affecté ici
check-read-only-why = Le navigateur fixe lui-même cette propriété : le type et la touche d’un évènement, le statut d’une réponse, le corps d’une page. Une page ne peut pas la changer.
check-read-only-fix = lisez-la dans une variable à vous, et changez celle-ci

# E0247
check-extern = { $reason ->
    [resource] `{ $name }` donnerait une ressource à JavaScript, qui ne peut pas la libérer
    [callback] un type fonction n’est permis que pour un paramètre d’une fonction `extern`
    [self] `self` doit être d’un type que déclare ce bloc `extern`
    [async] `{ $name }` est `async` : son résultat doit être une `Promise`
   *[other] cette déclaration `extern` ne peut pas être gardée
}
check-extern-label = { $reason ->
    [resource] une ressource ici
    [callback] un type fonction ici
    [self] pas un type de ce bloc
    [async] pas une `Promise`
   *[other] ici
}
check-extern-why = Un bloc `extern` déclare le JavaScript qu’appelle un module managed, avec des types Peras : à chaque appel, chaque valeur que JavaScript rend est vérifiée contre eux. Une ressource appartient à une variable de Peras, qui la libère : JavaScript la garderait sans la libérer. Un type fonction au premier niveau d’un paramètre est un rappel, que Peras donne et que JavaScript appelle ; dans un autre type, ou comme résultat, ce serait une fonction de JavaScript dont rien ne vérifie les valeurs. Une méthode se déclare avec un `self` d’un type du bloc, les valeurs que crée JavaScript. Une fonction `async` rend une promesse.
check-extern-fix = { $reason ->
    [resource] donnez plutôt les données à JavaScript, et gardez la ressource en Peras
    [callback] déclarez le rappel comme paramètre d’une fonction `extern`, et passez-y une fonction fléchée
    [self] déclarez le type dans ce bloc, `type Chart;`, et écrivez `self: Chart`
    [async] déclarez son résultat comme `Promise<T>`
   *[other] vérifiez la déclaration
}

# E0248
check-floating-promise = cette promesse n’est jamais attendue
check-floating-promise-label = s’exécute plus tard, et rien ne l’attend
check-floating-promise-why = Appeler une fonction `async` lance son travail et rend une promesse de son résultat. Sans `await`, les instructions qui suivent l’appel s’exécutent avant que ce travail soit fini, et une erreur qu’il lève est perdue. Oublier `await` est l’un des bugs les plus courants du code asynchrone.
check-floating-promise-fix = écrivez `await` devant l’appel, dans une fonction `async` ; pour la laisser tourner exprès, gardez-la : `const _saving = save();`

# E0249
check-sort-key = un tableau ne peut pas être trié par un `{ $type }`
check-sort-key-label = un `{ $type }`, ni un nombre ni une chaîne
check-sort-key-why = `sortBy` et `sortByDescending` calculent une clé par élément, une fois, puis trient les clés : les nombres dans l’ordre de `sort()`, -0 avant +0 et NaN en dernier, les chaînes dans l’ordre de leurs points de code Unicode. Les deux modes les trient sans appeler de fonction à chaque comparaison, en un temps proportionnel à la longueur pour les nombres. Une clé d’un autre type n’a pas d’ordre propre : quel champ d’abord, dans quel ordre, c’est au programme de le dire.
check-sort-key-fix = triez par un nombre ou une chaîne : un champ, `(o) => o.amount`, ou une clé construite en texte

# E0900
gen-unsupported = la génération de code ne prend pas encore en charge { $feature ->
    [struct] les structures
    [global] les constantes et variables de module
    [inout] les paramètres `inout`
    [for-of] les boucles `for…of`
    [place] les affectations de champs et d’éléments
    [function-value] les fonctions utilisées comme valeurs
    [borrow] l’opérateur d’emprunt `&`
    [float-remainder] `%` sur des flottants
    [string] les chaînes
    [array] les tableaux
    [array-element] les tableaux d’autre chose que des nombres et des booléens
    [heap-option] les options qui contiennent un tableau ou une chaîne
    [inout-string] les chaînes `inout` des fonctions exportées, que JavaScript ne peut pas modifier
    [inout-export] les nombres et options `inout` des fonctions exportées : JavaScript les passe par valeur
    [deep-equality] `==` sur des tableaux de tables, d’ensembles, ou d’options qui contiennent des chaînes ou des tableaux
    [field] les champs
    [index] l’indexation
    [try] l’opérateur `?`
    [option] les valeurs `Option`
    [method] cette méthode
    [integer-type] les types entiers autres que `i32` et `i64`
    [recursive-type] les types qui se contiennent eux-mêmes
    [map-value] les tables dont les valeurs ont plus de 24 scalaires
    [nested-option] dans un module managed, les options d’options, dont JavaScript ne distingue pas `Some(None)` de `None`
   *[other] cette construction
}
gen-unsupported-label = du Peras valide, pas encore compilé
gen-unsupported-why = Le programme est correct : les vérifications l’ont accepté. La génération de code se construit par étapes, et cette construction n’est pas encore prise en charge ; la page « Ce qui fonctionne aujourd’hui » de la documentation dit, fonctionnalité par fonctionnalité, ce qui est compilé.
gen-unsupported-fix = n’utilise pour l’instant que les constructions prises en charge dans ce module, ou limite-le à `perasc check`

## Rendu des diagnostics

diag-error = erreur
diag-why = pourquoi
diag-fix = correction
diag-details = détails

## Exécution : bugs mémoire détectés par l’allocateur de débogage
# Complétés par le JavaScript généré : { $alloc } et { $release } sont des sites,
# comme `f (alloc 1)` ; { $count } et { $size } sont des nombres.

runtime-out-of-memory = mémoire épuisée : le bloc alloué à { $alloc } ne tient pas
runtime-too-large = { $alloc } demande { $size } octets d’un coup, plus de 1 Gio
runtime-overflow = des octets ont été écrits après la fin du bloc alloué à { $alloc } ; vu quand { $release } l’a libéré
runtime-invalid-release = { $release } libère quelque chose qui n’est pas un bloc vivant
runtime-double-release = le bloc alloué à { $alloc } est libéré une seconde fois, à { $release }
runtime-use-after-release = le bloc alloué à { $alloc } a été modifié après que { $release } l’a libéré
runtime-leak = { $count } bloc(s) encore vivant(s) à la fin ; le premier a été alloué à { $alloc }
runtime-index = l’indice { $index } est hors limites : la longueur est { $length }
runtime-integer-overflow = un entier a débordé : son type ne peut pas contenir le résultat. Pour boucler exprès, utilisez `wrappingAdd`, `wrappingSub` ou `wrappingMul` ; un build de production boucle
runtime-simd = ce module utilise le SIMD de WebAssembly, que ce navigateur n’a pas : Chrome ou Edge 91, Firefox 89 et Safari 16.4 l’ont. Compilez-le avec `--simd off` pour le charger partout.
