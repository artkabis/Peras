# Exception de runtime de Peras

_Traduction à titre d’information ; seule la version anglaise fait foi : [RUNTIME-EXCEPTION.md](RUNTIME-EXCEPTION.md)._

Version 1.0

Ceci est une autorisation supplémentaire au sens de la section 7 de la GNU Affero General Public License, version 3 ou toute version ultérieure (l’« AGPL »), accordée par les titulaires des droits d’auteur de Peras. Peras est sous licence AGPL ([LICENSE](LICENSE)) ; cette exception s’y ajoute et n’en retire rien.

## Définitions

- **Peras** désigne le logiciel de ce dépôt : le compilateur `perasc`, ses crates, ses plugins de build, son runtime et sa documentation.
- **Code de runtime** désigne la partie de Peras que le compilateur ou un plugin de build de Peras écrit dans le résultat d’une compilation : le runtime et les allocateurs de `crates/perasc-runtime`, le chargeur JavaScript et les déclarations TypeScript que génère `perasc`, et le code que `vite-plugin-peras` insère dans un bundle.
- **Programme compilé** désigne ce que produit la compilation d’un code source avec Peras, Code de runtime compris, combiné ou non avec d’autre code.

## Autorisation

Vous pouvez copier, modifier, distribuer et utiliser un Programme compilé, Code de runtime compris, aux conditions de votre choix, conditions propriétaires comprises. L’AGPL ne s’applique ni à un Programme compilé, ni au code avec lequel il est combiné, du fait du Code de runtime qu’il contient.

Le code source que vous écrivez et compilez avec Peras vous appartient : l’AGPL ne s’y est jamais appliquée.

## Limites

Cette autorisation ne couvre pas :

- Peras lui-même, ou un Peras modifié : le compilateur, ses crates et ses plugins de build, utilisés ou distribués seuls ou au sein d’un autre produit, restent sous AGPL ;
- le Code de runtime extrait d’un Programme compilé et utilisé ou distribué seul, ou au sein d’un produit qui n’est pas un Programme compilé : l’AGPL s’y applique ;
- un Programme compilé produit par un Peras modifié dont le code source n’est pas disponible sous AGPL avec cette exception.

## Exemples

Les exemples de code de la documentation (`docs/`) et de `examples/` peuvent aussi être utilisés sans aucune condition, sous la licence BSD Zero Clause (0BSD) : les copier dans un programme ne lui impose rien.
