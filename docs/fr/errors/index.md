# Codes d'erreur

Une page par code, nommée d'après lui (`E0204.md`). Chaque page explique la cause, montre un exemple fautif et sa correction. Les codes sont permanents : un code retiré garde sa page, marquée comme retirée.

Les codes `E0001` à `E0099` sont réservés au lexer, `E0100` à `E0199` au parser.

| Code              | Erreur                                        | Étape  |
| ----------------- | --------------------------------------------- | ------ |
| [E0001](E0001.md) | Caractère invalide                            | lexer  |
| [E0002](E0002.md) | Chaîne non terminée                           | lexer  |
| [E0003](E0003.md) | Commentaire de bloc non terminé               | lexer  |
| [E0004](E0004.md) | Séquence d'échappement invalide               | lexer  |
| [E0005](E0005.md) | Nombre mal formé                              | lexer  |
| [E0006](E0006.md) | Opérateur d'égalité stricte                   | lexer  |
| [E0007](E0007.md) | Caractère de contrôle bidirectionnel          | lexer  |
| [E0008](E0008.md) | Fichier source trop volumineux                | lexer  |
| [E0100](E0100.md) | Expression attendue                           | parser |
| [E0101](E0101.md) | Point-virgule manquant                        | parser |
| [E0102](E0102.md) | Nom attendu                                   | parser |
| [E0103](E0103.md) | Délimiteur non fermé                          | parser |
| [E0104](E0104.md) | Symbole inattendu                             | parser |
| [E0105](E0105.md) | Type attendu                                  | parser |
| [E0106](E0106.md) | Mot-clé supprimé                              | parser |
| [E0107](E0107.md) | Nombre trop grand                             | parser |
| [E0108](E0108.md) | Comparaisons enchaînées                       | parser |
| [E0109](E0109.md) | Opérateur bit à bit mélangé à une comparaison | parser |
| [E0110](E0110.md) | Déclaration attendue                          | parser |
| [E0111](E0111.md) | Imbrication trop profonde                     | parser |
| [E0112](E0112.md) | Const sans valeur                             | parser |
| [E0113](E0113.md) | Affectation utilisée comme expression         | parser |
| [E0114](E0114.md) | Cible d’affectation invalide                  | parser |
