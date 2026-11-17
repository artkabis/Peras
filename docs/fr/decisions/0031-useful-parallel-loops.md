# 0031 : Des boucles parallèles qui peuvent dessiner une image

_Version anglaise : [../../en/decisions/0031-useful-parallel-loops.md](../../en/decisions/0031-useful-parallel-loops.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond au constat F301, que la [décision 0029](0029-pond-of-lanterns.md) a rencontré de nouveau : le bassin ne peut pas être dessiné par un `parallel for`, et sur cette image seuls les threads peuvent faire passer Peras devant JavaScript (son profil, constat F222)._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Un `parallel for` est une tâche** ([décision 0007](0007-threads-over-shared-memory.md), `crates/perasc-ir/src/lower/parallel.rs`) : son corps devient une fonction, `f__par0(start, end, captures…)`, que les workers du chargeur exécutent sur des plages de tours, sur une seule mémoire partagée. Chaque worker instancie le module sans runtime : une tâche n’alloue jamais.
- **Ce qu’un tour ne peut pas faire** (`check`, `E0241`) : affecter une variable déclarée hors de la boucle ; écrire un élément à un autre indice que celui de son compteur (`Index`) ; lire, à un autre indice, un tableau que la boucle écrit (`Reads`) ; appeler une fonction (`Calls`) ; créer, déplacer ou libérer un tampon (`Heap`) ; lire ou écrire une variable de module, chaque thread ayant les siennes (`Global`) ; quitter la boucle.
- **Ce dont le bassin a besoin** : son ciel appelle `hash`, `unit` et `tri`, qui n’allouent rien et ne touchent aucune variable de module ; une lanterne remplit des pixels sur tout l’écran, si bien qu’un tour découpé par lanternes écrirait n’importe où, mais un tour découpé par bandes de lignes n’écrit que sa bande, et relit la profondeur qu’il y a écrite. Découpé par lignes, chaque pixel voit les mêmes opérations dans le même ordre que sur un thread : l’image reste la même, au bit près.
- **JavaScript a des threads lui aussi** : des Web Workers sur un `SharedArrayBuffer`. Un Peras sur threads est comparé à un TypeScript sur workers, pas seulement au TypeScript sur un thread.

## Options

- **p0. Comme aujourd’hui** : l’image ne peut pas s’écrire en `parallel for` ; le bassin tourne sur un thread.
- **p1. Des appels purs dans un tour** : un tour peut appeler une fonction qui, avec chaque fonction qu’elle appelle, ne crée, ne déplace ni ne libère aucun tampon, ne lit ni n’écrit aucune variable de module, n’écrit par aucun paramètre, et n’appelle aucune valeur fonction. Le vérificateur le prouve à partir du graphe d’appels ; un appel qui n’est pas pur reste refusé, sa raison nommée.
- **p2. p1, et des parts** : dans un tour compté par `b`, `xs.part(b, k)` est les éléments `b * k` à `(b + 1) * k` de `xs`, que le tour peut lire et écrire ; les parts de deux tours ne se chevauchent jamais, par construction. Une part est une vue : rien n’est copié, et elle ne peut pas quitter le tour. Le tour du bassin prend la part de sa bande dans `light`, `depth` et `pixels`.
- **p3. p1, et des plages prouvées** : aucune notion nouvelle ; le vérificateur prouve que chaque indice qu’un tour écrit, ou lit dans un tableau qu’il écrit, tombe dans une plage à lui, `[b * k, (b + 1) * k)`, à partir des bornes de ses boucles. Construite seulement si elle peut l’être dans cette décision ; sinon dite non construite, avec pourquoi (constat M001).

## Protocole, enregistré avant tout prototype

Quatre quantités, comme pour les décisions 0027, 0028 et 0030. Chaque option est prototypée derrière `PERAS_PROTO_PAR` ; le build sans elle garde p0.

1. **Facilité d’écriture** : quatre programmes, écrits comme un utilisateur les écrit à partir du TypeScript sur un thread, puis corrigés jusqu’à ce qu’ils se construisent, pour chaque option :
   - P1, le bassin, image 600 découpée en bandes de lignes ;
   - P2, le jardin de la décision 0013, découpé en lignes ;
   - P3, un flou d’image, chaque ligne de la sortie à partir de trois lignes de l’entrée ;
   - P4, le tableau de bord de la décision 0014 découpé en morceaux de lignes, qui alloue et somme : attendu refusé par chaque option, le refus rapporté.

   Pour chacun : les erreurs rencontrées avant qu’il se construise, avec leurs codes, les lignes changées par rapport au programme sur un thread, les notions que JavaScript ne demande pas.

2. **Les mêmes résultats** : chaque programme construit, sur threads, donne les octets qu’il donne sur un thread et que donne le TypeScript, vérifiés par un hash.
3. **Charges**, dans Chromium, la page isolée de toute autre origine, 15 exécutions après 2, les versions alternant, le rapport tour par tour à Peras sur un thread (constat M017) : W1, le bassin, image 600 en 1024 × 768 avec 1 000 lanternes ; W2, le jardin en 1024 × 1024 ; W3, le flou d’une image de 2048 × 2048. Chacune contre le TypeScript sur un thread et le TypeScript sur autant de workers, découpé de la même façon, sa mémoire un `SharedArrayBuffer`.
4. **Charge du processeur** : le temps CPU de chaque thread sur le temps réel, comme le mesure la décision 0012 : les threads finissent plus tôt et occupent plus de cœurs, et les deux sont rapportés.
5. **Mémoire et octets** : la mémoire de chaque module et des workers ; les octets du module et de son chargeur, bruts et gzip.
6. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-parallel.mjs` les lance ; `--check` vérifie que chaque programme construit donne ses octets sur un thread, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
