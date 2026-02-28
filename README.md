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

Kopieer `.env.example` naar `.env.local` en vul je waarden in.
