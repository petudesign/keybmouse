# Keybmouse 0.2.1

**Keskeneräinen prototyyppi — kehitys jatkuu.** Tämä repo säilyttää nykyisen lähdekoodin, asetusten toteutuksen ja testit. Kyseessä ei ole valmis julkaisu.

Kevyt Windows-ohjelma, jolla hiirtä ohjataan näppäimistöltä. Rust-ydin, natiivi asetusikkuna ja ilmoitusalueen kuvake. Ei Electronia, Tauri-riippuvuutta, automaattista käynnistystä tai ylläpitäjän oikeuksien vaatimusta. macOS-ohjaus on vielä toteuttamatta.

## Käynnistä

Nykyisessä paikallisessa kehityskansiossa voit avata **Keybmouse-0.2.1.exe**-tiedoston kaksoisnapsauttamalla. PowerShelliä tai Rustia ei tarvita valmiin exe-tiedoston käyttöön. Sama koonti löytyy myös `target\release\keybmouse.exe`-polusta.

Exe-tiedostot ja paikallinen työkaluketju eivät kuulu tähän repoon. Uudella koneella kloonaa repo, asenna vakaa Rust ja Windowsin C++ build tools / SDK ja suorita `cargo build --release`. Käynnistä sitten `target\release\keybmouse.exe`.

- **Tallenna muutokset** tarkistaa asetukset, tallentaa ne ja ottaa ne käyttöön.
- **Palauta oletukset** täyttää lomakkeen oletuksilla; paina Tallenna ottaaksesi ne käyttöön.
- **Ohjaus käytössä** keskeyttää tai jatkaa ohjausta heti. Tauko on istuntokohtainen.
- **Piilota taustalle** ja ikkunan X piilottavat ikkunan ilmoitusalueelle. Ohjelma jatkaa toimintaansa.
- Napsauta ilmoitusalueen Keybmouse-kuvaketta avataksesi asetukset. Oikean painikkeen valikossa ovat asetukset, ohjauksen keskeytys ja **Lopeta**.
- **Lopeta** sulkee ohjelman ja vapauttaa sen painamat hiiren painikkeet.
- **Perusasetukset** näyttää nykyiset sidonnat värillisessä näppäimistökartassa sekä activation- ja drag-tilat.
- **Liikkeen tuntuma** sisältää nopeuden, kiihtyvyyden, tarkkuuden ja vierityksen säädöt omassa näkymässään.

## Oletusohjaus

Pidä **Caps Lock** pohjassa:

| Näppäin | Toiminto |
| --- | --- |
| W / A / S / D | Ylös / vasemmalle / alas / oikealle |
| Enter | Vasen painike; pidä pohjassa raahataksesi |
| Space | Oikea painike |
| Q / E | Vieritys ylös / alas |
| Kumpi tahansa Shift | Tarkkuustila |

Aktivointinäppäimen vapautus lopettaa liikkeen, vierityksen ja raahauksen. Jo kaapatut näppäimet pysyvät kaapattuina fyysiseen vapautukseen asti, jotta Enter tai kirjaimet eivät vuoda kirjoittamiseen. Caps Lock ei vaihda kirjainkokoa ohjelman ollessa käytössä oletussidonnalla; aikaisempi kirjainkokotila säilyy.

## Yhden käden käyttö

Asetuksissa **Aktivointitila → Vaihtokytkin (Toggle)** aktivoi hiiritilan yhdellä painalluksella ja poistaa sen seuraavalla. Aktivointinäppäintä ei tarvitse pitää pohjassa muiden näppäinten aikana.

**Raahaustila → Lukitse painamalla (Toggle)** muuttaa vasemman painikkeen painalluksen kaksivaiheiseksi: ensimmäinen painallus ottaa raahauksen käyttöön ja toinen vapauttaa painikkeen. Liikkuminen, klikkaus ja vieritys voidaan siten tehdä yhdellä kädellä ilman aktivointi- tai drag-chordia. Oikea painike ja tarkkuusnäppäin ovat tässä versiossa edelleen hold-toimintoja.

Yhden käden testilista:

- valitse Toggle-aktivointi ja testaa selain, ikkunan vaihto sekä asetusten muokkaus yhdellä kädellä
- testaa tekstin valinta: Toggle-drag päälle, liikuta, Toggle-drag pois
- testaa drag & drop ja vieritys ilman toisen käden apua
- kirjaa erikseen, jos nykyinen näppäinsijoittelu vaatii sormien venyttämistä; kaikki sidonnat voi vaihtaa toiminnolle sopiviksi

Vielä tietoisesti avoimeksi jäävät vasemman ja oikean käden valmiit presetit sekä Precision moden Toggle-versio. Näiden oletusnäppäimet kannattaa valita oikealla laitteella tehtyjen yhden käden testien perusteella, ei olettamalla WASD-asettelua.

## Liikkeen tuntuma

Suunnan vaihtaminen säilyttää saavutetun nopeuden. Näppäinten vapautus pysäyttää kursorin heti: ei liukumista. Lyhyt tauko säilyttää nopeuden seuraavaa suuntaa varten (oletus 100 ms). Pidempi tauko tai aktivointinäppäimen vapautus palauttaa hitaan aloituksen. Diagonaalit normalisoidaan ja vastakkaiset suunnat kumoavat toisensa.

Asetuksissa ovat lähtönopeus, enimmäisnopeus, kiihdytys, tarkkuusnopeus prosentteina, vieritysnopeus ja suunnanvaihdon jousto. Esimerkiksi 18 % tarkoittaa 18 prosenttia tavallisesta nopeudesta; tallennusmuoto säilyy yhteensopivana aiemman version kanssa. Pilkku ja piste hyväksytään desimaalierottimina. Tasaisen nopeuden saa asettamalla lähtö- ja enimmäisnopeuden samoiksi. Nopeus on suhteellisia hiiriliikkeen yksiköitä sekunnissa, kiihdytys yksiköitä/s². Tämä ei muuta fyysisen hiiren DPI:tä. Windowsin omat hiiriasetukset vaikuttavat lopputulokseen.

Sidonnoiksi voi valita A–Z, 0–9, F1–F12, nuolinäppäimet, Caps Lockin, Enterin, Spacen, Tabin, Backspacen, Escapen ja vasemman/oikean Shiftin. Jokaisella toiminnolla pitää olla eri näppäin. Näppäinyhdistelmiä tai hiiren lisäpainikkeita ei vielä tueta. Sidonnat ovat loogisia näppäimiä, eivät fyysisiä skannauskoodeja.

Asetukset ovat käyttäjäkohtaisessa `%LOCALAPPDATA%\Keybmouse\settings.conf`-tiedostossa. Kirjoitus korvaa vanhan tiedoston vasta uuden tiedoston valmistuttua. Virheellisiä asetuksia ei tallenneta. Jos tiedostoa ei voi lukea, ohjelma käyttää oletuksia ja näyttää syyn; vanhaa tiedostoa ei muuteta ennen Tallenna-painiketta.

## Kehitys ja testit

Tässä työtilassa on projektikohtainen Rust 1.98.1 / GNU -työkaluketju ja LLVM-MinGW. Ne eivät muuta järjestelmän PATH-asetusta. Työkaluketju ja käännöstulokset eivät kuulu versionhallintaan.

```powershell
cd "C:\Users\petsk\Documents\ChatGPT\Keybmouse"
.\dev.ps1 test --offline
.\dev.ps1 build --release --offline
.\target\release\keybmouse.exe --debug
```

Tuoreessa checkoutissa käytä vakaata Rustia ja Windowsin C++ build tools / SDK -ympäristöä: `cargo test` ja `cargo build --release`. `--debug` näyttää siirtymälokit, kun ohjelma käynnistetään konsolista. `--smoke-test` asentaa oikean näppäimistökoukun sekunniksi ja lopettaa avaamatta asetusikkunaa.

14 testiä kattavat normaalin kirjoittamisen, näppäintoiston, diagonaalit, raahauksen, tarkkuuden, vierityksen, suunnanvaihdon tauon, keskeytyksen, sidontojen vaihtamisen, callbackin uudelleenkutsun hiirikomennon aikana sekä oikean Windows-asetuslomakkeen tallennuksen, validoinnin ja uudelleenlatauksen. Ulkoasun tarkistus ja liikkeen miellyttävyys oikealla näppäimistöllä ovat erillisiä testejä.

## Rakenne

- `src/config.rs`: siirrettävät näppäimet, asetukset, validointi ja tiedostomuoto.
- `src/core.rs`: käyttöjärjestelmästä riippumaton tila, näppäinten omistus, liike ja painikkeet. `InputListener` ja `PointerOutput` erottavat alustarajapinnat.
- `src/platform/windows.rs`: oma input-säie, `WH_KEYBOARD_LL`, ajastettu liike ja `SendInput`. Hiirikomennot suoritetaan tilalainan päätyttyä, koska Windows voi kutsua koukkua kesken `SendInput`-kutsun.
- `src/platform/windows_ui.rs`: natiivit kontrollit, ilmoitusalue, atominen tallennus ja asetuskomentojen lähetys input-säikeelle. Ikkunan siirtäminen tai valikko ei pysäytä input-säiettä.
- `src/platform/mod.rs`: alustavalinta ja macOS:n puuttuvan toteutuksen ilmoitus.

Ainoa suora riippuvuus on Microsoftin `windows-sys` 0.61.2, vain Windowsilla. `Cargo.lock` lukitsee sen ja `windows-link`-riippuvuuden. Lähteet: [Microsoftin Rust-rajapinnat](https://github.com/microsoft/windows-rs), [julkaisut](https://github.com/microsoft/windows-rs/releases), [SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput), [näppäimistökoukku](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc).

## Rajaukset

- Ylläpitäjänä toimivat ohjelmat, UAC, lukitusnäyttö ja jotkin pelit voivat estää synteettisen syötteen. Syötevirhe pysäyttää ohjelman ja käynnistää painikkeiden vapautusyrityksen.
- Pakotettu prosessin lopetus, näppäimistön irrotus ilman vapautustapahtumaa tai käyttöjärjestelmän poistama koukku eivät ole täysin palautettavissa. Vapauta ja paina aktivointinäppäintä uudelleen; tarvittaessa lopeta ohjelma ja napsauta fyysistä hiiren painiketta.
- Älä pidä fyysistä ja synteettistä samaa hiiren painiketta yhtä aikaa pohjassa. Windows yhdistää niiden tilan.
- Ennen aktivointia pohjassa olevat näppäimet säilyttävät normaalin merkityksensä. Esimerkiksi ennen aktivointia painettu Shift voi vaikuttaa myös hiiren klikkaukseen kohdeohjelmassa.
- UI käyttää järjestelmän skaalausta käynnistyshetkellä. Eri skaalausten näyttöjen välillä siirtäminen voi vaatia uudelleenkäynnistyksen parhaan terävyyden saamiseksi.
- Asennusohjelmaa, omaa viimeisteltyä kuvaketta, profiileja, automaattista käynnistystä tai ääniohjausta ei vielä ole.

macOS: lisää CoreGraphics-tapahtumakuuntelija ja `PointerOutput`-adapteri, käyttöoikeuspyynnöt sekä natiivi asetusikkuna. Rustin ydintä ja asetuksia voi käyttää sellaisinaan. Caps Lockin todellinen painallus/vapautuskäyttäytyminen on validoitava erikseen macOS:llä. Ääniohjaus on jätetty tarkoituksella myöhemmäksi.
