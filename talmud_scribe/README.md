# Talmud Scribe

Audio/vidéo de cours de Guemara et notes manuscrites → Word (`.docx`) structuré, avec
hébreu/araméen en RTL et français en LTR. **100 % hors-ligne** (licence MIT).

## Installation (PC i5, 8–16 Go, sans GPU)

```bash
python -m venv .venv && source .venv/bin/activate     # Windows : .venv\Scripts\activate
pip install -r requirements.txt
# ffmpeg obligatoire ; Tesseract + langues heb/fra seulement pour l'OCR
```

## Utilisation

```bash
python main.py transcribe cours.mp3 --model small --context "Baba Kama 42a"
python main.py transcribe cours.mp3 --model ivrit-turbo      # meilleur en hébreu, plus lent
python main.py ocr notes.jpg
python main.py transcribe cours.mp3 --mixed --model small --he-model ivrit-turbo   # français + hébreu
```

**Mode `--mixed`** : passe 1 en français, puis détection de langue sur chaque segment (≥ 1,5 s) ;
ceux détectés hébreux (`--he-threshold`, 0,5 par défaut) sont retranscrits en hébreu par `--he-model`.
Deux modèles sont chargés (≈ 2 Go de RAM avec `small` + `ivrit-turbo`). Whisper n'a pas d'araméen :
l'araméen est transcrit comme de l'hébreu, ce qui est en général lisible.

Modèles : `tiny|base|small|medium|large-v3-turbo|ivrit-turbo|ivrit-large`. Sur i5/8 Go, itérez
avec `small`, réservez `ivrit-turbo` (≈1,6 Go en int8) aux enregistrements importants.
Les résultats sont mis en cache (`~/.cache/talmud_scribe`) : relancer ne retraite rien.

> **Test à faire :** les modèles `ivrit-*` sont affinés sur l'hébreu. Sur un cours surtout en français,
> comparez-les à `small`/`medium` avec `--lang fr`, `--lang he` et `--lang auto` sur votre patron
> (`python main.py calibrate-asr --models small medium ivrit-turbo`).

## Calibration par patrons (facultative, à chaque lancement)

Aucun patron n'est requis : sans option, l'outil utilise le vocabulaire par défaut.
Si vous voulez calibrer, vous fournissez vos patrons au moment du lancement :

```bash
python main.py transcribe cours.mp3 --patron-audio mon_patron.wav --patron-text mon_patron.txt
python main.py ocr notes.jpg --patron-image ma_feuille.png [--patron-text ma_feuille.txt]
```

- `--patron-text` sert de prompt à Whisper ; avec `--patron-audio`, le WER du modèle sur votre voix est affiché.
- `--patron-image` règle le prétraitement OCR pour votre écriture, pour ce lancement seulement.
- `python main.py make-template` génère une feuille guide à recopier à la main.
- `calibrate-asr` / `calibrate-ocr` (avec fichiers dans `calibration/`) restent disponibles pour une calibration mémorisée dans `profile.json`.

Limites assumées :
- Le patron vocal sert aussi de prompt ; on ne mesure donc qu'avec la liste de termes pour ne pas gonfler le score.
- Tesseract est faible sur le manuscrit : la calibration optimise le prétraitement, elle n'entraîne pas de modèle. Pour mieux, brancher un modèle HTR affiné derrière `ocr_engine.recognize()`.
- Aucun modèle n'est inclus ; le premier lancement télécharge les poids (puis tout est hors-ligne).

## Tests

```bash
pytest -q
```

## Arborescence

`core/` (asr, ocr, docx) · `utils/` (preprocessor, cache, metrics, template) · `calibration/` · `main.py`
