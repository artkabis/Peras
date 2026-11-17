# 0032 : Les valeurs partagées, `Rc<T>`

_Version anglaise : [../../en/decisions/0032-shared-values.md](../../en/decisions/0032-shared-values.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond au constat F310, et au maillage partagé que la [décision 0029](0029-pond-of-lanterns.md) a trouvé refusé. Il vient après les décisions [0030](0030-numeric-literals.md), sur les littéraux numériques, et 0031, sur les threads, dans l’ordre fixé par le propriétaire._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Une structure, ce sont ses champs, aplatis.** Dans un module owned, une structure n’a pas de bloc à elle : l’abaissement donne à chacun de ses scalaires une variable locale, une structure dans une structure étant insérée champ par champ (`leaves_in`, `crates/perasc-ir/src/lower/values.rs`). Un type qui se contient lui-même, comme `struct Tree { children: Array<Tree> }` ou `enum List { Nil, Cons(i32, List) }`, n’y a pas de fin : il est vérifié, pas compilé (`E0900`, « types qui se contiennent eux-mêmes », constat F310).
- **Chaque bloc du tas a un seul propriétaire.** La passe de possession (`crates/perasc-check/src/owner.rs`) déplace une chaîne, un tableau, une table ou un ensemble à l’affectation, au retour, en `sink` ou rangé dans une structure, et le prête sinon ; un second détenteur demande `copy()`. `Rc<T>` n’est pas un type connu : la spécification le promet (« le partage seulement par un `Rc<T>` explicite, visible dans le type ») et le laisse à une décision.
- **Un bloc compté existe déjà.** L’environnement d’une valeur fonction ([décision 0026](0026-closures.md)) porte un compte de ses détenteurs : une copie en compte un de plus, le dernier à le lâcher le libère, un environnement statique n’est jamais compté. C’est le premier et le seul compteur de références de l’exécution, sur un seul thread, et ses environnements ne pointent jamais les uns vers les autres.
- **Un tour de `parallel for` ne peut ni appeler, ni allouer, ni réduire** (`E0241`, constat F301) ; la décision 0031 pourra l’élargir avant celle-ci.
- **Le maillage partagé du bassin est refusé** : une lanterne qui tient le maillage que mille lanternes partagent, `lanterns.push(Lantern(mesh, …))` dans une boucle, rencontre `E0204` ; `mesh.copy()` en garderait mille copies (`bench/lanterns/shared-mesh.peras`).

Ce qu’enseignent les langages sans ramasse-miettes, et ce que le propriétaire a soulevé le 2026-11-17 : un pointeur compté qu’on peut modifier permet à un programme de construire un cycle, qu’aucun compte ne libère ; chaque copie et chaque libération touchent le compte ; et un compte modifié par deux threads à la fois demande des opérations atomiques. Les tableaux de Swift partagent leur tampon et le copient à la première modification tant qu’il est partagé (copie à l’écriture). Perceus (Reinking, Xie, de Moura et Leijen, PLDI 2021) et _Counting Immutable Beans_ (Ullrich et de Moura, IFL 2019) montrent un compilateur qui supprime la plupart des opérations de compte, en prêtant ce qui n’est que lu et en réutilisant ce qui est libéré.

## Options

- **r0. Pas de `Rc`** : le partage par un indice dans un tableau, le maillage gardé une fois et chaque lanterne tenant son numéro, comme le fait `bench/lanterns/pond.peras`. La référence.
- **r1. `Rc<T>` en lecture seule, copié à l’écriture.** Une valeur partagée ne peut pas être modifiée sur place : pour la modifier, le programme demande la sienne (`rc.make()` ou un champ écrit), faite sur place quand le compte vaut 1, après une copie sinon. Une valeur ne peut alors pointer que vers des valeurs qui existaient avant elle : aucun cycle ne peut se construire, et il n’y a pas de `Weak`. Les comptes ne changent que là où un `Rc` est rangé pendant que sa source est gardée, et là où son dernier détenteur finit ; prêter et déplacer n’en touchent aucun. Cloner ou libérer un `Rc` dans un tour de `parallel for` est refusé, si bien qu’un compte simple suffit, jamais un compte atomique.
- **r2. `Rc<T>` modifiable, avec `Weak<T>`**, comme `Rc<RefCell<T>>` et `Weak` de Rust : les cycles peuvent se construire, et se cassent à la main par une référence faible, qui ne garde pas sa valeur en vie.
- **r3. `Rc<T>` modifiable, avec un détecteur de cycles** à la libération, par suppression d’essai (Bacon et Rajan, ECOOP 2001) : un compte qui baisse sans tomber à zéro marque sa valeur comme racine possible, et les racines sont parcourues de temps en temps. Construite seulement si elle peut l’être dans cette décision ; sinon dite non construite, avec pourquoi (constat M001).

## Protocole, enregistré avant tout prototype

Quatre quantités, comme pour les décisions 0027, 0028 et 0030, et trois questions qui lui sont propres. Chaque option est prototypée derrière `PERAS_PROTO_RC` ; le build sans elle garde le comportement d’aujourd’hui, r0.

1. **Facilité d’écriture** : quatre programmes, écrits comme un utilisateur les écrit à partir de JavaScript, puis corrigés jusqu’à ce qu’ils se construisent, pour chaque option :
   - P1, le maillage partagé du bassin, `bench/lanterns/shared-mesh.peras` tel qu’écrit ;
   - P2, un arbre binaire construit à partir d’une liste de nombres et sommé, les types qui se contiennent eux-mêmes de F310 ;
   - P3, un graphe de scène dont les nœuds pointent vers leur parent, le cas d’un cycle ;
   - P4, une liste tenue par deux détenteurs, puis modifiée par l’un : l’autre ne doit pas voir la modification.

   Pour chacun : les erreurs rencontrées avant qu’il se construise, avec leurs codes, les lignes changées par rapport au JavaScript, les notions que JavaScript ne demande pas.

2. **Charges**, modules owned sur TLSF dans Node, le module de chaque option et r0 dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports à r0 tour par tour et leur étendue (constat M017) :
   - **W1**, le bassin où chaque lanterne tient son maillage, contre la version par indices : image 600 en 1024 × 768 avec 1 000 lanternes ;
   - **W2**, un arbre de 1 000 000 de nœuds construit, sommé, puis libéré.
3. **Charge du processeur** : le temps CPU sur le temps réel des tours de chaque module, comme le mesure la décision 0012.
4. **Mémoire et octets** : la mémoire du module après chaque charge ; ses octets, bruts et en gzip.
5. **Fuites possibles ou non** : P3 lancé sur l’allocateur de débogage ; les blocs laissés vivants une fois son dernier détenteur fini. Une option qui laisse P3 construire un cycle que le programme ne casse pas fuit ; combien de blocs est rapporté.
6. **Opérations de compte par image du bassin** : les comptes incrémentés et décrémentés pendant que W1 dessine une image, comptés par l’allocateur de débogage. La cible est 0 dans la boucle qui dessine : le maillage d’une lanterne est pris une fois, quand la lanterne est faite.
7. **Utilisable dans `parallel for` ou non** : si un tour peut lire une valeur partagée, et ce que dit le vérificateur d’un tour qui en clone ou en libère une.
8. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-shared.mjs` les lance ; `--check` vérifie que chaque programme se construit et ne laisse rien de vivant là où son option le dit, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
