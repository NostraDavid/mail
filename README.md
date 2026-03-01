# mail

Rust mail-client project met `direnv + devenv`.

## Starten

1. Sta direnv toe in deze map:

   ```bash
   direnv allow
   ```

2. Enter de shell (optioneel, direnv doet dit automatisch in je terminal):

   ```bash
   devenv shell
   ```

3. Controleer dat de workspace bouwt:

   ```bash
   check
   ```

4. Start de app:

   ```bash
   run
   ```

## Structuur

- `crates/mail-engine`: engine-first core library
- `crates/mail`: binary crate (naam: `mail`)
- `devenv.nix`: dev environment (Rust tooling + WebKit/GTK libs voor iced/webview)

## Turso

De app gebruikt standaard een lokale Turso/libSQL database op `.mail/mail.db`.

## OAuth login (Google/Outlook)

Je hoeft geen OAuth env-variabelen te zetten.

1. Start `run`.
2. Vul in de app je Google/Outlook client-id (en optioneel secret) in.
3. Klik `Login met Google` of `Login met Outlook`.

De ingevulde OAuth-instellingen worden lokaal opgeslagen in `.mail/mail.db`.

### Google OAuth checklist

Als je "Client missing a project id" of `invalid_client` ziet:

1. Gebruik een echte Google OAuth Client ID uit Google Cloud Console (niet projectnummer).
2. De Client ID moet eindigen op `.apps.googleusercontent.com`.
3. Maak bij voorkeur een OAuth client van type `Desktop app` (of `Web application` met loopback redirect).
4. Als je oude/onjuiste waarde hebt opgeslagen: verwijder `.mail/mail.db` en vul opnieuw in.
