# Plume

**Vos cours de Torah, de Guemara et de halakha, en texte — et en Word mis en page.**

Plume est une application macOS / Windows, gratuite et open source, qui transforme un
fichier audio ou vidéo en transcription propre. Tout se passe sur votre ordinateur :
aucun fichier n'est envoyé à un service en ligne.

Elle reprend la même base que [Handy](https://github.com/cjpais/Handy) — Whisper en
local, modèles GGML, Tauri + Rust — mais pour un autre usage : Handy sert à dicter
dans un champ de texte ; Plume importe **des fichiers entiers** (un shiur d'une heure,
un MP3 de téléphone, un enregistrement Zoom…) et rend un document prêt à étudier.

## Ce que fait Plume

- **Import audio et vidéo** : MP3, M4A/AAC, WAV, FLAC, OGG, AIFF, MP4, MOV, MKV…
  glisser-déposer, sélection multiple, file d'attente avec progression et annulation.
- **Transcription locale** avec Whisper (tiny → large-v3-turbo), accélérée par le GPU
  quand c'est possible, sinon CPU.
- **Hébreu et araméen en RTL** : les paragraphes hébraïques sont détectés et mis en
  forme de droite à gauche dans le document Word ; le français reste LTR.
- **Lecteur audio intégré** : lecture/pause, recul et avance de 10 s, barre de
  progression. Le paragraphe en cours est surligné et le texte suit l'audio.
- **Repérage des mots incertains** : les mots transcrits avec peu de confiance sont
  soulignés en rouge ; un clic propose une correction issue du lexique.
- **Apprentissage** : chaque correction acceptée est mémorisée, réappliquée aux
  transcriptions suivantes et ajoutée au vocabulaire du modèle.
- **Word vraiment mis en page** : titre, date, tableau source/durée/langue/modèle,
  paragraphes justifiés, pied de page numéroté, propriétés du document.
- **Exports** : `.docx`, `.txt`, `.md`, `.srt` (sous-titres).
- **Sujet détecté depuis le nom du fichier** : `baba-kama-42a.mp3` prépare le traité,
  la page et les termes probables. Sinon, choix manuel du traité, de la paracha ou du
  thème de halakha.
- **Lexique Talmud de 291 entrées** : traités, concepts, araméen talmudique, halakha,
  Rishonim, Tannaïm/Amoraïm, mesures, parachiot, avec **champs lexicaux liés**
  (un traité propose automatiquement ses notions voisines).
- **Banque de 8 980 mots** (Strong hébreu biblique + Jastrow talmudique) consultable
  dans l'onglet **Dictionnaire** ; elle alimente aussi les suggestions de correction.
- **Réglages d'étude** : contexte facultatif, termes techniques facultatifs, glossaire
  personnel, corrections automatiques, normalisation des guillemets hébraïques.
- **Modèle affiné importable** : un Whisper GGML spécialisé (fine-tuning) peut être
  chargé depuis les réglages.
- **100 % hors ligne** après le téléchargement du modèle.

## Télécharger

- **macOS (Apple Silicon)** : [Plume-0.3.0-arm64.dmg](https://github.com/arielbaz/plume/releases/download/v0.3.0/Plume-0.3.0-arm64.dmg)
- **Windows** : [installateur .exe](https://github.com/arielbaz/plume/releases/download/v0.3.0/Plume_0.3.0_x64-setup.exe)
  ou [paquet .msi](https://github.com/arielbaz/plume/releases/download/v0.3.0/Plume_0.3.0_x64_en-US.msi)
- **Android** : [APK arm64](https://github.com/arielbaz/plume/releases/download/v0.3.0/Plume-0.3.0-android-arm64.apk)
- **Toutes les versions** : [github.com/arielbaz/plume/releases](https://github.com/arielbaz/plume/releases)
- **Article de présentation** : [aribaz.com/articles/plume.html](https://aribaz.com/articles/plume.html)

Le build Windows est produit par GitHub Actions (workflow `.github/workflows/build.yml`).
Les binaires ne sont pas signés : macOS → clic droit **Ouvrir** ; Windows →
SmartScreen → **Informations complémentaires** → **Exécuter quand même**.

## Démarrage rapide (macOS)

Plume est installée dans `/Applications/Plume.app`. Le modèle Large v3 Turbo est
déjà présent sur cette machine.

Ouvrez-la, laissez-la télécharger le modèle **Large v3 Turbo** (574 Mo, une seule
fois), puis glissez un fichier audio dans la fenêtre. Le Word et le texte sont créés
dans `~/Documents/Plume`.

Au premier lancement, macOS peut demander une confirmation (application non signée) :
clic droit → **Ouvrir**.

## Écouter, vérifier, corriger

Après une transcription :

1. **Lecteur** : bouton lecture/pause, `−10` / `+10`, curseur de position.
2. **Suivi du texte** : le paragraphe correspondant à l'instant audio est surligné et
   se cale au centre. Décochez « Suivre » pour lire librement.
3. **Navigation** : cliquez un paragraphe pour aller à ce moment ; cliquez un mot
   souligné en rouge pour voir la suggestion et l'accepter.
4. **Apprentissage** : la correction est appliquée au texte, au Word et au SRT, puis
   mémorisée (`learning.json`). Les prochaines transcriptions l'appliquent d'office.

## Sujet, page et vocabulaire

Le nom du fichier est analysé à l'import (`baba-kama-42a.mp3`, `chabbat_15b`,
`פרשת וירא`, `yoreh-deah-kashrout`…). Plume reconnaît le traité / la paracha, extrait
la page quand elle est présente, et prépare les termes probables.

Le bouton **Sujet du cours** permet de changer manuellement :
- onglet **Talmud** : les 63 traités ;
- onglet **Torah** : les 54 parachiot ;
- onglet **Halakha** : thèmes et sections du Choulhan Aroukh.

Les **termes liés** du lexique sont proposés automatiquement (Baba Kama → שור, בור,
הבער, אדם, מזיק, נזיקין, תשלומין…). Vous pouvez en retirer, en ajouter, et saisir des
termes techniques ou un contexte libres.

## Utilisation pour un cours de Guemara

1. **Langue principale** : `Hébreu / araméen` pour un shiur en hébreu, `Français`
   pour un cours en français avec des citations hébraïques.
2. **Contexte du cours** : `Gemara / Talmud (iyoun)` ajoute automatiquement un
   vocabulaire de départ (מסכת, דף, עמוד, רש״י, תוספות, סוגיא, קושיא, תירוץ,
   איבעיא להו, תא שמע, שולחן ערוך…).
3. **Glossaire** : ajoutez les noms propres et expressions récurrents de votre
   rabbi ou de votre masekhet. Ils orientent Whisper avant la transcription.
4. **Corrections** : `רמבם = רמב״ם`, `רשי = רש״י`, etc. Elles s'appliquent au texte
   final, sans retoucher l'audio.
5. Choisissez **Large v3 Turbo** : c'est le modèle qui s'en sort le mieux en hébreu.
   `Small` est deux à trois fois plus rapide mais nettement moins précis.

### Cours bilingues

Whisper travaille avec **une langue principale** à la fois. En pratique :

- cours surtout en **français**, citations en hébreu/araméen → choisissez `Français` ;
- cours surtout en **hébreu**, explications en français → choisissez `Hébreu /
  araméen` ; les termes français sont alors souvent rendus en caractères hébraïques.

Le glossaire et les corrections compensent une partie de ces limites. Une détection
de langue par segment (pour basculer automatiquement entre hébreu et français) est
le prochain chantier naturel ; voir [docs/FINETUNING.md](docs/FINETUNING.md) pour
les autres pistes.

## Précision : ce qui est mis en œuvre

- **Large v3 Turbo** par défaut, le modèle Whisper le plus solide en hébreu.
- **Beam search (5 faisceaux)** au lieu d'un décodage glouton : sorties plus
  stables sur les phrases longues et les termes techniques.
- **Normalisation audio** : suppression de l'offset DC et calibrage du niveau
  avant l'ASR, sans modifier la durée (le lecteur reste synchronisé).
- **Contexte de domaine** : preset Guemara/Torah/Daf, traité et page détectés,
  termes liés, glossaire et termes techniques.
- **Apprentissage** : corrections acceptées réappliquées, termes mémorisés.
- **Signalement des mots incertains** avec suggestion issue du lexique.
- **Modèle affiné** importable pour aller plus loin.

## Modèles

| Modèle | Taille | Usage |
|---|---:|---|
| Tiny | 32 Mo | test, brouillon |
| Base | 60 Mo | mémos courts |
| Small | 190 Mo | bon compromis vitesse/précision |
| Medium | 539 Mo | plus précis, plus lent |
| **Large v3 Turbo** | **574 Mo** | **recommandé pour l'hébreu et l'araméen** |
| Modèle affiné | variable | Whisper GGML issu de votre fine-tuning |

Les modèles sont téléchargés à la demande depuis le dépôt officiel
`ggerganov/whisper.cpp` (quantification Q5). Vous pouvez en supprimer un depuis les
réglages ; les fichiers vivent dans
`~/Library/Application Support/com.plume.desktop/models` (macOS) ou
`%APPDATA%\com.plume.desktop\models` (Windows).

## Android

Un APK arm64 est fourni dans la release (Android 7.0 / API 24 minimum). Pour le
construire soi-même, avec le SDK Android, le NDK 27, le JDK 17, CMake et Ninja :

```bash
./scripts/build-android.sh
```

La signature de release utilise un keystore fourni par l'utilisateur
(`PLUME_ANDROID_KEYSTORE`, `PLUME_ANDROID_STOREPASS`). Sans keystore, l'APK produit
n'est pas signé. Le modèle Whisper se télécharge dans l'application, comme sur
ordinateur ; compter 574 Mo pour Large v3 Turbo.

## Compiler soi-même

Prérequis : Rust, Node.js, CMake, Xcode Command Line Tools (macOS) ou Visual Studio
Build Tools + LLVM (Windows).

```bash
cd plume
python3 -m venv .tools && .tools/bin/pip install cmake   # macOS, si cmake manque
export PATH="$PWD/.tools/bin:$PATH"
npm install --cache ./.npm-cache
npx tauri build
```

Sous Windows :

```powershell
py -m venv .tools
.\.tools\Scripts\pip install cmake
$env:PATH = "$PWD\.tools\Scripts;$env:PATH"
npm install
npx tauri build
```

`whisper.cpp` est compilé avec l'application. Le dossier `src-tauri/vendor/whisper-rs-sys`
contient une version récente de whisper.cpp : la version vendue par le crate Rust
échouait à l'encodage sur macOS récent (`failed to encode`). Le correctif consiste à
remplacer la source embarquée et à adapter le parseur de version dans son `build.rs`.

### Ligne de commande

```bash
Plume.app/Contents/MacOS/plume --transcribe cours.m4a \
  --model turbo --lang he --preset gemara \
  --out ~/Documents/Plume --timestamps
```

Utile pour automatiser un dossier entier ou vérifier un modèle.

## Architecture

```
src-tauri/src/
  audio.rs      décodage Symphonia + rééchantillonnage Rubato → 16 kHz mono
  engine.rs     whisper.cpp (whisper-rs) : chargement, progression, annulation
  jobs.rs       file d'attente, états, prompts, cycle de vie
  learning.rs   corrections apprises et termes mémorisés
  text.rs       paragraphes, RTL, corrections, confiance, horodatage
  ui/lexicon.json  lexique Talmud et champs lexicaux (source unique UI + Rust)
  docx.rs       génération OOXML (Word) sans dépendance externe
  export.rs     TXT / Markdown / SRT / DOCX
  models.rs     catalogue et téléchargement des modèles
  lib.rs        commandes Tauri + interface
ui/             interface (HTML/CSS/JS, sans build front)
```

Le décodage audio est adapté de `audio_toolkit/audio/decode.rs` de Handy (licence MIT).

## Accélération sur Mac Apple Silicon

- **GPU Metal activé** : whisper.cpp encode et décode sur le GPU via Metal 4.
  Sur le Mac M5 de test, un cours de **2 min 15** est transcrit en **~15 s**
  avec Large v3 Turbo (contre ~69 s en CPU, soit environ 4,5× plus rapide).
- Le CPU reste utilisé pour certaines opérations et sert de repli :
  `PLUME_CPU=1` force le CPU (utile si un pilote Metal pose problème).
- Le modèle reste chargé en mémoire entre deux fichiers : le coût de démarrage
  n'est payé qu'une fois par session.
- **Neural Engine (ANE)** : non utilisé directement. Whisper.cpp ne sait pas
  l'exploiter ; il faudrait convertir l'encodeur en Core ML (poids, formes fixes,
  qualité à vérifier). Le décodeur, qui domine le temps de calcul, resterait de
  toute façon sur GPU/CPU. Piste envisagée, pas prioritaire.
- Sous Windows, le moteur tourne sur CPU pour l'instant ; un backend Vulkan
  pourra être ajouté plus tard.

## Dépannage

- **Le sélecteur de fichiers figeait l'application** : corrigé. Le dialogue natif
  est maintenant ouvert de façon asynchrone (les appels `blocking_*` de Tauri
  bloquaient la boucle d'événements macOS).
- **macOS refuse d'ouvrir l'app** (application non notarisée) : clic droit sur
  `Plume.app` → **Ouvrir**, ou `xattr -dr com.apple.quarantine /Applications/Plume.app`.
- **Modèle absent** : ouvrez Réglages → téléchargez Large v3 Turbo, ou lancez
  l'application une première fois avec Internet.

## Limites connues

- Pas encore de diarisation (séparation des intervenants).
- Les suggestions de mots incertains restent probabilistes : vérifiez à l'écoute.
- Le lecteur audio n'est disponible que pendant la session (pas d'historique
  persistant des transcriptions pour l'instant).
- L'araméen du Talmud n'a pas de modèle dédié : il passe par l'hébreu, avec les
  erreurs que cela implique sur les expressions rares.
- Une seule langue principale par transcription (voir ci-dessus).
- Le texte n'est pas éditable dans l'application ; le Word est fait pour être
  annoté ensuite.
- La version Windows est fournie par le workflow de build, mais n'a pas encore été
  testée sur une machine Windows réelle.

## Licence et données

Code sous MIT. Les données `ui/wordbank.json` proviennent de Strong (CC-BY-SA,
Open Scriptures) et Jastrow (domaine public, via Sefaria) : voir
[NOTICE.md](NOTICE.md). Whisper.cpp, whisper-rs et Handy conservent leurs licences.
