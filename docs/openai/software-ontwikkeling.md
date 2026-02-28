# Software ontwikkeling

Denk aan deze zaken, in deze volgorde: wat het product precies moet doen, hoe je het gaat bouwen, hoe je het gaat
runnen, en hoe je het gaat onderhouden.

1. Probleemdefinitie en scope Je moet helder hebben welke gebruiker je bedient, welk probleem je oplost, en welke
   uitkomst “succes” is. Leg dit vast als een korte scope (wat zit er expliciet wel in, wat expliciet niet) en een lijst
   met kernuse-cases. Definieer meteen niet-functionele eisen: performance, latency, schaal, beschikbaarheid,
   data-retentie, privacy, security, kostenplafond.
2. Requirements die testbaar zijn Schrijf requirements als acceptatiecriteria die je kunt verifiëren. Bijvoorbeeld: “Bij
   1.000 requests/min blijft p95 latency < 200 ms” of “Exports zijn idempotent en reproduceerbaar op dezelfde input.”
   Vermijd wensen die niet meetbaar zijn.
3. Architectuurkeuzes en grenzen Kies je grenzen: monolith vs services, sync vs async, event-driven of request/response,
   stateful of stateless. Teken je domein en de dataflow: waar ontstaat data, waar wordt het verrijkt, waar wordt het
   opgeslagen, waar wordt het geconsumeerd. Definieer interfaces (API-contracten), schema’s, en versioning-strategie.
4. Data-ontwerp en migraties Als je data opslaat: definieer schema’s, keys, constraints, indexen, en data-lifecycle
   (archiveren, purgen, back-ups, restore-test). Voor schema-evolutie: migratiestrategie (bijv. forward-only, backwards
   compatible changes, feature flags). Denk aan idempotentie en deduplicatie als je ingest/ETL doet.
5. Security by default Minimale rechten (least privilege), secrets management, inputvalidatie, authn/authz, audit
   logging, encryptie in transit, en waar nodig at rest. Denk ook aan supply-chain security: dependency pinning, SBOM,
   signed releases, en kwetsbaarheidsscans. Leg threat model vast: wat zijn je assets, aanvallers, en failure modes.
6. Betrouwbaarheid en failure modes Definieer SLO’s (beschikbaarheid, latency, error budget). Ontwerp voor retries,
   timeouts, backoff, circuit breakers, en “graceful degradation”. Zorg voor duidelijke error-classificatie (user error
   vs transient vs bug). Maak load shedding of queueing expliciet als dat relevant is.
7. Observability vanaf dag 1 Structured logging, metrics, tracing, en correlatie-ID’s. Definieer wat je moet kunnen
   beantwoorden: “waarom is dit request traag”, “waarom is dit record fout”, “wat is de doorlooptijd per stap”. Voeg
   dashboards en alerts toe op basis van SLO’s, niet op ruis.
8. Testingstrategie die past bij risico Unit tests voor pure logica, contract tests voor API’s, integratietests voor
   DB/queues, en e2e tests voor kritieke flows. Voeg property-based tests toe voor parsers/transformaties. Denk aan
   determinisme (tijd, random, externe calls) via dependency injection en fakes.
9. CI/CD en release discipline Automatische builds, linting, type checks, tests, security scans, en reproducible builds.
   Releases met semver en changelog. Feature flags voor risicovolle veranderingen. Rollback/rollforward plan, en een
   migratieplan dat downtime minimaliseert.
10. Deployment en operations Kies runtime: container, VM, bare metal, serverless. Definieer configuratie (12-factor),
    environment separation (dev/stage/prod), en infra as code. Plan capaciteit (CPU/RAM/IO), autoscaling waar relevant,
    en budgetbewaking. Zorg dat je een incident kunt afhandelen: runbooks, on-call afspraken (ook al ben je alleen), en
    postmortems.
11. Documentatie en “ways of working” Minimaal nodig: README, architecture notes (C4-achtig), API docs, data dictionary,
    runbook, en “how to debug”. Leg coding standards vast: formatting, naming, error handling, logging policy,
    migrations policy.
12. Legal en compliance (pragmatisch) Als je persoonsgegevens verwerkt: dataminimalisatie, retentie, verwerkers/derden,
    en rechten van betrokkenen. Voor zakelijke software: licenties van dependencies, IP, en export/cryptorestricties
    indien relevant.
