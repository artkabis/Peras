# Lire les diagnostics

Peras n’a pas d’avertissements : chaque diagnostic est une erreur, et rien n’est compilé tant que toutes ne sont pas corrigées. En échange, chacune dit ce qui ne va pas, pourquoi la règle existe, et comment corriger.

## Anatomie d’une erreur

```text
erreur[E0006] : `===` n’existe pas en Peras
  --> demo.peras:3:17
   |
 3 |     const zero = b === 0.0;
   |                    ^^^ écris `==`
   |
   = pourquoi : Peras ne convertit jamais les types implicitement : `==` et `!=` comparent donc déjà strictement, comme `===` et `!==` en JavaScript.
   = correction : remplace `===` par `==`
   |
 3 -     const zero = b === 0.0;
 3 +     const zero = b == 0.0;
   |
   = détails : perasc explain E0006
```

| Partie          | Dit                                                                         |
| --------------- | --------------------------------------------------------------------------- |
| `erreur[E0006]` | un code stable : il ne change jamais de sens et n’est jamais réutilisé      |
| `-->`           | le fichier, la ligne et la colonne, cliquables dans la plupart des éditeurs |
| l’extrait       | la source, avec ce qui ne va pas souligné                                   |
| `pourquoi`      | pourquoi la règle existe : ce qui irait mal sans elle                       |
| `correction`    | ce qu’il faut faire, souvent sous forme de diff à appliquer                 |
| `détails`       | la commande qui affiche la page complète de cette erreur                    |

## Toutes les erreurs d’un coup

Le compilateur signale toutes les erreurs d’un fichier en une passe, au plus 50 par fichier, et jamais la conséquence d’une autre erreur : une faute, un message.

## En anglais

`--lang en` affiche chaque message en anglais ; sans cette option, `perasc` suit la langue de votre système.

```sh
perasc check app.peras --lang en
```

## La page complète : `perasc explain`

```sh
perasc explain E0006
```

affiche la page de cette erreur, avec un exemple et sa correction. Les mêmes pages sont dans cette documentation : [codes d’erreur](../errors/index.md).

## Pour les outils : `--json`

`perasc check --json` et `perasc build --json` écrivent les mêmes diagnostics en JSON, avec chaque position et les modifications de la correction. Le plugin Vite s’en sert pour afficher les erreurs dans son overlay ; les éditeurs et la CI peuvent s’en servir aussi.

Ensuite : [Peras et JavaScript](javascript.md).
