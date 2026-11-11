# Faut-il un dictionnaire, un modèle spécialisé, ou un fine-tuning ?

Réponse courte : **commencez par le glossaire, les corrections et Large v3 Turbo.**
C'est là que se trouve l'essentiel du gain, pour un coût nul. Le fine-tuning n'est
utile qu'après avoir mesuré ce qui reste faux, avec plusieurs heures de cours et
leurs transcriptions corrigées.

## 1. Ce que Plume fait déjà sans entraînement

| Levier | Effet |
|---|---|
| Modèle `Large v3 Turbo` | Meilleure compréhension de l'hébreu et des changements de registre |
| Contexte « Gemara / Torah / Daf » | Amorçage Whisper avec le vocabulaire du Talmud |
| Glossaire | Noms de Rabbanim, masekhtot, expressions récurrentes |
| Corrections | Remplace les graphies fautives après coup (`רמבם` → `רמב״ם`) |
| Normalisation des guillemets | Typographie hébraïque correcte dans Word |

Un glossaire de 30 à 80 termes bien choisis vaut souvent plus qu'un fine-tuning
bricolé sur 2 heures d'audio.

## 2. Quand envisager un fine-tuning

Quand, sur un échantillon représentatif :

- les mêmes mots rares (noms propres, expressions araméennes) échouent systématiquement ;
- le style de ponctuation ne convient pas ;
- vous disposez d'au moins **10 à 30 heures** d'audio **et** de transcriptions
  corrigées à la main, avec un locuteur et une qualité d'enregistrement proches.

Un fine-tuning sur peu de données dégrade souvent le modèle généraliste et produit
des hallucinations plus confiantes. Mesurez toujours un WER (taux d'erreur de mots)
sur un jeu de validation mis de côté.

## 3. Recette pratique (Whisper)

1. **Données** : paires `audio 16 kHz mono` + texte exact, ponctué, avec les
   abréviations développées (`רש״י` et non `רשי`). Nettoyez les répétitions.
2. **Base** : `openai/whisper-large-v3-turbo` (ou un modèle hébreu déjà affiné,
   par exemple les publications d'`ivrit-ai`).
3. **Entraînement** : Hugging Face `transformers`
   (`WhisperForConditionalGeneration`, `Seq2SeqTrainer`) ou LoRA pour réduire le
   coût. Geler l'encodeur dans un premier temps est un bon compromis.
4. **Évaluation** : WER + relecture humaine sur 1 à 2 heures de cours jamais vus.
5. **Conversion** vers le format whisper.cpp :
   - exporter le modèle HF ;
   - utiliser `models/convert-h5-to-ggml.py` de whisper.cpp ;
   - quantifier éventuellement avec l'outil `quantize` de whisper.cpp.

## 4. Brancher le modèle affiné dans Plume

Plume importe directement un fichier **GGML Whisper** (`ggml-*.bin`) :

1. convertir votre modèle Hugging Face en GGML avec whisper.cpp ;
2. ouvrir **Réglages → Moteur de transcription** ;
3. cliquer **Importer…** sur la ligne « Modèle affiné » et choisir le `.bin` ;
4. sélectionner « Modèle affiné », enregistrer.

Le fichier est copié dans
`~/Library/Application Support/com.plume.desktop/models/custom-ggml.bin`. Pour revenir
au modèle généraliste, sélectionnez Large v3 Turbo. (L'ancienne méthode — remplacer
le fichier du catalogue — reste possible.)

## 5. Alternative au fine-tuning : modèles hébreux existants

Avant d'entraîner, testez les modèles hébreu publiés (ivrit-ai, Whisper fine-tunés
communautaires). Leur conversion en GGML suit la même procédure. Vérifiez la licence
avant tout usage distribué.

## 6. Aramaïque : le vrai point dur

Il n'existe pas de modèle Whisper araméen. L'araméen talmudique s'écrit en caractères
hébraïques et Whisper le traite comme de l'hébreu. Les pistes réalistes :

- glossaire d'expressions talmudiques (`איבעיא להו`, `תא שמע`, `מאי טעמא`,
  `והא אמרינן`, `לאו דווקא`…) ;
- fine-tuning sur des enregistrements de Guemara **avec** leur transcription ;
- post-traitement par un petit modèle de langue spécialisé (pas encore intégré).

## 7. Piste produit : détection de langue par segment

Pour les cours bilingues français/hébreu, la solution propre n'est pas un fine-tuning
mais un découpage sur les silences, une détection de langue par segment, puis une
transcription avec la langue dominante du segment. C'est le prochain développement
naturel de Plume.
