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

## Rendu des diagnostics

diag-error = erreur
diag-why = pourquoi
diag-fix = correction
diag-details = détails
