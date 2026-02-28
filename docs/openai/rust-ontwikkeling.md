# Rust ontwikkeling

Ja. Praktische tips als je een mail-client in Rust bouwt.

Begin met een “engine-first” architectuur. Maak de synchronisatie en storage een losse crate (library) en bouw
daarbovenop een UI-crate (TUI/GUI) en eventueel een CLI-crate. Dit voorkomt dat je later je protocol- en data-laag moet
“uittrekken” uit je UI.

Kies één async-runtime en blijf consequent. In de praktijk is Tokio de meest frictieloze keuze voor netwerk-I/O, timers
en task orchestration. Zet harde timeouts op DNS/connect/read/write, en ontwerp reconnect/backoff als eerste-klas
gedrag, anders voelt je client willekeurig “instabiel”.

Wees streng in je protocolgrenzen.

* IMAP: ontwerp je sync als state machine per mailbox (initial sync, incremental sync, expunge, UIDVALIDITY-changes,
  reconnect, catch-up). Bewaar per mailbox minimaal: UIDVALIDITY, last-seen UID/ModSeq, flags, en een “watermark” voor
  incremental updates.
* SMTP: bouw een aparte verzend-queue (outbox) die idempotent werkt en retries kan doen zonder dubbele verzending.

Gebruik bij voorkeur rustls voor TLS. Dit scheelt OS-afhankelijke gedoe en is in de praktijk voorspelbaar op
Linux/NixOS. Als je toch native TLS gebruikt, plan dan vroeg hoe je certificaatstores en platformverschillen afhandelt.

OAuth en secrets zijn geen bijzaak.

* Gebruik een OAuth2-client crate met PKCE, refresh tokens en een nette token-cache.
* Sla tokens op in een OS keyring waar mogelijk (en bied een versleutelde file fallback). Op NixOS is het vaak
  praktischer om keyring-integratie als optionele feature te bouwen, zodat je app ook goed werkt in “minimal”
  omgevingen.

Behandel mail parsing als “hostile input”.

* Voor MIME: kies een parser die tolerant is voor broken headers/charsets. Bewaar altijd ook de ruwe RFC822/MIME blob
  zodat je later opnieuw kunt parsen met verbeterde code.
* Voor HTML: render via een sanitizer (bijv. allowlist), blokkeer remote resources standaard, en behandel inline CID
  attachments expliciet.
* Normaliseer headers (folding, encodings) en maak je parsing volledig fallible (Result) met duidelijke
  error-categorieën.

Storage en search bepalen hoe “snel” je client voelt.

* SQLite is prima als basis (embedded, betrouwbaar). Gebruik een schema met: accounts, mailboxes, messages (metadata),
  message_bodies (raw blob), flags/labels, attachments (liefst content-addressed op disk).
* Voor search: SQLite FTS5 is een pragmatische keuze; alternatief is Tantivy als je meer controle wilt. Indexeer
  incrementally, niet “rebuild everything”.

Maak “dedup” en idempotentie expliciet.

* Dedupe op Message-ID is nuttig maar onbetrouwbaar; combineer zo nodig met (date, from, subject, size) heuristiek en
  server-UID-ankers.
* Elke sync-iteratie moet herhaalbaar zijn zonder dat je lokale state corrupt raakt.

Gebruik tracing vanaf dag 1.

* `tracing` met spans per account/mailbox/command (bijv. IMAP FETCH/STORE) is goud waard.
* Log nooit volledige bodies of attachments standaard; log wel message-id, uid, mailbox, response codes, timings, byte
  counts.

Test met echte servers, niet alleen mocks.

* Draai Dovecot (IMAP) en een SMTP-testserver in integratietests (bijv. via testcontainers). Zet tests op voor:
  reconnects, expunges, flags, grote mailboxes, kapotte MIME, en throttling.
* Voeg “fuzz” toe op je MIME/HTML parsing; mail-input is de perfecte fuzz-target.

Houd je build en packaging netjes op NixOS.

* Maak een flake met reproducible builds (bijv. met crane of naersk) en splits features (gui, keyring, html-rendering)
  zodat je minimale build altijd werkt.
* Zet system dependencies (GTK/WebKit/openssl) achter features, en geef een “pure rustls” pad.

Concreet startpad dat vaak werkt: eerst een headless sync-daemon + lokale DB + search, daarna pas UI. Als je UI vroeg
bouwt, ga je te lang “gevoel” optimaliseren terwijl de sync-engine nog niet hard is.
