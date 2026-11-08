# Prompt de release Biblo — à copier dans une session Vibe du projet « biblo »

> Ce fichier décrit le cycle de release. Copie son contenu dans le prompt de ta
> prochaine session Vibe (projet biblo) et remplace les `<...>` par tes valeurs
> ( depuis ton gestionnaire de mots de passe ). Ce dépôt étant public, **aucun
> identifiant ni secret n'y figure** : l'agent devra te demander l'app password
> Nextcloud au moment de l'upload.

---

Tu es dans une session Vibe Code sur le dépôt `GPages/biblo` (appli Windows,
Rust/eframe). Le bac à sable est neuf à chaque session : seul le dépôt GitHub
persiste (branche / PR). L'accès réseau sortant fonctionne. Il n'y a **pas de
connecteur Nextcloud** ici : WebDAV en appels directs. Côté Outlook, seul le
brouillon est disponible (pas d'envoi direct).

Réalise le cycle de release complet :

1. **Développement**
   - branche `vibe/<sujet>-eca5fa`, commits, `cargo test` (tout doit passer) ;
   - PR en brouillon ; merge sur `main` — le token GitHub de la session ne permet
     pas `gh pr merge` : fais un merge local (fast-forward) puis
     `git push origin main`, ou merge via l'interface GitHub.

2. **Version** (si nouvelle release)
   - `Cargo.toml` + `Cargo.lock` : passer à `X.Y.Z` ;
   - recompiler et vérifier que l'exe embarque la version (chaîne UTF-16 dans le PE).

3. **Compilation Windows depuis Linux**
   - `rustup target add x86_64-pc-windows-gnu` ;
   - télécharger llvm-mingw (releases GitHub `mstorsjo/llvm-mingw`, asset
     `llvm-mingw-<date>-ucrt-ubuntu-22.04-x86_64.tar.xz`), extraire dans `/tmp` ;
   - copier `libgcc.a` et `libgcc_eh.a` du paquet Ubuntu `gcc-mingw-w64-x86-64-posix`
     (`pool/universe/g/gcc-mingw-w64/`) dans `<llvm-mingw>/x86_64-w64-mingw32/lib/` ;
   - build :
     ```
     PATH=<llvm-mingw>/bin:$PATH \
     CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=<llvm-mingw>/bin/x86_64-w64-mingw32-gcc \
     cargo build --release --target x86_64-pc-windows-gnu
     ```
   - vérifier le binaire : en-tête `MZ`/PE, taille, sha256 (à communiquer).

4. **Upload Nextcloud (WebDAV)**
   - demande-moi l'**app password** Nextcloud : ne le stocke jamais dans un
     fichier, ne l'affiche jamais (variable d'environnement en mémoire, puis
     `unset`) ;
   - base `<url-nextcloud>`, utilisateur `<user-nextcloud>`, dossier `biblo` ;
   - `MKCOL /remote.php/dav/files/<user>/biblo/` (405 = existe déjà) ;
   - `PUT /remote.php/dav/files/<user>/biblo/biblo-<X.Y.Z>.exe` (écrase l'ancien) ;
   - vérifier : `PROPFIND` (taille) puis `GET` et comparer le sha256 au build local ;
   - lien de téléchargement public (partage du dossier `biblo`) :
     `<url-partage>/download?path=/&files=biblo-<X.Y.Z>.exe` — le tester sans
     authentification (200 + même sha256).

5. **Mail (Outlook)**
   - seul `outlook_draft_outlook_email` existe ici : créer un **brouillon**
     (l'utilisateur cliquera sur « Envoyer » dans Outlook) ;
   - destinataire `<destinataire>`, sujet « Biblo `<X.Y.Z>` — nouvelle version disponible » ;
   - corps : lien de téléchargement, nouveauté de la release, et la mention
     « Ce message a été généré automatiquement. »

**Règles de sécurité** : aucun secret dans les fichiers ou les commits ; app
password en mémoire uniquement ; conseiller de révoquer l'app password après
la release.
