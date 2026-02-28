# Mail-client ontwikkeling

Als je een mail-client wilt bouwen die in de praktijk werkt, moet je vooral scherp zijn op (1) protocollen +
authenticatie, (2) synchronisatie + offline opslag, (3) berichtverwerking (MIME/HTML), en (4) search/UX-prestaties.

1. Protocollen en provider-compatibiliteit Neem IMAP + SMTP als baseline. IMAP4rev2 is de huidige basisspecificatie (RFC
   9051). ([RFC Editor][1]) Voor een bruikbare client heb je in de praktijk ook belangrijke IMAP-extensies nodig: IDLE
   (push-achtige updates), CONDSTORE/QRESYNC (snelle resync), MOVE (server-side verplaatsen), en SPECIAL-USE
   (Sent/Drafts/Trash automatisch herkennen). ([RFC Editor][2]) Overweeg daarnaast JMAP als “moderne sync-API” (veel
   simpeler change-tracking dan IMAP), maar reken erop dat je alsnog IMAP nodig hebt voor brede compatibiliteit. ([RFC
   Editor][3])

2. Authenticatie: gebruikersnaam/wachtwoord is geen veilige aanname meer Voor Gmail moet je OAuth gebruiken met XOAUTH2
   voor IMAP/SMTP, en Google heeft “less secure apps” (basic auth) voor third-party access uitgefaseerd richting OAuth.
   ([Google for Developers][4]) Voor Exchange Online/Microsoft 365 wordt Basic Auth voor SMTP AUTH (client submission)
   uitgefaseerd in 2026; je moet dus ook daar OAuth/Modern Auth ondersteunen als je breed wilt werken.
   ([TECHCOMMUNITY.MICROSOFT.COM][5]) Dit betekent concreet: OAuth2 flow (bij voorkeur met PKCE), token refresh, en
   veilige token-opslag (OS keyring) zijn geen “nice to have”.

3. Sync-engine: dit is het echte product Je hebt een deterministische state machine nodig per account/mailbox: initial
   sync, incremental sync, conflict cases, reconnects, expunges, UIDVALIDITY-wissels, en “catch-up” na offline. De
   combinatie CONDSTORE/QRESYNC + IDLE bepaalt of je client snel en batterijvriendelijk aanvoelt. ([IETF
   Datatracker][6]) Houd rekening met provider-limieten: Gmail IMAP-verbindingen zijn bijvoorbeeld beperkt in
   sessieduur. ([Google for Developers][7])

4. Lokale opslag en indexering Bepaal je lokale “source of truth”: meestal een embedded database (bijv. SQLite) met een
   duidelijke scheiding tussen (a) ruwe MIME blob, (b) genormaliseerde metadata (From/To/Date/Message-Id/flags/labels),
   en (c) full-text index. Zonder goede incremental indexing en snelle queries wordt de UI traag, ongeacht je rendering.

5. MIME, attachments en HTML-rendering: veel randgevallen Je moet robuust kunnen: multipart/alternative, inline
   attachments (CID), verschillende charsets/encodings, en foutieve mails “best effort” tonen. Voor HTML geldt:
   agressief sanitizen, geen actieve content, en remote resources standaard blokkeren (privacy + veiligheid). Dit is
   vaak een primaire reden dat bestaande clients “onveilig of irritant” voelen.

6. UX-keuzes die je vroeg moet vastleggen Threading-model (conversations), labels vs folders (Gmail is label-achtig),
   regels voor “Archive/Delete”, en een compose-editor (plain-text/HTML, quoting, signatures). Deze keuzes bepalen je
   datamodel en sync-semantiek, dus liever vroeg expliciet dan later “repareren”.

7. Een realistische MVP-scope Een haalbare eerste versie is meestal: één accounttype (IMAP+SMTP), OAuth2 waar nodig,
   Inbox + Sent + Drafts + Trash via SPECIAL-USE, basissearch, en een stabiele offline sync met IDLE + (liefst)
   QRESYNC/CONDSTORE. ([RFC Editor][8])

Als je dit goed neerzet, heb je al een client die in snelheid, betrouwbaarheid en controle vaak beter kan worden dan
veel bestaande opties; de rest (JMAP, PGP/S/MIME, rules, calendar/contacts) bouw je er daarna iteratief bovenop.

[1]: https://www.rfc-editor.org/info/rfc9051?utm_source=chatgpt.com "Information on RFC 9051 - » RFC Editor"
[2]: https://www.rfc-editor.org/rfc/rfc2177.html?utm_source=chatgpt.com "RFC 2177: IMAP4 IDLE command"
[3]: https://www.rfc-editor.org/rfc/rfc8620.html?utm_source=chatgpt.com "RFC 8620: The JSON Meta Application Protocol
    (JMAP)"
[4]: https://developers.google.com/workspace/gmail/imap/xoauth2-protocol?utm_source=chatgpt.com "OAuth 2.0 Mechanism |
    Gmail"
[5]:
    https://techcommunity.microsoft.com/blog/exchange/exchange-online-to-retire-basic-auth-for-client-submission-smtp-auth/4114750?utm_source=chatgpt.com
    "Exchange Online to retire Basic auth for Client Submission ..."
[6]: https://datatracker.ietf.org/doc/html/rfc7162?utm_source=chatgpt.com "RFC 7162 - IMAP Extensions: Quick Flag
    Changes ..."
[7]: https://developers.google.com/workspace/gmail/imap/imap-smtp?utm_source=chatgpt.com "IMAP, POP, and SMTP | Gmail"
[8]: https://www.rfc-editor.org/rfc/rfc6154.html?utm_source=chatgpt.com "RFC 6154: IMAP LIST Extension for Special-Use
    Mailboxes"
