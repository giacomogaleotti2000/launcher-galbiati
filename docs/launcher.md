# CNC LAUNCHER: accesso privato alle app

Il launcher (`cnc-launcher`) scarica e aggiorna le app (oggi HEIDENHAIN-SIEMENS CONVERTER) dalle
**Release private** del repository `giacomogaleotti2000/HEIDEN-SIEMENS`. Il repository resta privato:
per leggerlo il launcher usa un **token personale di sola lettura**, valido solo per quel repository.

## 1. Creare il token (una volta, ~3 minuti)

1. Accedi a GitHub con l'account `giacomogaleotti2000`.
2. In alto a destra: foto del profilo -> **Settings**.
3. In fondo al menu a sinistra: **Developer settings**.
4. **Personal access tokens** -> **Fine-grained tokens** -> **Generate new token**.
   (Indirizzo diretto: <https://github.com/settings/personal-access-tokens/new>)
5. Compila:
   - **Token name**: `cnc-launcher <nome del PC>` (es. `cnc-launcher ufficio`), uno per PC;
   - **Expiration**: 90 giorni o 1 anno (alla scadenza basta crearne uno nuovo);
   - **Resource owner**: `giacomogaleotti2000`;
   - **Repository access**: **Only select repositories** -> scegli **HEIDEN-SIEMENS** (solo quello);
   - **Permissions** -> **Repository permissions** -> **Contents**: **Read-only**.
     Non serve nient'altro (Metadata: Read-only viene aggiunto da solo).
6. **Generate token** e copia il valore (`github_pat_...`): GitHub lo mostra **una sola volta**.

Non salvarlo in file, mail o chat: incollalo direttamente nel launcher.

## 2. Inserirlo nel launcher

1. Avvia `cnc-launcher` (al primo avvio compare "Accesso alle app private").
2. Incolla il token nel campo **TOKEN GITHUB** e premi **Salva e continua** (o Invio).
3. Il launcher prova il token su GitHub e lo salva **solo se funziona**, nel portachiavi del sistema:
   - Windows: *Gestione credenziali* (voce `cnc-launcher`);
   - macOS: *Accesso Portachiavi* (voce `cnc-launcher`).
   Non viene mai scritto in un file di testo né nei log.
4. Nella **Libreria**: **Installa** scarica l'archivio della tua piattaforma, controlla l'impronta
   SHA-256 indicata nel `manifest.json` della Release e lo installa nella cartella dell'utente:
   - Windows: `%LOCALAPPDATA%\cnc-launcher\app\...`
   - macOS: `~/Library/Application Support/cnc-launcher/app/...`

   Poi **Apri** avvia l'app. Quando esce una versione nuova compare **AGGIORNAMENTO** e il pulsante
   **Aggiorna** (la versione vecchia viene rimossa).

## 3. Messaggi di errore

| Messaggio                         | Cosa fare                                                                 |
|-----------------------------------|---------------------------------------------------------------------------|
| token non valido o scaduto (401)  | crea un token nuovo (punto 1) e inseriscilo da *Impostazioni -> Sostituisci token* |
| accesso negato (403)              | il token non ha *Contents: Read-only*: rigeneralo con quel permesso         |
| repository non trovato (404)      | il token non include il repository HEIDEN-SIEMENS                          |
| nessun rilascio                   | non è ancora stata pubblicata una Release `converter-vX.Y.Z`               |
| impronta SHA-256 diversa          | file scaricato male o alterato: riprova; se si ripete, non usare quel file |

## 4. Revocare o sostituire il token

- Nel launcher: **Impostazioni -> Rimuovi token** (lo cancella dal portachiavi).
- Su GitHub: *Settings -> Developer settings -> Fine-grained tokens* -> apri il token -> **Delete**.
  Fallo subito se perdi o cambi un PC: gli altri PC hanno il loro token e continuano a funzionare.

## 5. Primo download del launcher

Il launcher si scarica una volta dalla pagina pubblica delle Release, senza account:
<https://github.com/giacomogaleotti2000/cnc-launcher/releases/latest> -> file
`cnc-launcher-<versione>-windows-x64.zip` o `...-macos-arm64.tar.gz`. Estrai la cartella dove
preferisci e avvia `cnc-launcher.exe` (Windows) o `cnc-launcher` (macOS). Le app invece si
aggiornano dal launcher.

- **Windows**: SmartScreen può avvisare ("Windows ha protetto il PC") perché l'eseguibile non è
  firmato: *Ulteriori informazioni -> Esegui comunque*.
- **macOS**: la prima volta, clic destro sull'eseguibile -> **Apri** -> **Apri** (app non firmata).
  In alternativa, nel Terminale: `xattr -dr com.apple.quarantine <cartella estratta>`.

## In futuro

- Accesso con "Sign in with GitHub" (device flow) al posto del token incollato a mano (TODO nel
  codice, `launcher_core::source::GitHubSource`).
- Sorgente alternativa da cartella di rete (`SourceSpec::NetworkShare`), senza cambiare il resto.
