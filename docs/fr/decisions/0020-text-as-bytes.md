# 0020 : Le texte donné en octets

_English version: [../../en/decisions/0020-text-as-bytes.md](../../en/decisions/0020-text-as-bytes.md)._

_Statut : proposée le 2026-11-12, avec ses critères et un amendement à la version C de la décision 0014 enregistrés avant tout code ; construite, mesurée et gardée le même jour (voir [Mesuré : gardé](#mesuré--gardé))._

## Contexte

Le but de Peras inclut un bon usage de la mémoire : des blocs libérés quand ils cessent de servir, prouvés par le compilateur, sans ramasse-miettes, sans pause et sans fuite. La décision 0014 mesure combien de mémoire prend le tableau de bord commercial. À l’exécution 4, le pic de Peras valait 0,78 fois celui de TypeScript simple à 4 000 000 de lignes (H3, qui demande 0,5, réfutée).

**De quoi est fait le pic.** Une sonde dans Node, à 4 000 000 de lignes (`target/mem-probe`, la même construction que la version C de la décision 0014), a mesuré la mémoire résidente après un passage du ramasse-miettes à chaque étape :

| Étape                                                        | Mémoire ajoutée |
| ------------------------------------------------------------ | --------------- |
| Les octets du fichier, un `Uint8Array`, avant la requête     | 121,6 Mio       |
| `new TextDecoder().decode(bytes)` : la chaîne donnée à Peras | +118,2 Mio      |
| `summarize(text)` : la mémoire du module                     | +183,3 Mio      |

La mémoire du module tient le texte qui y est copié, 121,6 Mio, et les tables, environ 62 Mio. La chaîne JavaScript n’existe que parce qu’une fonction exportée de Peras qui prend un `String` accepte une chaîne JavaScript et rien d’autre. Les données arrivent en octets. JavaScript les décode en chaîne, puis le chargeur de Peras réencode cette chaîne en octets UTF-8 dans la mémoire du module. Les deux versions TypeScript lisent les octets et ne construisent pas une telle chaîne.

**Ce qui ne peut pas partir.** Un module WebAssembly ne lit que sa propre mémoire. Sous le protocole de la décision 0014, les données partent d’un `Uint8Array` en dehors de lui, donc une copie dans le module demeure. Une copie d’octets est un `memcpy`, bien moins cher que décoder puis réencoder du texte.

## Le changement

Là où une fonction exportée prend un `String`, le chargeur accepte aussi un `Uint8Array` :

1. Il copie les octets tels quels dans une nouvelle chaîne du module.
2. Le runtime vérifie qu’ils sont de l’UTF-8, comme le fait `new TextDecoder("utf-8", { fatal: true })`.
3. Des octets qui ne sont pas de l’UTF-8 lèvent la même `TypeError` qu’un tel décodeur, et rien ne reste alloué.

La vérification est la règle du standard Unicode, chapitre 3, table 3-7 :

- aucun octet `C0`, `C1` ni `F5` à `FF` ;
- aucune forme trop longue ;
- aucune demi-paire de substitution, `ED A0` à `ED BF` ;
- rien au-delà de U+10FFFF ;
- aucune séquence coupée.

Un texte ASCII est vérifié 16 octets à la fois en SIMD (l’approche de Keiser et Lemire, _Validating UTF-8 in less than one instruction per byte_, Software: Practice and Experience 51(5), 2021, réduite à son chemin rapide ASCII).

**Pour un programme.** Rien ne change dans Peras : la syntaxe, les types et chaque résultat restent les mêmes. La déclaration TypeScript générée d’un tel paramètre devient `string | Uint8Array`, un ajout à l’API que voit une page, qui ne casse aucun appel existant.

**Pour la sûreté.** Les chaînes de Peras sont de l’UTF-8 par construction ; la vérification les garde ainsi. Une chaîne qui ne serait pas de l’UTF-8 fausserait le parcours de ses caractères et ses changements de casse, donc la vérification n’est jamais sautée, ni en debug ni en production.

## Amendement à la décision 0014, enregistré avant toute mesure

Le code d’appel de la version C, la ligne de `scripts/bench-data.mjs` qui l’appelle, devient `peras.summarize(bytes)` au lieu de `peras.summarize(new TextDecoder().decode(bytes))`. Le protocole mesure à partir des octets du fichier en mémoire, « y compris tout décodage et toute copie dans un module ». Les versions A et B prennent déjà les octets. Le source Peras de C, A, B, les données et les seuils ne changent pas. La prochaine exécution de la 0014 est l’exécution 5, et elle liste ceci comme son écart à l’exécution 4.

## Critères, enregistrés avant tout code

Gardé seulement si les trois tiennent, sinon retiré :

1. **Mémoire.** À l’exécution 5 de la décision 0014, H3, le pic de mémoire de C sur celui de A à 4 000 000 de lignes, vaut au plus 0,65. Il valait 0,78 à l’exécution 4.
2. **Temps.** À l’exécution 5, C sur A vaut au moins 2,58 à 1 000 000 de lignes et 2,34 à 4 000 000. Ce sont les 2,63 et 2,39 de l’exécution 4 moins 0,05 : pas de perte.
3. **Justesse.** Un test de bout en bout sur les deux allocateurs donne 10 000 chaînes d’octets aléatoires et les cas limites de la table 3-7. Un `Uint8Array` est accepté exactement quand `TextDecoder` avec `fatal: true` l’accepte, et donne alors la même chaîne. Rien ne reste vivant.

## Mesuré : gardé

Le changement est arrivé au commit `39728be`, et l’exécution 5 de la décision 0014 l’a mesuré au commit `8f98a87`, la version C recevant les octets comme le dit l’amendement.

| Critère                                                             | Mesuré                                                                                                                                                                                        | Verdict |
| ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| 1. H3 au plus 0,65                                                  | 0,23 : le pic de C 49,4 Mio à 4 000 000 de lignes, contre 171,6 à l’exécution 4                                                                                                               | tient   |
| 2. C sur A au moins 2,58 et 2,34                                    | 3,00 à 1 000 000 de lignes, 2,52 à 4 000 000                                                                                                                                                  | tient   |
| 3. Des octets acceptés exactement comme par un `TextDecoder` strict | 10 000 chaînes aléatoires et les limites de la table 3-7, sur les deux allocateurs, dans les tests de bout en bout ; 50 000 de plus avec et sans SIMD : aucun désaccord, rien ne reste vivant | tient   |

**Gardé.** Une mutation qui accepte les demi-paires de substitution fait échouer le test de bout en bout sur les deux allocateurs. Les déclarations des paramètres `String` disent `string | Uint8Array` ; deux tests qui les lisent ont été mis à jour.

**Ce qu’il ne fait pas.** Le module copie toujours le texte, environ 122 Mio à 4 000 000 de lignes, parce qu’un module WebAssembly ne lit que sa propre mémoire. Les 49,4 Mio de l’exécution 5 sont le pic que le protocole de la décision 0014 voit au-delà du chargement des données (M008), pas tout ce que C tient. Retirer cette copie demanderait des données écrites dès le départ dans la mémoire du module, comme une page qui lit un fichier ou une réponse directement dedans. C’est un changement de la façon dont une page donne ses données à Peras, pour une autre décision.

## Corrigé après avoir été gardé : la marque d’ordre des octets

Le critère de justesse a été tenu par un test qui comparait ce que le module rendait. Ce test a manqué deux différences (constat F217) :

- Des octets commençant par une marque d’ordre des octets, `EF BB BF`, la gardaient comme un caractère, là où un `TextDecoder` la retire. Un fichier CSV enregistré avec elle aurait eu son premier en-tête faux.
- Une chaîne rendue qui commençait par U+FEFF le perdait, parce que le décodeur du chargeur prenait ce caractère pour une marque.

Les deux sont corrigées. Le chargeur retire une marque au début des octets, et décode ce qu’il rend avec `ignoreBOM`. Le test de bout en bout compare aussi maintenant ce que le module calcule à partir des octets et à partir du même texte décodé : sa longueur, ses caractères, et s’il commence par `a`. Chaque correction, retirée exprès, le fait échouer.
