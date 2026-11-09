# 0002 : L'allocateur de production est TLSF

_Version anglaise, qui fait foi : [../../en/decisions/0002-tlsf-allocator.md](../../en/decisions/0002-tlsf-allocator.md)._

## Contexte

La [décision 0001](0001-webassembly-backend.md) a donné à Peras son propre allocateur, lié seulement aux modules qui utilisent le tas. Jusqu'ici, seul l'allocateur de débogage existait. Il est lent à dessein :

- il empoisonne la mémoire libérée, ce qui coûte du temps à chaque libération ;
- il garde les blocs libérés en quarantaine, si bien qu'ils ne resservent que tard ;
- il vérifie une zone rouge à chaque libération ;
- il parcourt bloc par bloc la liste libre d'une classe de taille.

Un module en production demande une allocation en temps borné, une mémoire réutilisée aussitôt et une fragmentation contenue.

## Décision

1. **Les modules de production utilisent TLSF**, _Two-Level Segregated Fit_ (Masmano, Ripoll, Crespo et Real, 2004). Il est écrit en texte WebAssembly (`crates/perasc-runtime/runtime/alloc_tlsf.wat`), comme l'allocateur de débogage, et assemblé à la compilation de `perasc`.
2. **Le développement garde l'allocateur de débogage.**
   - En ligne de commande, `perasc build` utilise `--allocator debug` par défaut ; les builds de production passent `--allocator tlsf`.
   - Dans le plugin Vite, le serveur de développement lie l'allocateur de débogage et `vite build` lie TLSF. L'option `allocator` du plugin remplace ce choix.
3. **Les deux allocateurs ont les mêmes exports** et partagent les fonctions de tampons, écrites une seule fois (`runtime/buffers.wat`). Le code généré ne dépend pas de celui qui est lié.

## Fonctionnement

- **Classes de taille.** Les blocs libres sont rangés dans 384 listes selon leur taille : 24 premiers niveaux, un par puissance de 2, chacun divisé en 16 classes, avec une classe tous les 16 octets sous 256.
- **Recherche d'un bloc.** Deux bitmaps indiquent quelles listes ne sont pas vides. Un bloc qui convient se trouve avec un `i32.clz` et deux `i32.ctz`, sans parcourir de liste.
- **Découpe.** Un bloc plus grand que nécessaire est découpé, et le reste reste libre.
- **Fusion.** Un bloc libéré fusionne aussitôt avec ses voisins libres : deux blocs libres ne sont jamais côte à côte. Un bloc libre qui termine le tas lui rend sa mémoire.
- **Temps borné.** Chaque opération prend un temps borné, quels que soient la taille demandée et l'état du tas.
- **Vérifications.** Il ne détecte que ce qui ne coûte rien : la libération de quelque chose qui n'est pas l'un de ses blocs, ou d'un bloc déjà libéré. Il ne garde pas de site d'allocation par bloc : son rapport de fuites donne le nombre de blocs vivants, pas leur origine. L'allocateur de débogage couvre le reste, pendant le développement.

## Mesures

Le programme `churn` construit 100 000 fois un tableau de 20 chaînes, soit environ 6 millions d'allocations et autant de libérations, sous Node 26 :

| Allocateur | Temps   | Blocs vivants à la fin |
| ---------- | ------- | ---------------------- |
| débogage   | 1190 ms | 0                      |
| TLSF       | 444 ms  | 0                      |

C'est 2,7 fois plus rapide. Les fonctions qui passent leur temps à la frontière avec JavaScript gagnent moins : là, l'encodage et le décodage des chaînes dominent.

Les tests de `crates/perasc-runtime/tests/tlsf_alloc.rs` vérifient :

- la réutilisation : 10 000 allocations et libérations laissent la fin du tas à sa place ;
- la fusion dans les deux sens, et la découpe ;
- les libérations invalides ou en double ;
- un manque de mémoire géré proprement ;
- 6 000 opérations aléatoires comparées à un modèle. Tous les 50 pas, toute la disposition du tas est vérifiée : blocs contigus, liens arrière, jamais deux voisins libres. Le tas ne dépasse jamais deux fois son pic d'octets vivants, plus 128 Kio. Une fois tout libéré, il est de nouveau vide.

## Conséquences

- La fragmentation est bornée, pas nulle. Un bloc perd au plus une fraction de sa classe de taille : moins de 1/16 au-dessus de 256 octets, moins de 16 octets en dessous. Récupérer la mémoire laissée entre les blocs vivants demandera un compactage : les éléments peuvent se déplacer derrière l'en-tête de leur tableau, sans que l'adresse de cet en-tête change.
- Les tests de bout en bout exécutent des modules avec les deux allocateurs.
