# CNC LAUNCHER

Installa, apre e aggiorna le app CNC di Galbiati Costruzioni Meccaniche (oggi HEIDENHAIN-SIEMENS
CONVERTER) su **Windows** e **macOS (Apple Silicon)**.

## Scaricare il launcher

Dalla pagina **[Releases](../../releases/latest)** di questo repository, nessun account necessario:

- Windows: `cnc-launcher-<versione>-windows-x64.zip` -> estrai e avvia `cnc-launcher.exe`
- macOS: `cnc-launcher-<versione>-macos-arm64.tar.gz` -> estrai e avvia `cnc-launcher`

Il launcher non e' firmato: al primo avvio Windows (SmartScreen) chiede *Ulteriori informazioni ->
Esegui comunque*, macOS chiede *clic destro -> Apri*.

## Installare le app

Le app stanno in un repository **privato**: al primo avvio il launcher chiede un token GitHub di
sola lettura, che si ottiene da chi gestisce il software. Il token viene conservato nel portachiavi
del sistema. Poi **Installa** / **Aggiorna** / **Apri** dalla Libreria. Dettagli: [docs/launcher.md](docs/launcher.md).

## Sviluppo

```bash
cargo run -p launcher_app
cargo test --workspace
```

Nuova versione: tag `launcher-vX.Y.Z` -> la GitHub Action compila Windows e macOS e pubblica la Release.
