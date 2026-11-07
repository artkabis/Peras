# Codes d'erreur

Une page par code, nommée d'après lui (`E0204.md`). Chaque page explique la cause, montre un exemple fautif et sa correction. Les codes sont permanents : un code retiré garde sa page, marquée comme retirée.

Les codes `E0001` à `E0099` sont réservés au lexer, `E0100` à `E0199` au parser, `E0200` à `E0299` aux noms et aux types, `E0900` à `E0999` aux limites de la génération de code.

| Code              | Erreur                                              | Étape              |
| ----------------- | --------------------------------------------------- | ------------------ |
| [E0001](E0001.md) | Caractère invalide                                  | lexer              |
| [E0002](E0002.md) | Chaîne non terminée                                 | lexer              |
| [E0003](E0003.md) | Commentaire de bloc non terminé                     | lexer              |
| [E0004](E0004.md) | Séquence d'échappement invalide                     | lexer              |
| [E0005](E0005.md) | Nombre mal formé                                    | lexer              |
| [E0006](E0006.md) | Opérateur d'égalité stricte                         | lexer              |
| [E0007](E0007.md) | Caractère de contrôle bidirectionnel                | lexer              |
| [E0008](E0008.md) | Fichier source trop volumineux                      | lexer              |
| [E0100](E0100.md) | Expression attendue                                 | parser             |
| [E0101](E0101.md) | Point-virgule manquant                              | parser             |
| [E0102](E0102.md) | Nom attendu                                         | parser             |
| [E0103](E0103.md) | Délimiteur non fermé                                | parser             |
| [E0104](E0104.md) | Symbole inattendu                                   | parser             |
| [E0105](E0105.md) | Type attendu                                        | parser             |
| [E0106](E0106.md) | Mot-clé supprimé                                    | parser             |
| [E0107](E0107.md) | Nombre trop grand                                   | parser             |
| [E0108](E0108.md) | Comparaisons enchaînées                             | parser             |
| [E0109](E0109.md) | Opérateur bit à bit mélangé à une comparaison       | parser             |
| [E0110](E0110.md) | Déclaration attendue                                | parser             |
| [E0111](E0111.md) | Imbrication trop profonde                           | parser             |
| [E0112](E0112.md) | Const sans valeur                                   | parser             |
| [E0113](E0113.md) | Affectation utilisée comme expression               | parser             |
| [E0114](E0114.md) | Cible d’affectation invalide                        | parser             |
| [E0115](E0115.md) | Condition sans parenthèses                          | parser             |
| [E0116](E0116.md) | Boucle `for…in`                                     | parser             |
| [E0117](E0117.md) | `break` ou `continue` hors d’une boucle             | parser             |
| [E0118](E0118.md) | Marqueur optionnel `?:`                             | parser             |
| [E0119](E0119.md) | Corps vide après `;`                                | parser             |
| [E0200](E0200.md) | Nom non défini                                      | noms et types      |
| [E0201](E0201.md) | Masquage                                            | noms et types      |
| [E0202](E0202.md) | Déclaration en double                               | noms et types      |
| [E0203](E0203.md) | Variable ou paramètre inutilisé                     | noms et types      |
| [E0204](E0204.md) | Valeur utilisée après un déplacement                | possession         |
| [E0205](E0205.md) | Modification impossible                             | noms et types      |
| [E0206](E0206.md) | `&` et les arguments `inout`                        | possession         |
| [E0207](E0207.md) | Alias avec un argument `inout`                      | possession         |
| [E0209](E0209.md) | Déplacer ce qui n’est qu’emprunté                   | possession         |
| [E0210](E0210.md) | Types incompatibles                                 | noms et types      |
| [E0211](E0211.md) | Entier littéral à la place d’un flottant            | noms et types      |
| [E0212](E0212.md) | Littéral hors bornes                                | noms et types      |
| [E0213](E0213.md) | Type inconnu                                        | noms et types      |
| [E0214](E0214.md) | Mauvais nombre d’arguments                          | noms et types      |
| [E0215](E0215.md) | Champ ou méthode inconnu                            | noms et types      |
| [E0216](E0216.md) | Mauvais nombre d’arguments de type                  | noms et types      |
| [E0217](E0217.md) | La condition n’est pas un bool                      | noms et types      |
| [E0218](E0218.md) | Valeur de retour incorrecte                         | noms et types      |
| [E0219](E0219.md) | Retour manquant                                     | noms et types      |
| [E0220](E0220.md) | Type de retour manquant                             | noms et types      |
| [E0221](E0221.md) | Opérateur invalide                                  | noms et types      |
| [E0222](E0222.md) | `match` incomplet                                   | noms et types      |
| [E0223](E0223.md) | Variante inconnue                                   | noms et types      |
| [E0224](E0224.md) | Bras inatteignable                                  | noms et types      |
| [E0225](E0225.md) | `?` dans une fonction qui ne peut pas renvoyer tôt  | noms et types      |
| [E0900](E0900.md) | Pas encore pris en charge par la génération de code | génération de code |
