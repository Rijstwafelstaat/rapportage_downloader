# Rapportage Downloader
Deze applicatie haalt de nieuwste versie van een rapport op. Je moet hiervoor wel ingelogd zijn op de website en de cookies van DB Energie kopiëren. Let op dat deze cookies ongeldig worden zodra je uitgelogd bent of automatisch uitgelogd wordt. Deze applicatie draait momenteel alleen in de terminal.
## Dependencies
[Rust](https://www.rust-lang.org/tools/install)
## Compileren
Je kan de applicatie compileren met `cargo build` of `cargo run` met eventueel de `--release` flag, afhankelijk van of je de debug of geoptimaliseerde versie wilt. Het verschil tuseen `cargo build` en `cargo run` is dat je bij `cargo run` de binary direct runt. Aangezien het verplicht is om direct de nodige informatie als argument te geven, moet je `--` tussen het run command en de argumenten zetten.
## Runnen
Je kan de binary direct runnen in de terminal of met het `cargo run` command. Bij het runnen moeten een email-adres, wachtwoord en de benodigde rapportage meegegeven worden. Het email-adres en wachtwoord moeten hetzelfde zijn als die je gebruikt om bij DB Energie in te loggen. De rapportage dient meegegevent te worden met de `-r` of `--report` flag. De volgende rapportages zijn op dit moment downloadbaar:
- `aansluitinglijst`: Energie aansluitingenlijst
- `belastingcluster`: Energie belastingcluster per meter
- `gebouwen`: Gebouwen
- `meet-en-infra`: Meet- en infradiensten
- `metadata`: Aansluiting metadata
- `meterstanden`: Meterstanden van het huidige jaar
- `tussenmeter`: Tussenmeters
## Todo
Voor de volgende rapportages is nog meer werk nodig:
- meterstanden van vorige jaren
- verbruik (per product)
- Verbruik (in MJ of CO2)
- Datacompleetheid
- Datakwaliteitsrapportage

De volgende rapportages zijn niet downloadbaar:
- Energie analyse
- Energie analyse - extern
- Meetdata Export

Werkt niet op de site:
- Verbruiks notities
