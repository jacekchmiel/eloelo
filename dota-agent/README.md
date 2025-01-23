# DotA Agent

Parses DotA 2 screenshots to scrap data for EloElo.


# Dependencies

## Linux (Ubuntu)

The new and wonderful python package manager:
```
curl -LsSf https://astral.sh/uv/install.sh | sh
```

OCR libraries
```
sudo apt install tesseract-ocr libtesseract-dev
```

## Windows

The new and wonderful python package manager:
```
powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
```

For windows, download and install from: https://github.com/UB-Mannheim/tesseract/wiki

# Usage

```
uv run dota-agent.py watch <path-to-dota-screenshot-dir>
```