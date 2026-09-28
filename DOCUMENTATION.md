# Documentation OLY

**OLY** est un langage de programmation **compilé** (pas interprété) dont la syntaxe est **100 % en malgache**. Il a été conçu dans un but pédagogique : aider à mieux comprendre les mécanismes de la programmation grâce à des mots-clés dans une langue maternelle plutôt qu'en anglais.

Pipeline de compilation : `code source .oly` → **lexer** → **parser (AST)** → **vérificateur de types** → **compilateur bytecode** → **VM (machine virtuelle à pile)**.

---

## Table des matières

1. [Installation](#1-installation)
2. [Configuration et utilisation](#2-configuration-et-utilisation)
3. [Référence de syntaxe](#3-référence-de-syntaxe)
4. [Tutoriel pas à pas](#4-tutoriel-pas-à-pas)
5. [Annexes](#5-annexes)

---

## 1. Installation

OLY est écrit en Rust et se compile avec **Cargo**, l'outil de build standard de Rust. Le projet n'a **aucune dépendance externe** (uniquement la bibliothèque standard), donc n'importe quelle version stable récente de Rust convient.

### 1.1. Prérequis communs

Il faut installer la chaîne d'outils Rust (`rustc` + `cargo`). La méthode recommandée sur les trois systèmes est **rustup**, l'installeur officiel.

### 1.2. Windows

1. Téléchargez et exécutez `rustup-init.exe` depuis **https://rustup.rs** (ou via `winget install Rustlang.Rustup` dans un terminal PowerShell).
2. L'installeur peut demander les **Build Tools for Visual Studio** (composant "Desktop development with C++") pour le linker MSVC. Acceptez l'installation proposée, ou choisissez la chaîne GNU (`x86_64-pc-windows-gnu`) si vous préférez éviter Visual Studio.
3. Redémarrez le terminal, puis vérifiez :
   ```powershell
   rustc --version
   cargo --version
   ```
4. Placez les fichiers du projet OLY dans un dossier, par exemple `C:\Users\vous\oly\`.

### 1.3. macOS

1. Installez les outils en ligne de commande Xcode (fournissent le linker) :
   ```bash
   xcode-select --install
   ```
2. Installez Rust via rustup :
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
   (ou via Homebrew : `brew install rust`, moins recommandé car moins à jour).
3. Ouvrez un nouveau terminal (ou lancez `source $HOME/.cargo/env`), puis vérifiez :
   ```bash
   rustc --version
   cargo --version
   ```

### 1.4. Linux

**Méthode recommandée (rustup, toutes distributions) :**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version
cargo --version
```

**Alternative via le gestionnaire de paquets** (versions parfois moins récentes) :
```bash
# Debian / Ubuntu
sudo apt update && sudo apt install -y rustc cargo

# Fedora
sudo dnf install rust cargo

# Arch
sudo pacman -S rust
```

### 1.5. Récupérer et compiler OLY

Le projet suit la structure Cargo standard :

```
oly/
├── Cargo.toml
├── src/
│   ├── main.rs         # point d'entrée (fichier .oly ou REPL)
│   ├── token.rs         # définition des tokens
│   ├── lexer.rs         # analyse lexicale
│   ├── ast.rs            # arbre syntaxique
│   ├── parser.rs        # analyse syntaxique
│   ├── errors.rs        # erreurs avec contexte source
│   ├── module.rs        # résolution des imports (ampiasao)
│   ├── typechecker.rs   # vérificateur de types
│   ├── bytecode.rs      # format bytecode
│   ├── compiler.rs      # AST -> bytecode
│   ├── vm.rs            # machine virtuelle
│   └── repl.rs          # REPL interactif
└── examples/
    └── *.oly
```

Placez tous ces fichiers dans un dossier (même arborescence), ouvrez un terminal à la racine (`oly/`), puis :

```bash
# Build de développement (plus rapide à compiler, binaire plus lent)
cargo build

# Build optimisé (recommandé pour un usage régulier)
cargo build --release
```

Le binaire compilé se trouve ensuite dans :
- `target/release/oly` (Linux/macOS)
- `target\release\oly.exe` (Windows)

C'est la même procédure sur Windows, macOS et Linux — Cargo gère les différences de plateforme automatiquement.

---

## 2. Configuration et utilisation

Aucune configuration particulière n'est nécessaire : OLY n'utilise ni fichier de configuration ni variables d'environnement.

### 2.1. Exécuter un fichier `.oly`

```bash
# Avec cargo (depuis le dossier du projet)
cargo run -- chemin/vers/programme.oly

# Avec le binaire compilé
./target/release/oly chemin/vers/programme.oly      # Linux/macOS
target\release\oly.exe chemin\vers\programme.oly     # Windows
```

### 2.2. Lancer le REPL interactif

Sans argument, OLY démarre un REPL (boucle lire-évaluer-afficher) :

```bash
cargo run
# ou
./target/release/oly
```

Dans le REPL :
- Tapez une instruction ou une expression, terminée par Entrée.
- Une expression seule (ex. `2 + 2`) affiche automatiquement son résultat.
- Les blocs multi-lignes (`asa`, `endrika`, `raha`, ...) sont supportés : le REPL attend la fermeture de l'accolade avant d'exécuter.
- Les variables, fonctions et structs définis restent disponibles d'une ligne à l'autre.
- `:aide` ou `:help` affiche l'aide ; `:quit` ou `:q` quitte.

```
oly> aoka x = 10;
oly> x * 2
20
oly> asa carre(n) -> isa {
...>     avereno n * n;
...> }
oly> carre(7)
49
oly> :quit
```

### 2.3. Programmes multi-fichiers (`ampiasao`)

`ampiasao "nom";` charge et fusionne `nom.oly`, cherché **dans le même dossier** que le fichier qui l'importe. Les imports sont résolus récursivement et chaque fichier n'est chargé qu'une seule fois (protection contre les imports circulaires).

```
projet/
├── principal.oly       # ampiasao "matematique";
└── matematique.oly
```

```bash
cargo run -- projet/principal.oly
```

---

## 3. Référence de syntaxe

### 3.1. Commentaires

```oly
// Ceci est un commentaire sur une ligne
```

### 3.2. Types de base

| Mot-clé OLY | Signification | Exemple |
|---|---|---|
| `isa` | entier | `42` |
| `ampahany` | flottant | `3.14` |
| `teny` | chaîne de caractères | `"salama"` |
| `marina` | booléen | `marina` (vrai) / `diso` (faux) |
| `NomStruct` | type struct nommé | `Olona` |
| `T[]` | tableau/liste d'éléments de type T | `isa[]`, `teny[]` |

### 3.3. Variables

```oly
aoka age = 25;              // déclaration + inférence du type
aoka nom: teny = "Rina";    // déclaration avec annotation de type explicite
tsymiova pi: ampahany = 3.14;
```

> **Note** : `tsymiova` (constante) est actuellement traité comme `aoka` à l'exécution — l'immutabilité n'est pas encore appliquée par le compilateur (voir [Limitations](#54-limitations-connues)).

L'affectation `nom = valeur` réassigne une variable existante (globale ou locale selon le contexte).

### 3.4. Opérateurs

| Catégorie | Opérateurs |
|---|---|
| Arithmétiques | `+` `-` `*` `/` `%` |
| Comparaison | `==` `!=` `<` `>` `<=` `>=` |
| Logiques | `sy` (et), `na` (ou), `tsy` (non, unaire) |
| Affectation | `=` |

`+` fonctionne aussi sur les chaînes (`teny`) pour la concaténation ; si l'opérande gauche est une chaîne, l'opérande droit (quel que soit son type) est converti en texte.

### 3.5. Structures de contrôle

**Condition :**
```oly
raha age >= 18 {
    asehoy("majeur");
} na raha age >= 13 {
    asehoy("adolescent");
} raha tsy izany {
    asehoy("enfant");
}
```

**Boucle tant que :**
```oly
aoka i = 0;
raha mbola i < 5 {
    asehoy(i);
    i = i + 1;
}
```

**Boucle pour (style C : init ; condition ; incrément) :**
```oly
isaky (aoka i = 0; i < 5; i = i + 1) {
    asehoy(i);
}
```

**Rupture de boucle :** `tapaka;` (break) et `manohy;` (continue).

### 3.6. Fonctions

```oly
asa additionner(a: isa, b: isa) -> isa {
    avereno a + b;
}

asehoy(additionner(2, 3)); // 5
```

Les annotations de type des paramètres et du retour (`-> type`) sont optionnelles ; sans elles, le vérificateur de types applique un **typage graduel** (il ne signale une erreur que lorsqu'il peut réellement prouver une incompatibilité).

### 3.7. Structs et méthodes

```oly
endrika Olona {
    anarana: teny,
    taona: isa,
}

fomba Olona {
    asa salama(ity) -> teny {
        avereno "Miarahaba, " + ity.anarana;
    }
    asa mihalehibe(ity) {
        ity.taona = ity.taona + 1;
    }
}

aoka rina = Olona { anarana: "Rina", taona: 20 };
asehoy(rina.salama());   // "Miarahaba, Rina"
rina.mihalehibe();
asehoy(rina.taona);      // 21
```

Le premier paramètre d'une méthode (souvent nommé `ity` ou `tena` par convention) représente l'instance sur laquelle la méthode est appelée ; il reçoit automatiquement le récepteur (`rina`) lors de l'appel `rina.salama()`.

### 3.8. Tableaux

```oly
aoka nombres: isa[] = [10, 20, 30];
asehoy(nombres[1]);        // 20
nombres[1] = 99;

asehoy(habeny(nombres));   // longueur -> 3
ampio(nombres, 40);        // ajoute un élément en fin de liste
aoka dernier = esory(nombres); // retire et renvoie le dernier élément
```

| Fonction native | Rôle |
|---|---|
| `habeny(x)` | longueur d'une liste ou d'une chaîne |
| `ampio(liste, valeur)` | ajoute `valeur` en fin de `liste` |
| `esory(liste)` | retire et renvoie le dernier élément |

### 3.9. Modules

```oly
// matematique.oly
asa carre(x: isa) -> isa {
    avereno x * x;
}
```
```oly
// principal.oly
ampiasao "matematique";
asehoy(carre(5)); // 25
```

### 3.10. Affichage

```oly
asehoy(expression);
```

### 3.11. Erreurs et vérification de types

OLY vérifie les types **avant** l'exécution (typage graduel : il n'exige pas d'annotation partout, mais signale toute incohérence qu'il peut prouver). Toute erreur — lexicale, syntaxique, de type ou d'exécution — est affichée avec le fichier, la ligne et un extrait du code source :

```
erreur: Type incompatible pour 'x' : déclaré Isa, valeur de type Teny
  --> exemple.oly:3
   |
 3 | aoka x: isa = "texte";
   |
```

---

### 3.12. Opérateurs pratiques et confort d'écriture

**Affectations composées et incrément/décrément :**
```oly
aoka n = 10;
n += 5;   n -= 3;   n *= 2;   n /= 4;   n %= 4;
n++;      n--;      // uniquement en instruction (pas dans une expression)
```

**Opérateur ternaire** (`condition ? si_vrai : si_faux`, chaînable) :
```oly
asehoy(age >= 18 ? "majeur" : "mineur");
asehoy(age > 60 ? "senior" : age > 30 ? "adulte" : "jeune");
```

**Boucle for-each** : `isaky element amin collection { ... }` — parcourt une liste, un tuple ou une chaîne-liste ; `tapaka`/`manohy` fonctionnent normalement. Pour un dictionnaire, on parcourt ses clés avec `fanalahidy` :
```oly
isaky fruit amin ["mangue", "banane", "litchi"] {
    asehoy(fruit);
}
isaky nom amin fanalahidy(notes) {
    asehoy(nom + " -> " + notes[nom]);
}
```

**Commentaires multi-lignes :** `/* ... */` (en plus de `// ...`).

**Court-circuit :** `sy` (et) et `na` (ou) n'évaluent le côté droit que si nécessaire, comme en C ou Python.
`diso sy f()` n'appelle jamais `f`.

### 3.13. Gestion d'erreurs : `andramo` / `sambotra` / `atsipazo`

```oly
asa diviser(a: isa, b: isa) -> isa {
    raha b == 0 {
        atsipazo "division par zéro interdite";   // lève une erreur (message = teny)
    }
    avereno a / b;
}

andramo {                       // essaie
    asehoy(diviser(10, 2));
    asehoy(diviser(1, 0));
} sambotra (erreur) {           // rattrape ; 'erreur' contient le message (teny)
    asehoy("échec : " + erreur);
}
```
Sont rattrapées : les erreurs levées par `atsipazo` **et** toutes les erreurs d'exécution (index hors limites, division par zéro, clé introuvable, etc.), y compris celles survenant dans des fonctions appelées depuis le bloc `andramo`. Les blocs peuvent être imbriqués, et on peut relancer une erreur depuis `sambotra`.
Une erreur non rattrapée arrête le programme avec un message contextuel (fichier, ligne, extrait).

### 3.14. Fonctions anonymes (lambdas)

```oly
aoka carre = asa(x: isa) -> isa { avereno x * x; };
asehoy(carre(7));                                   // 49

asa appliquer(f, valeur) { avereno f(valeur); }     // fonction en argument
asehoy(appliquer(carre, 9));                        // 81
asehoy(appliquer(asa(n) { avereno n + 100; }, 1));  // 101

aoka ops = [asa(x) { avereno x + 1; }, asa(x) { avereno x * 2; }];
isaky op amin ops { asehoy(op(10)); }               // 11 puis 20
asehoy(asa(z) { avereno z * 3; }(5));               // appel immédiat : 15
```
> **Limite actuelle** : une lambda voit les variables **globales** et ses propres paramètres, mais ne capture pas les variables locales de la fonction qui la contient (pas de fermetures complètes).

### 3.15. Dictionnaires

Littéral `[cle: valeur, ...]` ; clés de type `isa` ou `teny` uniquement.
```oly
aoka notes = ["Rina": 15, "Soa": 12];
asehoy(notes["Soa"]);          // 12
notes["Vola"] = 9;             // ajout / modification
notes["Soa"] += 3;
asehoy(habeny(notes));         // 3
asehoy(misy(notes, "Rina"));   // marina (la clé existe)
aoka cles = fanalahidy(notes); // liste triée des clés
```
Accéder à une clé absente lève une erreur (rattrapable). Un `[]` vide est toujours une **liste** vide.

### 3.16. Tuples

Groupe de valeurs de types éventuellement différents, de taille fixe et **immuable** : `(a, b, ...)` (au moins 2 éléments).
```oly
aoka fiche = ("Rina", 25, marina);
asehoy(fiche[0] + " a " + fiche[1] + " ans");   // Rina a 25 ans
asehoy(habeny(fiche));                          // 3
fiche[0] = "Soa";                               // erreur : un tuple est immuable
```

### 3.17. Bibliothèque standard

| Domaine | Fonction | Rôle |
|---|---|---|
| Collections | `habeny(x)` | taille (liste, tuple, dictionnaire, chaîne) |
| | `ampio(liste, v)` / `esory(liste)` | ajouter / retirer le dernier |
| | `misy(c, x)` | contient ? (clé de dictionnaire, élément de liste/tuple, sous-chaîne) |
| | `fanalahidy(dict)` | liste des clés |
| Maths | `faka(x)` | racine carrée |
| | `heriny(x, y)` | x puissance y |
| | `habe(x)` | valeur absolue |
| | `kely(a, b)` / `lehibe(a, b)` | minimum / maximum |
| Chaînes | `avo(s)` / `ambany(s)` | majuscules / minuscules |
| | `fafao(s)` | supprime les espaces autour (trim) |
| | `vaky(s, sep)` | découpe (split) → liste de `teny` |
| | `akambana(liste, sep)` | assemble (join) |
| | `s[i]` | i-ème caractère (`teny`) |
| Entrée | `henoy()` | lit une ligne au clavier (`teny`) |

```oly
asehoy(faka(heriny(3, 2) + heriny(4, 2)));       // 5
aoka mots = vaky("mangoro,akondro,voanio", ",");
asehoy(akambana(mots, " | "));                   // mangoro | akondro | voanio
asehoy("Anaranao ?");
aoka nom = henoy();
asehoy("Salama, " + nom + " !");
```

---

## 4. Tutoriel pas à pas

### Étape 1 — Bonjour le monde

```oly
asehoy("Salama tontolo izao!");
```
```bash
cargo run -- bonjour.oly
```

### Étape 2 — Variables et calculs

```oly
aoka prix = 1500;
aoka quantite = 3;
aoka total = prix * quantite;
asehoy(total); // 4500
```

### Étape 3 — Une fonction

```oly
asa remise(prix: isa, pourcentage: isa) -> isa {
    avereno prix - (prix * pourcentage / 100);
}

asehoy(remise(1000, 20)); // 800
```

### Étape 4 — Une boucle

```oly
isaky (aoka i = 1; i <= 5; i = i + 1) {
    asehoy(i * i); // carrés de 1 à 5
}
```

### Étape 5 — Un struct avec méthode

```oly
endrika CompteBancaire {
    titulaire: teny,
    solde: isa,
}

fomba CompteBancaire {
    asa deposer(ity, montant: isa) {
        ity.solde = ity.solde + montant;
    }
    asa etat(ity) -> teny {
        avereno ity.titulaire + " : " + ity.solde;
    }
}

aoka compte = CompteBancaire { titulaire: "Voahangy", solde: 1000 };
compte.deposer(500);
asehoy(compte.etat()); // "Voahangy : 1500"
```

### Étape 6 — Un tableau

```oly
aoka notes: isa[] = [12, 15, 8, 18, 10];

asa moyenne(liste: isa[]) -> isa {
    aoka total = 0;
    isaky (aoka i = 0; i < habeny(liste); i = i + 1) {
        total = total + liste[i];
    }
    avereno total / habeny(liste);
}

asehoy(moyenne(notes)); // 12
```

### Étape 7 — Programme complet : FizzBuzz

```oly
isaky (aoka i = 1; i <= 20; i = i + 1) {
    raha i % 15 == 0 {
        asehoy("FizzBuzz");
    } na raha i % 3 == 0 {
        asehoy("Fizz");
    } na raha i % 5 == 0 {
        asehoy("Buzz");
    } raha tsy izany {
        asehoy(i);
    }
}
```

### Étape 8 — Erreurs, lambdas et dictionnaires ensemble

```oly
aoka stock = ["pomme": 5, "poire": 0];

asa retirer(fruit: teny) {
    raha tsy misy(stock, fruit) { atsipazo "produit inconnu : " + fruit; }
    raha stock[fruit] == 0 { atsipazo "rupture de stock : " + fruit; }
    stock[fruit] -= 1;
}

aoka essayer = asa(nom) {
    andramo { retirer(nom); asehoy("ok " + nom); }
    sambotra (e) { asehoy("erreur -> " + e); }
};

isaky f amin ["pomme", "poire", "kiwi"] { essayer(f); }
```
Résultat : `ok pomme`, `erreur -> rupture de stock : poire`, `erreur -> produit inconnu : kiwi`.

---

## 5. Annexes

### 5.1. Table des mots-clés

| Mot-clé | Sens |
|---|---|
| `aoka` | déclaration de variable |
| `tsymiova` | déclaration de constante (non encore appliquée) |
| `raha` | si |
| `na raha` | sinon si |
| `raha tsy izany` | sinon |
| `raha mbola` | tant que |
| `isaky` | pour (style C) ou pour-chaque (`isaky x amin liste`) |
| `amin` | « dans » (boucle for-each) |
| `tapaka` | break |
| `manohy` | continue |
| `asa` | fonction (nommée, ou anonyme = lambda) |
| `andramo` / `sambotra` | try / catch |
| `atsipazo` | lever une erreur (throw) |
| `avereno` | return |
| `endrika` | struct |
| `fomba` | bloc de méthodes (impl) |
| `ampiasao` | import de module |
| `asehoy` | afficher (print) |
| `marina` / `diso` | vrai / faux |
| `tsisy` | valeur nulle |
| `sy` / `na` / `tsy` | et / ou / non |

### 5.2. Fonctions natives

| Fonction | Rôle |
|---|---|
| `habeny`, `ampio`, `esory`, `misy`, `fanalahidy` | collections (voir §3.17) |
| `faka`, `heriny`, `habe`, `kely`, `lehibe` | maths (voir §3.17) |
| `avo`, `ambany`, `fafao`, `vaky`, `akambana` | chaînes (voir §3.17) |
| `henoy` | entrée clavier |

### 5.3. Commandes du REPL

| Commande | Effet |
|---|---|
| `:quit`, `:q` | quitter |
| `:aide`, `:help` | afficher l'aide |

### 5.4. Limitations connues

- `tsymiova` (constante) n'est pas encore appliqué comme immuable par le compilateur.
- Le système de modules (`ampiasao`) fait une fusion textuelle simple ; il n'y a pas d'espaces de noms.
- Les blocs imbriqués partagent la portée de leur fonction englobante (pas de vrai scoping par bloc).
- Les lambdas ne capturent pas les variables locales de leur fonction parente (globales uniquement).
- Les appels de lambda via une variable et l'indexation de tuples ne sont pas vérifiés statiquement (vérifiés à l'exécution).
- Les clés de dictionnaire sont limitées à `isa` et `teny` ; l'ordre d'itération passe par `fanalahidy` (clés triées).
- Un `[]` vide désigne toujours une liste ; `n++` / `n--` ne s'utilisent qu'en instruction.
- Les noms des fonctions natives sont des choix pratiques et pourront évoluer (ils sont centralisés dans `vm.rs`/`compiler.rs`).
- Le typage reste graduel : une variable ou un paramètre sans annotation n'est vérifié qu'une fois son type déduit d'une valeur concrète.

### 5.5. Résolution de problèmes

| Symptôme | Cause probable |
|---|---|
| `error: linker 'cc' not found` (Linux) | Installer les outils de compilation : `sudo apt install build-essential` |
| Erreur MSVC au build (Windows) | Installer les "Build Tools for Visual Studio" (composant C++), ou utiliser la cible `-gnu` |
| `ampiasao` ne trouve pas le fichier | Le chemin est résolu relativement au fichier qui importe, pas au dossier courant du terminal |
| Erreur de type inattendue sur un paramètre | Ajouter une annotation (`param: isa`) pour que le vérificateur puisse la contrôler |
