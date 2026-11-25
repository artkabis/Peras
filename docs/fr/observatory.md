# L’observatoire de Peras

_Original anglais : [../en/observatory.md](../en/observatory.md)._

Une page qui montre ce que Peras fait bien et ce qu’il ne fait pas encore, en un seul lieu : des scènes dessinées en direct par TypeScript et par Peras côte à côte, une mesure que le visiteur lance sur sa propre machine, et chaque chiffre que le dépôt a consigné, lu dans ses fichiers de résultats quand la page est construite ([source](../../examples/observatory)). Elle suit le jardin d’Apeiron ([décision 0013](decisions/0013-garden-of-apeiron.md)) : une mesure qui compte est aussi une scène, le même parcours pour chaque version, les chiffres en dessous ; la poésie montre ce que mesurent les sondes et ne décide jamais.

Ce protocole a été écrit avant la page, et avant toute mesure qu’elle fait.

## Ce que montre la page

1. **Une scène dessinée en direct, deux fois.** L’image pleine de la décision 0047, un plasma de 640 sur 400 : à gauche, `bench/frame/plasma.ts`, le TypeScript qui l’a spécifiée ; à droite, `bench/frame/canvas-f3.peras`, le module Peras que la décision 0047 a choisi, qui dessine dans un tableau gardé dans la mémoire du module et montré par une vue, sans pixel copié. La page importe ces fichiers mêmes : ce qu’elle dessine est ce que mesurent les bancs. À chaque image, les deux dessinent au même temps `t`, celui qui commence alternant d’une image à l’autre ; toutes les 30 images, les octets des deux images sont comparés, et la page dit s’ils sont les mêmes.
2. **Une bougie sous chacune**, dont la cire brûle comme le temps que sa version a passé à dessiner, additionné depuis le début de la scène, face au plus grand des deux : une image du temps du processeur, pas une mesure en soi.
3. **La mesure du visiteur.** Sur un bouton, la scène s’arrête et la page mesure, sans canvas : trois versions, le TypeScript, le module Peras, et le TypeScript de nouveau en seconde instance (le contrôle A/A, constat M042) ; 21 tours de 20 images, l’ordre alterné à chaque tour ; pour chaque tour, le rapport du temps de chaque version à celui du TypeScript. La page donne la médiane et l’étendue de chaque rapport, le navigateur, et le nombre de processeurs logiques qu’il annonce, et laisse le visiteur enregistrer les tours en JSON. Une médiane du contrôle A/A hors de 0,95 à 1,05 est montrée comme telle : la machine était trop bruitée pour que l’autre rapport décide de quoi que ce soit.
4. **Les chiffres consignés**, lus dans les résultats du dépôt quand la page est construite, chacun avec son fichier : les quatre applications de la décision 0046 face à TypeScript (`bench/next/RESULTS.md`), les programmes de la décision 0047 face à TypeScript (`bench/frame/RESULTS.md`), et les dix-sept bogues d’applications web de la décision 0011 (`bench/web-bugs/RESULTS.md`). Ils sont tracés sur un seul axe, le temps de Peras sur celui de TypeScript, à une échelle logarithmique avec 1 au milieu : à sa gauche là où Peras est plus rapide, à sa droite là où il est plus lent, avec la bande du contrôle A/A là où le fichier la donne. Rien n’est tapé à la main dans la page : quand une décision change un résultat, la page montre le nouveau.

## Ce qu’elle ne peut pas montrer, dit maintenant

- La machine du visiteur n’est pas celle du dépôt : son rapport est le sien, et la page dit où le chiffre consigné a été mesuré. Un navigateur qui n’est pas Chromium peut compiler WebAssembly et JavaScript autrement.
- Le second TypeScript du contrôle A/A est une seconde instance de la même classe, pas un second module : il mesure le bruit de la machine et de l’ordre, pas celui du chargement d’un module.
- Les temps d’image de la scène en direct comprennent le dessin de deux canvas par la page ; seule la mesure du visiteur, sans canvas, est une mesure.
- Une scène n’est pas chaque programme : les chiffres consignés sont là pour cela, les applications où Peras est plus lent parmi eux.
