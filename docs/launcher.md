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

   Poi **Apri** avvia l'app. Aggiornamenti: punto 6.

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

## 5. Primo download e installazione del launcher (senza password di amministratore)

Il launcher si scarica una volta dalla pagina pubblica delle Release, senza account:
<https://github.com/giacomogaleotti2000/launcher-galbiati/releases/latest> -> file
`cnc-launcher-<versione>-windows-x64.zip` o `...-macos-arm64.tar.gz`.

**Windows**

1. Estrai lo zip dove capita (es. *Download*) e avvia `cnc-launcher.exe`. SmartScreen può avvisare
   ("Windows ha protetto il PC") perché l'eseguibile non è firmato: *Ulteriori informazioni ->
   Esegui comunque*.
2. In alto compare **Installa per questo utente**: premilo. Il launcher si copia in
   `%LOCALAPPDATA%\cnc-launcher\launcher\cnc-launcher.exe`, crea il collegamento **CNC Launcher**
   nel menu Start e sul Desktop e si riapre da lì. La cartella estratta in *Download* si può
   cancellare.
3. Tutto (launcher, app, aggiornamenti) sta nel profilo dell'utente: **nessuna password di
   amministratore**, né ora né agli aggiornamenti. Non mettere il launcher in *Programmi*
   (`C:\Program Files`): lì ogni aggiornamento chiederebbe l'amministratore.

Se il PC aziendale blocca i programmi avviati da `%LOCALAPPDATA%` (criteri AppLocker / Smart App
Control), l'amministratore deve consentire una volta la cartella `%LOCALAPPDATA%\cnc-launcher`:
è l'unico passaggio che può richiedere l'amministratore, e si fa una volta sola.

**macOS**: estrai l'archivio dove preferisci; la prima volta clic destro sull'eseguibile ->
**Apri** -> **Apri** (app non firmata). In alternativa, nel Terminale:
`xattr -dr com.apple.quarantine <cartella estratta>`.

## 6. Aggiornamenti

- **App** (es. il convertitore): nella Libreria compare **AGGIORNAMENTO** -> **Aggiorna**. La nuova
  versione si installa nel profilo dell'utente, la vecchia viene rimossa.
- **Launcher**: a ogni avvio controlla la Release pubblica `launcher-vX.Y.Z` (senza token). Se ce
  n'è una più recente compare **Aggiorna launcher**: scarica l'archivio, controlla l'impronta
  SHA-256 pubblicata nel file `.sha256` e sostituisce l'eseguibile, poi si riapre. La versione in
  uso è in *Impostazioni -> Launcher*.
- I launcher fino alla 0.4.0 non sanno aggiornarsi da soli: va scaricata a mano una volta la 0.5.0
  (punto 5); da lì in poi si aggiorna da solo.

## In futuro

- Accesso con "Sign in with GitHub" (device flow) al posto del token incollato a mano (TODO nel
  codice, `launcher_core::source::GitHubSource`).
- Sorgente alternativa da cartella di rete (`SourceSpec::NetworkShare`), senza cambiare il resto.
