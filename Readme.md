# Rapportage Downloader
Deze applicatie haalt de nieuwste versie van een rapport op. Deze applicatie draait alleen in de terminal.
## Dependencies
[Rust](https://www.rust-lang.org/tools/install)
## Compileren
Je kan de applicatie compileren met `cargo build` of `cargo run` met eventueel de `--release` flag, afhankelijk van of je de debug of geoptimaliseerde versie wilt. Het verschil tuseen `cargo build` en `cargo run` is dat je bij `cargo run` de binary direct runt. Aangezien het verplicht is om direct de nodige informatie als argument te geven, moet je `--` tussen het run command en de argumenten zetten.
## Runnen
### Argumenten
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

Om meer info te krijgen over de mogelijke argumenten kan je `-h` of `--help` gebruiken.
### Docker
Naast een lokale binary kan je het ook builden en runnen met docker. Op deze manier hoeft Rust niet geïnstalleerd te worden op de computer waar de applicatie op draait. In plaats daarvan wordt Rust geïnstalleerd in de container of wordt er een container geladen waar Rust al in geïnstalleerd is. Het gebruik is ongeveer hetzelfde, maar in plaats van `cargo run --release --` moet je `docker run -e MAIL=[e-mail] -e PASSWORD="wachtwoord" -e OUTPUT="http(s)://pad.naar.server/" rapportage_downloader` gebruiken. Indien nodig, kan daar `-e REPORT=rapportage-naam` aan toegevoegd worden voor een andere rapportage dan die met meterstanden. Het is bij Docker belangrijk dat de argumenten tussen `run` en `rapportage_downloader` komen, want anders begrijpt Docker het niet. Indien je de client wilt testen, kan je een bericht naar localhost sturen door `--add-host host.docker.internal:host-gateway` te plaatsen bij de argumenten.
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
