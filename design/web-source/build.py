"""Build the self-contained Tundra HTML prototype (no external assets or dependencies)."""
from pathlib import Path
import argparse

def build(destination: Path) -> None:
    root = Path(__file__).resolve().parent
    html = (root / 'index.html').read_text()
    for name in ('tokens.css', 'styles.css', 'coin-styles.css', 'refined.css', 'brand.css'):
        html = html.replace(f'<link rel="stylesheet" href="{name}">', '<style>\n' + (root/name).read_text() + '\n</style>')
    for name in ('theme.js','qr-assets.js','descriptor.js','camera.js','examples.js','coins.js','coin-ui.js','app.js'):
        html = html.replace(f'<script src="{name}"></script>', '<script>\n' + (root/name).read_text() + '\n</script>')
    destination.parent.mkdir(parents=True,exist_ok=True)
    destination.write_text(html)
    print(destination)

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,default=Path(__file__).resolve().parent.parent/'Tundra-Wallet-Prototype.html')
    build(parser.parse_args().out)
