//! Zastrzeżenia prawne — wspólna treść dla programu, PDF i G-code.

/// Wersja treści zastrzeżeń; zmiana wymusza ponowną akceptację w programie.
pub const VERSION: u32 = 1;

pub const TITLE: &str = "Ważne — przeczytaj przed użyciem";

/// Pełna treść (akapity rozdzielone pustą linią).
pub const FULL: &str = "\
Pakieciarka jest narzędziem pomocniczym. Wszystkie wartości — napięcia, prądy, spadki i straty, dobór taśmy niklowej, \
przewodów, BMS, ładowarki i bezpieczników, masy i wymiary — są wyliczane z uproszczonych modeli, reguł kciuka \
i danych katalogowych, które mogą być nieaktualne lub błędne. Rysunki połączeń, wydruki PDF i wygenerowany G-code \
również mogą zawierać błędy.

Budowa, zgrzewanie, ładowanie i użytkowanie akumulatorów litowych wiąże się z ryzykiem pożaru, wybuchu, \
porażenia prądem, poparzeń i uszkodzenia mienia. Napięcie powyżej 60 V DC jest niebezpieczne dla życia.

Ty odpowiadasz za sprawdzenie wyników z kartami katalogowymi ogniw, BMS i taśmy oraz z obowiązującymi przepisami, \
za bezpieczeństwo pracy, za sprawdzenie G-code w trybie próbnym przed zgrzewaniem i za zbudowane urządzenie.

Program jest dostarczany „tak jak jest”, bez jakiejkolwiek gwarancji, także dorozumianej gwarancji przydatności \
do określonego celu (licencja GNU GPL v3, pkt 15 i 16). Autor i współtwórcy programu nie ponoszą odpowiedzialności \
za jakiekolwiek szkody wynikłe z użycia programu, jego wyników albo pakietów i urządzeń zbudowanych na ich podstawie. \
Korzystasz z programu na własną odpowiedzialność.";

/// Jedna linia — stopka PDF, pasek stanu.
pub const SHORT: &str = "Wartości orientacyjne, bez gwarancji — sprawdź z kartami katalogowymi. \
Korzystasz na własną odpowiedzialność; autor nie odpowiada za szkody (GNU GPL v3, pkt 15–16).";

/// Linie komentarza do nagłówka G-code.
pub const GCODE: [&str; 3] = [
    "UWAGA: program wygenerowany automatycznie, bez gwarancji poprawności.",
    "Uruchom najpierw w trybie próbnym, bez ogniw. Korzystasz na własną odpowiedzialność;",
    "autor Pakieciarki nie odpowiada za szkody (GNU GPL v3, pkt 15-16).",
];
