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
   just setup
   just check
   just build
   ```

4. Start de app:

   ```bash
   just run
   ```

## Structuur

Voer `just update` in de shell uit om de devenv- en Cargo-lockfiles te verversen.
`devenv.nix` en `devenv.lock` definiëren de ontwikkelomgeving. Activeer die met
`devenv shell` of direnv.
Controleer daarna de wijzigingen met `just check`. Deze checks starten de app
niet en gebruiken geen mailaccount. De bestaande shellcommando's `build`,
`check`, `fmt`, `lint`, `test` en `run` blijven beschikbaar.

`reqwest` blijft op 0.12 omdat `oauth2` 5 deze versie voor zijn HTTP-client
vereist. Werk beide samen bij wanneer `oauth2` een nieuwere client ondersteunt.

- `crates/mail-engine`: engine-first core library
- `crates/mail`: binary crate (naam: `mail`)
- `devenv.nix`: Rust-tooling en de X11/Wayland-bibliotheken voor Iced

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
