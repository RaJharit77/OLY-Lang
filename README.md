# OLY

Langage de programmation **compilé**, syntaxe 100% malgache.
Pipeline : lexer -> parser (AST) -> vérificateur de types -> compilateur bytecode -> VM.

Fonctionnalités : variables typées (typage graduel), fonctions, vraies fermetures/lambdas,
structs et méthodes, tableaux, dictionnaires, tuples, for-each, ternaire, opérateurs
composés (+=, ++...), court-circuit sy/na, gestion d'erreurs (andramo/sambotra/atsipazo),
modules (ampiasao), bibliothèque standard (maths, chaînes, entrée clavier),
erreurs avec contexte source, REPL.

Voir **DOCUMENTATION.md** pour l'installation (Windows/macOS/Linux), la référence complète
de la syntaxe et un tutoriel pas à pas.

Démarrage rapide :

    cargo build --release
    ./target/release/oly examples/hello.oly
    ./target/release/oly            # REPL interactif
