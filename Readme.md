# Rapportage Downloader
Deze applicatie haalt de nieuwste versie van een rapport op. Je moet hiervoor wel ingelogd zijn op de website en de cookies van DB Energie kopiëren. Let op dat deze cookies ongeldig worden zodra je uitgelogd bent of automatisch uitgelogd wordt. Deze applicatie draait momenteel alleen in de terminal.
## Dependencies
[Rust](https://www.rust-lang.org/tools/install)
## Compileren
Je kan de applicatie compileren met `cargo build` of `cargo run` met eventueel de `--release` flag, afhankelijk van of je de debug of geoptimaliseerde versie wilt. Het verschil tuseen `cargo build` en `cargo run` is dat je bij `cargo run` de binary direct runt. Aangezien het verplicht is om direct de nodige informatie als argument te geven, moet je `--` tussen het run command en de argumenten zetten.
## Runnen
Je kan de binary direct runnen in de terminal of met het `cargo run` command. Bij het runnen moeten een email-adres, wachtwoord, een output path/url en de benodigde rapportage meegegeven worden. Het email-adres en wachtwoord moeten hetzelfde zijn als die je gebruikt om bij DB Energie in te loggen. Het email-adres dient meegegeven te worden met `-m` of `--mail` en het wachtwoord met `-p` of `--password`. De output path/url kan een directory path of url zijn en dient meegegeven te worden met `-o` of `--output`. De rapportage dient meegegevent te worden met de `-r` of `--report` flag. De volgende rapportages zijn op dit moment downloadbaar:
- `aansluitinglijst`: Energie aansluitingenlijst
- `belastingcluster`: Energie belastingcluster per meter
- `co2`: Verbruik (in CO2)
- `datakwaliteit`: Datakwaliteits rapportage
- `gebouwen`: Gebouwen
- `meet-en-infra`: Meet- en infradiensten
- `metadata`: Aansluiting metadata
- `meterstanden`: Meterstanden van het huidige jaar
- `mj` Verbruik (in MJ)
- `tussenmeter`: Tussenmeters
- `verbruik`: verbruik (per product)
## Todo
Voor de volgende rapportages is nog meer werk nodig:
- meterstanden buiten 2023
- Datakwaliteits rapportage buiten november 2023
- Verbruik rapportages buiten 2023

De volgende rapportages zijn niet downloadbaar:
- Energie analyse
- Energie analyse - extern
- Meetdata Export

Werkt niet op de site:
- Datacompleetheid
- Verbruiks notities
