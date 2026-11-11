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
```

Modèles : `tiny|base|small|medium|large-v3-turbo|ivrit-turbo|ivrit-large`. Sur i5/8 Go, itérez
avec `small`, réservez `ivrit-turbo` (≈1,6 Go en int8) aux enregistrements importants.
Les résultats sont mis en cache (`~/.cache/talmud_scribe`) : relancer ne retraite rien.

## Calibration par patrons

1. Lisez `calibration/vocal_text.txt` (~1 min) → `calibration/vocal_patron.wav`.
2. `python main.py make-template`, imprimez `feuille_guide.png`, recopiez à la main, scannez → `calibration/manuscrit_patron.png`.
3. `python main.py calibrate-asr` compare `small` et `ivrit-turbo` (WER) ; `python main.py calibrate-ocr` cherche le meilleur prétraitement (CER). Résultat dans `calibration/profile.json`.

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
