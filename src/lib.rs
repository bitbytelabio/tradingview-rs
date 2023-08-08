use reqwest::header::{HeaderMap, HeaderValue};

pub mod chart;
pub mod client;
pub mod error;
pub mod model;
mod prelude;
pub mod quote;
pub mod socket;
pub mod user;
pub mod utils;

static UA: &'static str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36";

lazy_static::lazy_static! {
    static ref WEBSOCKET_HEADERS: HeaderMap<HeaderValue> = {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        headers
    };
}

#[macro_export]
macro_rules! payload {
    ($($payload:expr),*) => {{
        let payload_vec = vec![$(serde_json::Value::from($payload)),*];
        payload_vec
    }};
}

use std::fmt;

#[derive(Debug)]
pub enum Timezone {
    AfricaCairo,
    AfricaCasablanca,
    AfricaJohannesburg,
    AfricaLagos,
    AfricaNairobi,
    AfricaTunis,
    AmericaAnchorage,
    AmericaArgentinaBuenosAires,
    AmericaBogota,
    AmericaCaracas,
    AmericaChicago,
    AmericaElSalvador,
    AmericaJuneau,
    AmericaLima,
    AmericaLosAngeles,
    AmericaMexicoCity,
    AmericaNewYork,
    AmericaPhoenix,
    AmericaSantiago,
    AmericaSaoPaulo,
    AmericaToronto,
    AmericaVancouver,
    AsiaAlmaty,
    AsiaAshkhabad,
    AsiaBahrain,
    AsiaBangkok,
    AsiaChongqing,
    AsiaColombo,
    AsiaDhaka,
    AsiaDubai,
    AsiaHoChiMinh,
    AsiaHongKong,
    AsiaJakarta,
    AsiaJerusalem,
    AsiaKarachi,
    AsiaKathmandu,
    AsiaKolkata,
    AsiaKuwait,
    AsiaManila,
    AsiaMuscat,
    AsiaNicosia,
    AsiaQatar,
    AsiaRiyadh,
    AsiaSeoul,
    AsiaShanghai,
    AsiaSingapore,
    AsiaTaipei,
    AsiaTehran,
    AsiaTokyo,
    AsiaYangon,
    AtlanticReykjavik,
    AustraliaAdelaide,
    AustraliaBrisbane,
    AustraliaPerth,
    AustraliaSydney,
    EuropeAmsterdam,
    EuropeAthens,
    EuropeBelgrade,
    EuropeBerlin,
    EuropeBratislava,
    EuropeBrussels,
    EuropeBucharest,
    EuropeBudapest,
    EuropeCopenhagen,
    EuropeDublin,
    EuropeHelsinki,
    EuropeIstanbul,
    EuropeLisbon,
    EuropeLondon,
    EuropeLuxembourg,
    EuropeMadrid,
    EuropeMalta,
    EuropeMoscow,
    EuropeOslo,
    EuropeParis,
    EuropeRiga,
    EuropeRome,
    EuropeStockholm,
    EuropeTallinn,
    EuropeVilnius,
    EuropeWarsaw,
    EuropeZurich,
    PacificAuckland,
    PacificChatham,
    PacificFakaofo,
    PacificHonolulu,
    PacificNorfolk,
    USMountain,
}

impl fmt::Display for Timezone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Timezone::AfricaCairo => write!(f, "Africa/Cairo"),
            Timezone::AfricaCasablanca => write!(f, "Africa/Casablanca"),
            Timezone::AfricaJohannesburg => write!(f, "Africa/Johannesburg"),
            Timezone::AfricaLagos => write!(f, "Africa/Lagos"),
            Timezone::AfricaNairobi => write!(f, "Africa/Nairobi"),
            Timezone::AfricaTunis => write!(f, "Africa/Tunis"),
            Timezone::AmericaAnchorage => write!(f, "America/Anchorage"),
            Timezone::AmericaArgentinaBuenosAires => write!(f, "America/Argentina/Buenos_Aires"),
            Timezone::AmericaBogota => write!(f, "America/Bogota"),
            Timezone::AmericaCaracas => write!(f, "America/Caracas"),
            Timezone::AmericaChicago => write!(f, "America/Chicago"),
            Timezone::AmericaElSalvador => write!(f, "America/El_Salvador"),
            Timezone::AmericaJuneau => write!(f, "America/Juneau"),
            Timezone::AmericaLima => write!(f, "America/Lima"),
            Timezone::AmericaLosAngeles => write!(f, "America/Los_Angeles"),
            Timezone::AmericaMexicoCity => write!(f, "America/Mexico_City"),
            Timezone::AmericaNewYork => write!(f, "America/New_York"),
            Timezone::AmericaPhoenix => write!(f, "America/Phoenix"),
            Timezone::AmericaSantiago => write!(f, "America/Santiago"),
            Timezone::AmericaSaoPaulo => write!(f, "America/Sao_Paulo"),
            Timezone::AmericaToronto => write!(f, "America/Toronto"),
            Timezone::AmericaVancouver => write!(f, "America/Vancouver"),
            Timezone::AsiaAlmaty => write!(f, "Asia/Almaty"),
            Timezone::AsiaAshkhabad => write!(f, "Asia/Ashkhabad"),
            Timezone::AsiaBahrain => write!(f, "Asia/Bahrain"),
            Timezone::AsiaBangkok => write!(f, "Asia/Bangkok"),
            Timezone::AsiaChongqing => write!(f, "Asia/Chongqing"),
            Timezone::AsiaColombo => write!(f, "Asia/Colombo"),
            Timezone::AsiaDhaka => write!(f, "Asia/Dhaka"),
            Timezone::AsiaDubai => write!(f, "Asia/Dubai"),
            Timezone::AsiaHoChiMinh => write!(f, "Asia/Ho_Chi_Minh"),
            Timezone::AsiaHongKong => write!(f, "Asia/Hong_Kong"),
            Timezone::AsiaJakarta => write!(f, "Asia/Jakarta"),
            Timezone::AsiaJerusalem => write!(f, "Asia/Jerusalem"),
            Timezone::AsiaKarachi => write!(f, "Asia/Karachi"),
            Timezone::AsiaKathmandu => write!(f, "Asia/Kathmandu"),
            Timezone::AsiaKolkata => write!(f, "Asia/Kolkata"),
            Timezone::AsiaKuwait => write!(f, "Asia/Kuwait"),
            Timezone::AsiaManila => write!(f, "Asia/Manila"),
            Timezone::AsiaMuscat => write!(f, "Asia/Muscat"),
            Timezone::AsiaNicosia => write!(f, "Asia/Nicosia"),
            Timezone::AsiaQatar => write!(f, "Asia/Qatar"),
            Timezone::AsiaRiyadh => write!(f, "Asia/Riyadh"),
            Timezone::AsiaSeoul => write!(f, "Asia/Seoul"),
            Timezone::AsiaShanghai => write!(f, "Asia/Shanghai"),
            Timezone::AsiaSingapore => write!(f, "Asia/Singapore"),
            Timezone::AsiaTaipei => write!(f, "Asia/Taipei"),
            Timezone::AsiaTehran => write!(f, "Asia/Tehran"),
            Timezone::AsiaTokyo => write!(f, "Asia/Tokyo"),
            Timezone::AsiaYangon => write!(f, "Asia/Yangon"),
            Timezone::AtlanticReykjavik => write!(f, "Atlantic/Reykjavik"),
            Timezone::AustraliaAdelaide => write!(f, "Australia/Adelaide"),
            Timezone::AustraliaBrisbane => write!(f, "Australia/Brisbane"),
            Timezone::AustraliaPerth => write!(f, "Australia/Perth"),
            Timezone::AustraliaSydney => write!(f, "Australia/Sydney"),
            Timezone::EuropeAmsterdam => write!(f, "Europe/Amsterdam"),
            Timezone::EuropeAthens => write!(f, "Europe/Athens"),
            Timezone::EuropeBelgrade => write!(f, "Europe/Belgrade"),
            Timezone::EuropeBerlin => write!(f, "Europe/Berlin"),
            Timezone::EuropeBratislava => write!(f, "Europe/Bratislava"),
            Timezone::EuropeBrussels => write!(f, "Europe/Brussels"),
            Timezone::EuropeBucharest => write!(f, "Europe/Bucharest"),
            Timezone::EuropeBudapest => write!(f, "Europe/Budapest"),
            Timezone::EuropeCopenhagen => write!(f, "Europe/Copenhagen"),
            Timezone::EuropeDublin => write!(f, "Europe/Dublin"),
            Timezone::EuropeHelsinki => write!(f, "Europe/Helsinki"),
            Timezone::EuropeIstanbul => write!(f, "Europe/Istanbul"),
            Timezone::EuropeLisbon => write!(f, "Europe/Lisbon"),
            Timezone::EuropeLondon => write!(f, "Europe/London"),
            Timezone::EuropeLuxembourg => write!(f, "Europe/Luxembourg"),
            Timezone::EuropeMadrid => write!(f, "Europe/Madrid"),
            Timezone::EuropeMalta => write!(f, "Europe/Malta"),
            Timezone::EuropeMoscow => write!(f, "Europe/Moscow"),
            Timezone::EuropeOslo => write!(f, "Europe/Oslo"),
            Timezone::EuropeParis => write!(f, "Europe/Paris"),
            Timezone::EuropeRiga => write!(f, "Europe/Riga"),
            Timezone::EuropeRome => write!(f, "Europe/Rome"),
            Timezone::EuropeStockholm => write!(f, "Europe/Stockholm"),
            Timezone::EuropeTallinn => write!(f, "Europe/Tallinn"),
            Timezone::EuropeVilnius => write!(f, "Europe/Vilnius"),
            Timezone::EuropeWarsaw => write!(f, "Europe/Warsaw"),
            Timezone::EuropeZurich => write!(f, "Europe/Zurich"),
            Timezone::PacificAuckland => write!(f, "Pacific/Auckland"),
            Timezone::PacificChatham => write!(f, "Pacific/Chatham"),
            Timezone::PacificFakaofo => write!(f, "Pacific/Fakaofo"),
            Timezone::PacificHonolulu => write!(f, "Pacific/Honolulu"),
            Timezone::PacificNorfolk => write!(f, "Pacific/Norfolk"),
            Timezone::USMountain => write!(f, "US/Mountain"),
        }
    }
}
