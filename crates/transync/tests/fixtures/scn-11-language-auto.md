## Notes de version

Cette section décrit les changements apportés à la chaîne de traitement depuis la dernière itération du document.

L'option `source-language=auto` indique au pipeline que la langue source doit être détectée par le fournisseur. Le résultat de la détection traverse le pipeline sous la forme du champ `detected_source_language` de la carte d'alignement.

Si la détection échoue, l'appelant peut toujours fournir une langue source explicite ; le champ `detected_source_language` sera alors absent.
